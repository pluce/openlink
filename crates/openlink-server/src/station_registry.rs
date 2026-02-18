//! Station registry backed by a JetStream KV store.
//!
//! Maps [`StationId`]s to their runtime status, network address, and ACARS
//! routing endpoint. Used by the server to resolve callsigns to routable
//! destinations.

use anyhow::Result;
use chrono::{DateTime, Utc};
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
    ) -> Result<Self> {
        let bucket_name = openlink_sdk::NatsSubjects::kv_station_registry(&network_id);
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
        Ok(Self { kv_registry_store })
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
    /// **Note:** this performs a full scan of all keys in the KV bucket.
    /// Acceptable at low station counts; consider a secondary index if the
    /// registry grows large.
    pub async fn lookup_callsign(
        &self,
        callsign: &AcarsEndpointCallsign,
    ) -> Result<Option<StationEntry>> {
        let mut keys = self.kv_registry_store.keys().await?;
        while let Some(key) = keys.try_next().await? {
            if let Some(content) = self.kv_registry_store.get(&key).await? {
                let entry: StationEntry = serde_json::from_slice(content.as_ref())?;
                if entry.acars_endpoint.callsign == *callsign {
                    return Ok(Some(entry));
                }
            }
        }
        Ok(None)
    }

    /// Insert or update a station's status in the registry.
    pub async fn update_status(
        &self,
        station_id: &StationId,
        status: &StationStatus,
        acars_endpoint: &AcarsRoutingEndpoint,
        network_address: &NetworkAddress,
    ) -> Result<()> {
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
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlink_models::{NetworkId, StationId, StationStatus};

    async fn setup_registry() -> StationRegistry {
        let nats_url = "nats://localhost:4222";
        let client = async_nats::connect(nats_url)
            .await
            .expect("Failed to connect to NATS server");
        let js = async_nats::jetstream::new(client.clone());
        let network_id = NetworkId::new("test_network");
        StationRegistry::new(network_id, js)
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
}

