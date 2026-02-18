//! CPDLC session state machine and server-side message handler.
//!
//! Manages per-aircraft CPDLC sessions stored in a JetStream KV bucket,
//! processing meta-messages (logon, connection, NDA, termination) and
//! resolving ACARS callsigns to network station entries.

use anyhow::Result;
use openlink_models::{
    AcarsEndpointCallsign, AcarsEnvelope, AcarsRoutingEndpoint, CpdlcEnvelope, CpdlcMessageType,
    CpdlcMetaMessage, NetworkId,
};
use tracing::{debug, info, warn};

use crate::station_registry::{self, StationEntry};

/// A CPDLC session for a single aircraft.
///
/// Tracks up to two concurrent connections (active + inactive) and an
/// optional Next Data Authority for handover scenarios.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CPDLCSession {
    pub aircraft: AcarsRoutingEndpoint,
    pub active_connection: Option<CPDLCConnection>,
    pub inactive_connection: Option<CPDLCConnection>,
    pub next_data_authority: Option<AcarsRoutingEndpoint>,
}

/// A single CPDLC connection to a ground station within a session.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CPDLCConnection {
    pub station: AcarsRoutingEndpoint,
    pub logon: bool,
    pub connection: bool,
}

/// Wrapper around a session key derived from an aircraft endpoint address.
pub struct CPDLCSessionId(String);

impl std::fmt::Display for CPDLCSessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<CPDLCSessionId> for String {
    fn from(val: CPDLCSessionId) -> Self {
        val.0
    }
}

impl AsRef<str> for CPDLCSessionId {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl From<&AcarsRoutingEndpoint> for CPDLCSessionId {
    fn from(endpoint: &AcarsRoutingEndpoint) -> Self {
        CPDLCSessionId(endpoint.address.to_string())
    }
}

#[allow(dead_code)] // state machine methods exercised in tests, wired from production code as protocol grows
impl CPDLCConnection {
    pub fn new(station: AcarsRoutingEndpoint) -> Self {
        Self {
            station,
            logon: false,
            connection: false,
        }
    }

    /// Mark the connection as logged on.
    pub fn logon(&mut self) {
        debug!(station = ?self.station, "logon successful");
        self.logon = true;
    }

    /// Establish the connection (requires a prior successful logon).
    pub fn connect(&mut self) -> Result<()> {
        if self.logon {
            debug!(station = ?self.station, "connection established");
            self.connection = true;
        } else {
            warn!(station = ?self.station, "cannot connect without logon");
            return Err(anyhow::anyhow!("cannot connect without prior logon"));
        }
        Ok(())
    }

    /// Returns `true` when both logon and connection are complete.
    pub fn ready_exchange(&self) -> bool {
        self.logon && self.connection
    }

    /// Terminate the connection (logon stays intact).
    pub fn termination_request(&mut self) {
        debug!(station = ?self.station, "termination requested");
        self.connection = false;
    }
}


#[allow(dead_code)] // state machine methods exercised in tests, wired from production code as protocol grows
impl CPDLCSession {
    pub fn new(aircraft: AcarsRoutingEndpoint) -> Self {
        Self {
            aircraft,
            active_connection: None,
            inactive_connection: None,
            next_data_authority: None,
        }
    }

    /// Request logon to a ground station. Placed in the active slot if free,
    /// otherwise in the inactive slot.
    pub fn logon_request(&mut self, station: AcarsRoutingEndpoint) -> Result<()> {
        debug!(station = ?station, aircraft = ?self.aircraft, "logon requested");
        let connection = CPDLCConnection::new(station);
        if self.active_connection.is_none() {
            self.active_connection = Some(connection);
        } else {
            self.inactive_connection = Some(connection);
        }
        Ok(())
    }

    /// Mark the logon as accepted by `station`, wherever it sits.
    pub fn logon_accepted(&mut self, station: AcarsRoutingEndpoint) -> Result<()> {
        if let Some(ref mut conn) = self.active_connection
            && conn.station == station
        {
            conn.logon();
            return Ok(());
        }
        if let Some(ref mut conn) = self.inactive_connection
            && conn.station == station
        {
            conn.logon();
            return Ok(());
        }
        warn!(station = ?station, aircraft = ?self.aircraft, "no matching connection for logon acceptance");
        Ok(())
    }

    /// Handle a connection request from `station`.
    pub fn connection_request(&mut self, station: AcarsRoutingEndpoint) -> Result<()> {
        debug!(station = ?station, aircraft = ?self.aircraft, "connection requested");
        if let Some(ref mut conn) = self.active_connection
            && conn.station == station
        {
            return Ok(());
        }
        if let Some(ref mut conn) = self.inactive_connection
            && conn.station == station
        {
            return Ok(());
        }
        // NDA-based implicit logon
        if self
            .next_data_authority
            .as_ref()
            .is_some_and(|i| *i == station)
        {
            let mut nda_connection = CPDLCConnection::new(station);
            nda_connection.logon();
            if self.active_connection.is_none() {
                self.active_connection = Some(nda_connection);
            } else {
                self.inactive_connection = Some(nda_connection);
            }
            return Ok(());
        }
        warn!(station = ?station, aircraft = ?self.aircraft, "no matching connection for connection request");
        Ok(())
    }

