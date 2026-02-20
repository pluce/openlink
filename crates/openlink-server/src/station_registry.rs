//! Station registry backed by a JetStream KV store.
//!
//! Maps [`StationId`]s to their runtime status, network address, and ACARS
//! routing endpoint. Used by the server to resolve callsigns to routable
//! destinations.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use futures::TryStreamExt;
use openlink_models::{
    AcarsEndpointCallsign, AcarsRoutingEndpoint, NetworkAddress, NetworkId, StationId,
    StationStatus,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// A registry of ground stations on a single network.
#[derive(Debug, Clone)]
pub struct StationRegistry {
    kv_registry_store: async_nats::jetstream::kv::Store,
    kv_callsign_index_store: async_nats::jetstream::kv::Store,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CallsignIndexEntry {
    station_id: StationId,
}

/// A single entry in the station registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationEntry {
    pub station_id: StationId,
    pub status: StationStatus,
    pub last_updated: DateTime<Utc>,
    pub network_address: NetworkAddress,
    pub acars_endpoint: AcarsRoutingEndpoint,
}

impl StationRegistry {
    /// Create or bind to the KV bucket for the given network.
    pub async fn new(
        network_id: NetworkId,
        js: async_nats::jetstream::Context,
        force_reset: bool,
    ) -> Result<Self> {
        let bucket_name = openlink_sdk::NatsSubjects::kv_station_registry(&network_id);
        let callsign_index_bucket_name =
            openlink_sdk::NatsSubjects::kv_station_callsign_index(&network_id);
        if force_reset {
            info!(bucket = %bucket_name, "force-resetting station registry KV bucket");
            match js.delete_key_value(&bucket_name).await {
                Ok(_) => info!(bucket = %bucket_name, "bucket deleted"),
                Err(e) => debug!(bucket = %bucket_name, error = %e, "no bucket to delete"),
            }

            info!(bucket = %callsign_index_bucket_name, "force-resetting callsign index KV bucket");
            match js.delete_key_value(&callsign_index_bucket_name).await {
                Ok(_) => info!(bucket = %callsign_index_bucket_name, "bucket deleted"),
                Err(e) => debug!(bucket = %callsign_index_bucket_name, error = %e, "no bucket to delete"),
            }
        }
        let config = async_nats::jetstream::kv::Config {
            bucket: bucket_name.clone(),
            history: 1,
            ..Default::default()
        };
        let kv_registry_store = match js.create_key_value(config).await {
            Ok(store) => {
                info!(bucket = %bucket_name, "station registry KV bucket created");
                store
            }
            Err(_) => {
                debug!(bucket = %bucket_name, "bucket exists, binding");
                js.get_key_value(&bucket_name).await?
            }
        };

        let callsign_index_config = async_nats::jetstream::kv::Config {
            bucket: callsign_index_bucket_name.clone(),
            history: 1,
            ..Default::default()
        };
        let kv_callsign_index_store = match js.create_key_value(callsign_index_config).await {
            Ok(store) => {
                info!(bucket = %callsign_index_bucket_name, "station callsign index KV bucket created");
                store
            }
            Err(_) => {
                debug!(bucket = %callsign_index_bucket_name, "bucket exists, binding");
                js.get_key_value(&callsign_index_bucket_name).await?
            }
        };

        Ok(Self {
            kv_registry_store,
            kv_callsign_index_store,
        })
    }

    /// Look up a station by its [`StationId`].
    #[allow(dead_code)] // used in tests, will be used from handler code later
    pub async fn get_status(&self, station_id: &StationId) -> Result<Option<StationEntry>> {
        self.kv_registry_store
            .get(station_id.to_string())
            .await
            .map_err(|e| anyhow::anyhow!(e))?
            .map(|content| serde_json::from_slice::<StationEntry>(content.as_ref()))
            .transpose()
            .map_err(Into::into)
    }

    /// Find a station whose ACARS endpoint matches the given callsign.
    ///
    /// Uses a dedicated reverse-index KV bucket (`callsign -> station_id`) so
    /// lookups are O(1) instead of scanning the full registry.
    pub async fn lookup_callsign(
        &self,
        callsign: &AcarsEndpointCallsign,
    ) -> Result<Option<StationEntry>> {
        let callsign_key = callsign_index_key(callsign);
        let Some(index_content) = self.kv_callsign_index_store.get(&callsign_key).await? else {
            return Ok(None);
        };
        let index_entry: CallsignIndexEntry = serde_json::from_slice(index_content.as_ref())?;

        let Some(station_content) = self
            .kv_registry_store
            .get(index_entry.station_id.to_string())
            .await?
        else {
            return Ok(None);
        };

        let station_entry: StationEntry = serde_json::from_slice(station_content.as_ref())?;
        Ok(Some(station_entry))
    }

    /// Insert or update a station's status in the registry.
    pub async fn update_status(
        &self,
        station_id: &StationId,
        status: &StationStatus,
        acars_endpoint: &AcarsRoutingEndpoint,
        network_address: &NetworkAddress,
    ) -> Result<()> {
        // Remove stale callsign index if callsign changed for an existing station.
        if let Some(existing_content) = self.kv_registry_store.get(station_id.to_string()).await? {
            let existing: StationEntry = serde_json::from_slice(existing_content.as_ref())?;
            if existing.acars_endpoint.callsign != acars_endpoint.callsign {
                let old_key = callsign_index_key(&existing.acars_endpoint.callsign);
                self.kv_callsign_index_store
                    .delete(old_key)
                    .await
                    .ok();
            }
        }

        let entry = StationEntry {
            station_id: station_id.clone(),
            status: status.clone(),
            last_updated: Utc::now(),
            acars_endpoint: acars_endpoint.clone(),
            network_address: network_address.clone(),
        };
        self.kv_registry_store
            .put(station_id.to_string(), serde_json::to_vec(&entry)?.into())
            .await?;

        // Keep reverse index only for connected (online) stations.
        let callsign_key = callsign_index_key(&acars_endpoint.callsign);
        if *status == StationStatus::Online {
            let idx = CallsignIndexEntry {
                station_id: station_id.clone(),
            };
            self.kv_callsign_index_store
                .put(callsign_key, serde_json::to_vec(&idx)?.into())
                .await?;
        } else {
            self.kv_callsign_index_store
                .delete(callsign_key)
                .await
                .ok();
        }

        Ok(())
    }

    /// List all station entries from the registry bucket.
    pub async fn list_entries(&self) -> Result<Vec<StationEntry>> {
        let mut keys = self.kv_registry_store.keys().await?;
        let mut entries = Vec::new();

        while let Some(key) = keys.try_next().await? {
            if let Some(content) = self.kv_registry_store.get(&key).await? {
                let entry: StationEntry = serde_json::from_slice(content.as_ref())?;
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Mark stale online stations as offline based on a TTL lease.
    ///
    /// Returns the entries that were transitioned to offline.
    pub async fn expire_stale_online(&self, ttl: Duration) -> Result<Vec<StationEntry>> {
        let now = Utc::now();
        let entries = self.list_entries().await?;
        let mut expired = Vec::new();

        for entry in entries {
            if entry.status != StationStatus::Online {
                continue;
            }
            if now.signed_duration_since(entry.last_updated) <= ttl {
                continue;
            }

            self.update_status(
                &entry.station_id,
                &StationStatus::Offline,
                &entry.acars_endpoint,
                &entry.network_address,
            )
            .await?;
            expired.push(entry);
        }

        Ok(expired)
    }
}

fn callsign_index_key(callsign: &AcarsEndpointCallsign) -> String {
    callsign.to_string().to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlink_models::{AcarsEndpointCallsign, NetworkId, StationId, StationStatus};

    async fn setup_registry() -> StationRegistry {
        let nats_url = "nats://localhost:4222";
        let client = async_nats::connect(nats_url)
            .await
            .expect("Failed to connect to NATS server");
        let js = async_nats::jetstream::new(client.clone());
        let network_id = NetworkId::new("test_network");
        StationRegistry::new(network_id, js, false)
            .await
            .expect("create registry")
    }

    #[tokio::test]
    async fn test_update_and_get_status() {
        let registry = setup_registry().await;
        let station_id = StationId::new("station1");
        let status = StationStatus::Online;
        let acars_endpoint = AcarsRoutingEndpoint::new("HELO", "1234");
        let network_address = NetworkAddress::from("1234");

        registry
            .update_status(&station_id, &status, &acars_endpoint, &network_address)
            .await
            .expect("update status");

        let result = registry.get_status(&station_id).await.expect("get status");
        let found = result.expect("should exist");
        assert_eq!(found.station_id, station_id);
        assert_eq!(found.status, status);
        assert_eq!(found.acars_endpoint, acars_endpoint);
        assert_eq!(found.network_address, network_address);
    }

    #[tokio::test]
    async fn test_get_status_none() {
        let registry = setup_registry().await;
        let station_id = StationId::new("nonexistent");
        let result = registry.get_status(&station_id).await.expect("get status");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_lookup_callsign_uses_index() {
        let registry = setup_registry().await;
        let station_id = StationId::new("station_callsign");
        let status = StationStatus::Online;
        let acars_endpoint = AcarsRoutingEndpoint::new("LFPG", "ADDR1");
        let network_address = NetworkAddress::from("1234");

        registry
            .update_status(&station_id, &status, &acars_endpoint, &network_address)
            .await
            .expect("update status");

        let found = registry
            .lookup_callsign(&AcarsEndpointCallsign::new("LFPG"))
            .await
            .expect("lookup callsign")
            .expect("indexed callsign should exist");
        assert_eq!(found.station_id, station_id);
    }

    #[tokio::test]
    async fn test_lookup_callsign_removed_when_offline() {
        let registry = setup_registry().await;
        let station_id = StationId::new("station_offline");
        let acars_endpoint = AcarsRoutingEndpoint::new("EGLL", "ADDR2");
        let network_address = NetworkAddress::from("5678");

        registry
            .update_status(&station_id, &StationStatus::Online, &acars_endpoint, &network_address)
            .await
            .expect("online status");

        registry
            .update_status(&station_id, &StationStatus::Offline, &acars_endpoint, &network_address)
            .await
            .expect("offline status");

        let found = registry
            .lookup_callsign(&AcarsEndpointCallsign::new("EGLL"))
            .await
            .expect("lookup callsign");
        assert!(found.is_none());
    }
}