    /// Mark the connection as accepted by `station`.
    pub fn connection_accepted(&mut self, station: AcarsRoutingEndpoint) -> Result<()> {
        debug!(station = ?station, aircraft = ?self.aircraft, "connection accepted");
        if let Some(ref mut conn) = self.active_connection
            && conn.station == station
        {
            conn.connect()?;
            return Ok(());
        }
        if let Some(ref mut conn) = self.inactive_connection
            && conn.station == station
        {
            conn.connect()?;
            return Ok(());
        }
        warn!(station = ?station, aircraft = ?self.aircraft, "no matching connection for connection acceptance");
        Ok(())
    }

    /// Designate a Next Data Authority for handover.
    pub fn next_data_authority(&mut self, station: AcarsRoutingEndpoint) -> Result<()> {
        debug!(station = ?station, aircraft = ?self.aircraft, "NDA designated");
        self.next_data_authority = Some(station);
        Ok(())
    }

    /// Terminate the connection with `station`. If it was the active connection
    /// the inactive one (if any) gets promoted.
    pub fn termination_request(&mut self, station: AcarsRoutingEndpoint) -> Result<()> {
        debug!(station = ?station, aircraft = ?self.aircraft, "termination requested");
        if let Some(ref mut conn) = self.active_connection
            && conn.station == station
        {
            conn.termination_request();
            self.active_connection = self.inactive_connection.take();
            return Ok(());
        }
        if let Some(ref mut conn) = self.inactive_connection
            && conn.station == station
        {
            conn.termination_request();
            return Ok(());
        }
        warn!(station = ?station, aircraft = ?self.aircraft, "no matching connection for termination");
        Ok(())
    }
}

/// Server-side CPDLC message handler.
///
/// Owns a JetStream KV store for per-aircraft sessions and a
/// [`StationRegistry`](station_registry::StationRegistry) for callsign
/// resolution.
pub struct CPDLCServer {
    kv_sessions_store: async_nats::jetstream::kv::Store,
    station_registry: station_registry::StationRegistry,
}

impl CPDLCServer {
    /// Create the CPDLC server, optionally wiping the KV bucket first
    /// (useful for tests).
    pub async fn new(
        network_id: NetworkId,
        js: async_nats::jetstream::Context,
        force_reset: bool,
    ) -> Result<Self> {
        let kv_sessions_bucket = openlink_sdk::NatsSubjects::kv_cpdlc_sessions(&network_id);
        let kv_sessions_config = async_nats::jetstream::kv::Config {
            bucket: kv_sessions_bucket.clone(),
            history: 1,
            ..Default::default()
        };
        if force_reset {
            info!(bucket = %kv_sessions_bucket, "force-resetting KV bucket");
            match js.delete_key_value(&kv_sessions_bucket).await {
                Ok(_) => info!(bucket = %kv_sessions_bucket, "bucket deleted"),
                Err(e) => debug!(bucket = %kv_sessions_bucket, error = %e, "no bucket to delete"),
            }
        }
        let kv_sessions_store = match js.create_key_value(kv_sessions_config).await {
            Ok(store) => {
                info!(bucket = %kv_sessions_bucket, "CPDLC sessions KV bucket created");
                store
            }
            Err(_) => {
                debug!(bucket = %kv_sessions_bucket, "bucket exists, binding");
                js.get_key_value(&kv_sessions_bucket).await?
            }
        };
        let station_registry =
            station_registry::StationRegistry::new(network_id, js.clone()).await?;
        Ok(Self {
            kv_sessions_store,
            station_registry,
        })
    }

    /// Entry point for CPDLC envelope processing — dispatches to meta or
    /// application handlers.
    pub async fn handle_cpdlc_message(
        &self,
        cpdlc: CpdlcEnvelope,
        acars: AcarsEnvelope,
    ) -> Result<Option<StationEntry>> {
        debug!(?cpdlc, "handling CPDLC message");
        match cpdlc.message {
            CpdlcMessageType::Application(ref msg) => {
                debug!(?msg, "CPDLC application message (not yet implemented)");
                Err(anyhow::anyhow!(
                    "CPDLC application message handling not implemented yet"
                ))
            }
            CpdlcMessageType::Meta(ref meta) => {
                debug!(?meta, "CPDLC meta message");
                self.handle_cpdlc_meta_message(meta.clone(), cpdlc.clone(), acars.clone())
                    .await
            }
        }
    }

    /// Resolve an ACARS callsign to a [`StationEntry`] via the registry.
    pub async fn resolve_endpoint(
        &self,
        callsign: &AcarsEndpointCallsign,
    ) -> Result<StationEntry> {
        self.station_registry
            .lookup_callsign(callsign)
            .await?
            .ok_or_else(|| anyhow::anyhow!("station with callsign {callsign:?} not found"))
    }

    /// Process a CPDLC meta-message (logon request/response, etc.), updating
    /// the aircraft's session in KV and returning the destination station.
    pub async fn handle_cpdlc_meta_message(
        &self,
        message: CpdlcMetaMessage,
        cpdlc: CpdlcEnvelope,
        acars: AcarsEnvelope,
    ) -> Result<Option<StationEntry>> {
        let aircraft = acars.routing.aircraft.clone();
        let source_station = self.resolve_endpoint(&cpdlc.source).await?;
        let destination_station = self.resolve_endpoint(&cpdlc.destination).await?;

        debug!(
            source = ?source_station,
            dest = ?destination_station,
            "meta message routing"
        );

        match message {
            CpdlcMetaMessage::LogonRequest { station, .. } => {
                info!(aircraft = ?aircraft, station = ?station, "processing logon request");
                let destination_logon = self.resolve_endpoint(&station).await?;
                self.get_and_update_session_for_aircraft(&aircraft, |maybe_session: Option<CPDLCSession>| {
                    let aircraft = aircraft.clone();
                    Box::pin(async move {
                        let mut session =
                            maybe_session.unwrap_or_else(|| CPDLCSession::new(aircraft));
                        session.logon_request(destination_logon.acars_endpoint)?;
                        Ok(Some(session))
                    })
                })
                .await?;
            }
            CpdlcMetaMessage::LogonResponse { accepted } => {
                info!(aircraft = ?aircraft, accepted, "processing logon response");
                if accepted {
                    self.get_and_update_session_for_aircraft(&aircraft, |maybe_session: Option<CPDLCSession>| {
                        let aircraft = aircraft.clone();
                        Box::pin(async move {
                            let mut session =
                                maybe_session.unwrap_or_else(|| CPDLCSession::new(aircraft));
                            session.logon_accepted(source_station.acars_endpoint)?;
                            Ok(Some(session))
                        })
                    })
                    .await?;
                }
            }
            _ => {
                warn!(?message, "unhandled CPDLC meta message type");
            }
        }
        Ok(Some(destination_station))
    }

    /// Atomically read-modify-write a session for the given aircraft.
    ///
    /// `update_fn` receives the current session (or `None` if new) and returns
    /// the updated value. A `None` return deletes the key.
    async fn get_and_update_session_for_aircraft(
        &self,
        aircraft: &AcarsRoutingEndpoint,
        update_fn: impl AsyncFnOnce(Option<CPDLCSession>) -> Result<Option<CPDLCSession>>,
    ) -> Result<Option<CPDLCSession>> {
        let session_id: String = CPDLCSessionId::from(aircraft).into();
        let found = self.kv_sessions_store.entry(&session_id).await?;

        let (revision, value) = match found {
            Some(entry) if !entry.value.is_empty() => {
                let existing: CPDLCSession = serde_json::from_slice(entry.value.as_ref())?;
                (entry.revision, Some(existing))
            }
            Some(entry) => (entry.revision, None),
            None => (0, None),
        };

        debug!(
            callsign = %aircraft.callsign,
            revision,
            has_value = value.is_some(),
            "fetched session"
        );

        let updated = update_fn(value).await?;

        if let Some(ref session) = updated {
            self.kv_sessions_store
                .update(&session_id, serde_json::to_vec(session)?.into(), revision)
                .await?;
        } else {
            self.kv_sessions_store.delete(&session_id).await?;
        }

        Ok(updated)
    }
}



#[cfg(test)]
mod tests {
    use anyhow::Result;
    use openlink_models::{AcarsRoutingEndpoint, NetworkId};

    use crate::acars::{CPDLCServer, CPDLCSession};

    #[test]
    fn test_cpdlc_session() {
        let mut session = CPDLCSession::new(AcarsRoutingEndpoint::new("TEST123", "abc"));
        let station1 = AcarsRoutingEndpoint::new("STATION1", "def");

        let _ = session.logon_request(station1.clone());
        assert_eq!(session.active_connection.as_ref().unwrap().station, station1);
        assert!(!session.active_connection.as_ref().unwrap().logon);

        let _ = session.logon_accepted(station1.clone());
        assert!(session.active_connection.as_ref().unwrap().logon);
        assert!(!session.active_connection.as_ref().unwrap().ready_exchange());

        let _ = session.connection_request(station1.clone());
        assert!(!session.active_connection.as_ref().unwrap().ready_exchange());

        let _ = session.connection_accepted(station1.clone());
        assert!(session.active_connection.as_ref().unwrap().ready_exchange());

        let _ = session.termination_request(station1.clone());
        assert!(session.active_connection.is_none());
    }

    #[test]
    fn test_cpdlc_session_switch() {
        let mut session = CPDLCSession::new(AcarsRoutingEndpoint::new("TEST123", "abc"));
        let station1 = AcarsRoutingEndpoint::new("STATION1", "def");
        let station2 = AcarsRoutingEndpoint::new("STATION2", "ghi");

        let _ = session.logon_request(station1.clone());
        let _ = session.logon_accepted(station1.clone());
        let _ = session.connection_request(station1.clone());
        let _ = session.connection_accepted(station1.clone());

        let _ = session.logon_request(station2.clone());
        let _ = session.logon_accepted(station2.clone());
        let _ = session.connection_request(station2.clone());
        let _ = session.connection_accepted(station2.clone());

        assert!(session.active_connection.as_ref().unwrap().ready_exchange());
        assert!(session.inactive_connection.as_ref().unwrap().ready_exchange());

        let _ = session.termination_request(station1.clone());
        assert!(session.active_connection.as_ref().unwrap().station == station2);
        assert!(session.inactive_connection.is_none());
    }

    #[test]
    fn test_cpdlc_session_without_logon() {
        let mut session = CPDLCSession::new(AcarsRoutingEndpoint::new("TEST123", "abc"));
        let station1 = AcarsRoutingEndpoint::new("STATION1", "def");

        let _ = session.logon_accepted(station1.clone());
        assert!(session.active_connection.is_none());

        let _ = session.connection_request(station1.clone());
        assert!(session.active_connection.is_none());
        let _ = session.connection_accepted(station1.clone());
        assert!(session.active_connection.is_none());
    }

    #[test]
    fn test_cpdlc_session_with_nda() {
        let mut session = CPDLCSession::new(AcarsRoutingEndpoint::new("TEST123", "abc"));
        let station1 = AcarsRoutingEndpoint::new("STATION1", "def");

        let _ = session.next_data_authority(station1.clone());

        let _ = session.connection_request(station1.clone());
        assert_eq!(session.active_connection.as_ref().unwrap().station, station1);

        let _ = session.connection_accepted(station1.clone());
        assert_eq!(session.active_connection.as_ref().unwrap().station, station1);
    }

    #[test]
    fn test_cpdlc_session_with_nda_transfer() {
        let mut session = CPDLCSession::new(AcarsRoutingEndpoint::new("TEST123", "abc"));
        let station1 = AcarsRoutingEndpoint::new("STATION1", "def");
        let station2 = AcarsRoutingEndpoint::new("STATION2", "ghi");

        let _ = session.logon_request(station1.clone());
        let _ = session.logon_accepted(station1.clone());
        let _ = session.connection_request(station1.clone());
        let _ = session.connection_accepted(station1.clone());
        let _ = session.next_data_authority(station2.clone());
        let _ = session.connection_request(station2.clone());
        let _ = session.connection_accepted(station2.clone());
        assert_eq!(session.active_connection.as_ref().unwrap().station, station1);
        assert_eq!(session.inactive_connection.as_ref().unwrap().station, station2);
        let _ = session.termination_request(station1.clone());
        assert_eq!(session.active_connection.as_ref().unwrap().station, station2);
        assert!(session.inactive_connection.is_none());
    }

    async fn setup_cpdlc_server() -> CPDLCServer {
        let nats_url = "nats://localhost:4222";
        let client = async_nats::connect(nats_url)
            .await
            .expect("Failed to connect to NATS server");
        let js = async_nats::jetstream::new(client.clone());
        let network_id = NetworkId::new("test_network");
        CPDLCServer::new(network_id, js, true)
            .await
            .expect("create server")
    }

    #[tokio::test]
    async fn test_get_and_update_session_for_aircraft_create_and_update() {
        let server = setup_cpdlc_server().await;
        let aircraft = AcarsRoutingEndpoint::new("TEST123", "abc");

        // Create a new session
        server
            .get_and_update_session_for_aircraft(
                &aircraft,
                async |_: Option<CPDLCSession>| -> Result<Option<CPDLCSession>> {
                    Ok(Some(CPDLCSession::new(aircraft.clone())))
                },
            )
            .await
            .expect("create session");

        // Update existing session
        server
            .get_and_update_session_for_aircraft(
                &aircraft,
                async |maybe: Option<CPDLCSession>| -> Result<Option<CPDLCSession>> {
                    let mut session = maybe.expect("session should exist");
                    session.next_data_authority =
                        Some(AcarsRoutingEndpoint::new("STATION1", "def"));
                    Ok(Some(session))
                },
            )
            .await
            .expect("update session");
    }
}