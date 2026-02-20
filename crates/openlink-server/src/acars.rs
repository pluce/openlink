//! CPDLC session state machine and server-side message handler.
//!
//! Manages per-aircraft CPDLC sessions stored in a JetStream KV bucket,
//! processing meta-messages (logon, connection, NDA, termination) and
//! resolving ACARS callsigns to network station entries.

use std::sync::{Arc, Mutex};

use anyhow::Result;
use futures::TryStreamExt;
use openlink_models::{
    AcarsEndpointCallsign, AcarsEnvelope, AcarsMessage, AcarsRoutingEndpoint,
    CpdlcApplicationMessage, CpdlcArgument, CpdlcConnectionPhase, CpdlcConnectionView, CpdlcDialogue,
    CpdlcEnvelope, CpdlcMessageType, CpdlcMetaMessage, CpdlcSessionView, DialogueState,
    NetworkId, OpenLinkEnvelope, OpenLinkMessage, ResponseAttribute, find_definition,
};
use tracing::{debug, info, warn};

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
    /// Next MIN to assign for aircraft-originated (downlink) messages (0–63 cyclic).
    #[serde(default)]
    pub min_counter_aircraft: u8,
    /// Next MIN to assign for station-originated (uplink) messages (0–63 cyclic).
    #[serde(default)]
    pub min_counter_station: u8,
    /// Active CPDLC dialogues awaiting a closing response.
    #[serde(default)]
    pub dialogues: Vec<CpdlcDialogue>,
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


}


#[allow(dead_code)] // state machine methods exercised in tests, wired from production code as protocol grows
impl CPDLCSession {
    pub fn new(aircraft: AcarsRoutingEndpoint) -> Self {
        Self {
            aircraft,
            active_connection: None,
            inactive_connection: None,
            next_data_authority: None,
            min_counter_aircraft: 0,
            min_counter_station: 0,
            dialogues: Vec::new(),
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
    pub fn logon_accepted(&mut self, station: &AcarsEndpointCallsign) -> Result<()> {
        if let Some(ref mut conn) = self.active_connection
            && conn.station.callsign == *station
        {
            conn.logon();
            return Ok(());
        }
        if let Some(ref mut conn) = self.inactive_connection
            && conn.station.callsign == *station
        {
            conn.logon();
            return Ok(());
        }
        warn!(station = ?station, aircraft = ?self.aircraft, "no matching connection for logon acceptance");
        Ok(())
    }

    /// Handle a connection request from `station`.
    pub fn connection_request(&mut self, station: &AcarsEndpointCallsign) -> Result<()> {
        debug!(station = ?station, aircraft = ?self.aircraft, "connection requested");
        if let Some(ref mut conn) = self.active_connection
            && conn.station.callsign == *station
        {
            return Ok(());
        }
        if let Some(ref mut conn) = self.inactive_connection
            && conn.station.callsign == *station
        {
            return Ok(());
        }
        // NDA-based implicit logon
        if self
            .next_data_authority
            .as_ref()
            .is_some_and(|i| i.callsign == *station)
        {
            let mut nda_connection = CPDLCConnection::new(AcarsRoutingEndpoint::new(station.to_string(), ""));
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
    pub fn connection_accepted(&mut self, station: &AcarsEndpointCallsign) -> Result<()> {
        debug!(station = ?station, aircraft = ?self.aircraft, "connection accepted");
        if let Some(ref mut conn) = self.active_connection
            && conn.station.callsign == *station
        {
            conn.connect()?;
            return Ok(());
        }
        if let Some(ref mut conn) = self.inactive_connection
            && conn.station.callsign == *station
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
    pub fn termination_request(&mut self, station: &AcarsEndpointCallsign) -> Result<()> {
        debug!(station = ?station, aircraft = ?self.aircraft, "termination requested");
        if self.active_connection.as_ref().is_some_and(|c| c.station.callsign == *station) {
            self.active_connection = self.inactive_connection.take();
            return Ok(());
        }
        if self.inactive_connection.as_ref().is_some_and(|c| c.station.callsign == *station) {
            self.inactive_connection = None;
            return Ok(());
        }
        warn!(station = ?station, aircraft = ?self.aircraft, "no matching connection for termination");
        Ok(())
    }
}

impl CPDLCConnection {
    /// Convert to a client-visible `CpdlcConnectionPhase`.
    pub fn phase(&self) -> CpdlcConnectionPhase {
        if self.connection {
            CpdlcConnectionPhase::Connected
        } else if self.logon {
            CpdlcConnectionPhase::LoggedOn
        } else {
            CpdlcConnectionPhase::LogonPending
        }
    }

    /// Convert to a client-visible `CpdlcConnectionView` with a given peer callsign.
    pub fn to_view(&self, peer: &AcarsEndpointCallsign) -> CpdlcConnectionView {
        CpdlcConnectionView {
            peer: peer.clone(),
            phase: self.phase(),
        }
    }
}

impl CPDLCSession {
    /// Build the session view from the aircraft's perspective.
    ///
    /// In the aircraft view, `peer` is the ground-station callsign.
    pub fn to_aircraft_view(&self) -> CpdlcSessionView {
        CpdlcSessionView {
            aircraft: Some(self.aircraft.callsign.clone()),
            aircraft_address: Some(self.aircraft.address.clone()),
            active_connection: self.active_connection.as_ref().map(|c| {
                c.to_view(&c.station.callsign)
            }),
            inactive_connection: self.inactive_connection.as_ref().map(|c| {
                c.to_view(&c.station.callsign)
            }),
            next_data_authority: self.next_data_authority.as_ref().map(|nda| nda.callsign.clone()),
        }
    }

    /// Build the session view from a specific ground station's perspective.
    ///
    /// In the station view, `peer` is the aircraft callsign.
    pub fn to_station_view(&self, station: &AcarsEndpointCallsign) -> CpdlcSessionView {
        let aircraft_callsign = &self.aircraft.callsign;
        let conn_to_view = |c: &CPDLCConnection| -> Option<CpdlcConnectionView> {
            if c.station.callsign == *station {
                Some(c.to_view(aircraft_callsign))
            } else {
                None
            }
        };
        CpdlcSessionView {
            aircraft: Some(aircraft_callsign.clone()),
            aircraft_address: Some(self.aircraft.address.clone()),
            active_connection: self.active_connection.as_ref().and_then(conn_to_view),
            inactive_connection: self.inactive_connection.as_ref().and_then(conn_to_view),
            next_data_authority: self.next_data_authority.as_ref().map(|nda| nda.callsign.clone()),
        }
    }
}

/// Server-side CPDLC message handler.
///
/// Owns a JetStream KV store for per-aircraft sessions.
/// Callsign-to-network-address resolution is handled by the caller
/// (`OpenLinkServer`) via the station registry — the CPDLC state
/// machine works purely with ACARS-level identifiers from the messages.
pub struct CPDLCServer {
    kv_sessions_store: async_nats::jetstream::kv::Store,
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
        Ok(Self {
            kv_sessions_store,
        })
    }

    /// Entry point for CPDLC envelope processing — dispatches to meta or
    /// application handlers.
    ///
    /// Returns `(destination_callsign, updated_session)` so the caller can
    /// resolve the callsign to a network address for forwarding and
    /// broadcast session updates.
    /// Returns `(destination_callsign, updated_session, modified_envelope)`.
    /// For application messages the envelope carries the server-assigned MIN;
    /// for meta messages the original envelope is returned unchanged.
    pub async fn handle_cpdlc_message(
        &self,
        cpdlc: CpdlcEnvelope,
        acars: AcarsEnvelope,
        original_envelope: &OpenLinkEnvelope,
    ) -> Result<(AcarsEndpointCallsign, Option<CPDLCSession>, OpenLinkEnvelope)> {
        debug!(?cpdlc, "handling CPDLC message");
        match cpdlc.message {
            CpdlcMessageType::Application(ref msg) => {
                debug!(?msg, "CPDLC application message");
                let (dest, session, modified_cpdlc) = self
                    .handle_cpdlc_application_message(msg.clone(), cpdlc.clone(), acars.clone())
                    .await?;
                // Rebuild the full envelope with the modified CPDLC (server-assigned MIN)
                let mut modified_env = original_envelope.clone();
                if let OpenLinkMessage::Acars(ref mut acars_env) = modified_env.payload {
                    acars_env.message = AcarsMessage::CPDLC(modified_cpdlc);
                }
                Ok((dest, session, modified_env))
            }
            CpdlcMessageType::Meta(ref meta) => {
                debug!(?meta, "CPDLC meta message");
                let (dest, session) = self
                    .handle_cpdlc_meta_message(meta.clone(), cpdlc.clone(), acars.clone())
                    .await?;
                Ok((dest, session, original_envelope.clone()))
            }
        }
    }

    /// Process a CPDLC application message (uplinks / downlinks).
    ///
    /// 1. Validates that the active connection is in `Connected` state.
    /// 2. Assigns a MIN (Message Identification Number, 0–63 cyclic).
    /// 3. Validates the MRN (Message Reference Number) if this is a response.
    /// 4. Creates / closes dialogues according to the message's response attribute.
    /// 5. Returns the destination callsign for forwarding.
    pub async fn handle_cpdlc_application_message(
        &self,
        msg: CpdlcApplicationMessage,
        cpdlc: CpdlcEnvelope,
        acars: AcarsEnvelope,
    ) -> Result<(AcarsEndpointCallsign, Option<CPDLCSession>, CpdlcEnvelope)> {
        let aircraft = acars.routing.aircraft.clone();
        let source = cpdlc.source.clone();
        let destination = cpdlc.destination.clone();

        info!(
            source = %source,
            dest = %destination,
            elements = msg.elements.len(),
            mrn = ?msg.mrn,
            "processing CPDLC application message"
        );

        // Shared cell to capture the modified application message (with assigned MIN)
        // from inside the async closure.
        let msg_cell: Arc<Mutex<Option<CpdlcApplicationMessage>>> =
            Arc::new(Mutex::new(None));
        let msg_cell_inner = msg_cell.clone();

        let updated_session = self
            .get_and_update_session_for_aircraft(&aircraft, |maybe_session: Option<CPDLCSession>| {
                let _aircraft = aircraft.clone();
                let source = source.clone();
                Box::pin(async move {
                    let mut session =
                        maybe_session.ok_or_else(|| anyhow::anyhow!("no CPDLC session for aircraft"))?;

                    let mut msg = msg; // make mutable inside closure

                    // Normalize free-text arguments server-side.
                    for element in &mut msg.elements {
                        if let Some(def) = find_definition(&element.id) {
                            for (idx, arg_type) in def.args.iter().enumerate() {
                                if matches!(arg_type, openlink_models::ArgType::FreeText)
                                    && let Some(CpdlcArgument::FreeText(text)) = element.args.get_mut(idx)
                                {
                                    *text = text.to_uppercase();
                                }
                            }
                        }
                    }

                    // Determine if the source is the aircraft (downlink) or a station (uplink).
                    let is_downlink = source == session.aircraft.callsign;

                    // Validate the active connection is Connected.
                    let active = session
                        .active_connection
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("no active connection"))?;
                    if !active.ready_exchange() {
                        return Err(anyhow::anyhow!(
                            "active connection is not in Connected state"
                        ));
                    }

                    // Assign MIN.
                    let min = if is_downlink {
                        let m = session.min_counter_aircraft;
                        session.min_counter_aircraft = (m + 1) % 64;
                        m
                    } else {
                        let m = session.min_counter_station;
                        session.min_counter_station = (m + 1) % 64;
                        m
                    };
                    msg.min = min;

                    // Compute the effective response attribute (multi-element precedence).
                    let effective_attr = msg.effective_response_attr();

                    // Validate MRN if present — must reference an open dialogue.
                    if let Some(mrn) = msg.mrn {
                        // Check that the referenced dialogue exists and is open.
                        let dialogue_exists = session
                            .dialogues
                            .iter()
                            .any(|d| d.initiator_min == mrn && d.state == DialogueState::Open);
                        if !dialogue_exists {
                            warn!(mrn, "MRN does not reference an open dialogue — forwarding anyway");
                        }

                        // Determine if this response closes the dialogue.
                        // STANDBY (DM2, UM1, UM2) does NOT close the dialogue.
                        let is_standby = msg.elements.iter().any(|e| {
                            matches!(e.id.as_str(), "DM2" | "UM1" | "UM2")
                        });

                        if !is_standby {
                            // Close the referenced dialogue.
                            if let Some(d) = session
                                .dialogues
                                .iter_mut()
                                .find(|d| d.initiator_min == mrn && d.state == DialogueState::Open)
                            {
                                d.state = DialogueState::Closed;
                                debug!(mrn, "dialogue closed by response");
                            }
                        } else {
                            debug!(mrn, "STANDBY — dialogue remains open");
                        }
                    }

                    // Open a new dialogue if this message expects a response.
                    match effective_attr {
                        ResponseAttribute::N | ResponseAttribute::NE => {
                            // No response expected — no dialogue to track.
                        }
                        attr => {
                            session.dialogues.push(CpdlcDialogue {
                                initiator_min: min,
                                initiator: source.clone(),
                                state: DialogueState::Open,
                                response_attr: attr,
                            });
                            debug!(min, ?attr, "dialogue opened");
                        }
                    }

                    // Garbage-collect closed dialogues (keep only open ones + last 16 closed).
                    let open_count = session.dialogues.iter().filter(|d| d.state == DialogueState::Open).count();
                    if session.dialogues.len() > open_count + 16 {
                        // Remove oldest closed dialogues.
                        let mut closed_removed = 0;
                        let target_removals = session.dialogues.len() - open_count - 16;
                        session.dialogues.retain(|d| {
                            if d.state == DialogueState::Closed && closed_removed < target_removals {
                                closed_removed += 1;
                                false
                            } else {
                                true
                            }
                        });
                    }

                    // Store the modified message for the outer scope.
                    *msg_cell_inner.lock().unwrap() = Some(msg);

                    Ok(Some(session))
                })
            })
            .await?;

        // Reconstruct the CPDLC envelope with the modified application message.
        let modified_msg = msg_cell
            .lock()
            .unwrap()
            .take()
            .expect("application message should have been set by closure");
        let modified_cpdlc = CpdlcEnvelope {
            source: cpdlc.source.clone(),
            destination: cpdlc.destination.clone(),
            message: CpdlcMessageType::Application(modified_msg),
        };

        Ok((destination, updated_session, modified_cpdlc))
    }

    /// Process a CPDLC meta-message (logon request/response, etc.), updating
    /// the aircraft's session in KV and returning the destination callsign
    /// together with the updated session.
    ///
    /// This method works purely with ACARS-level identifiers from the
    /// messages — no station-registry lookups are performed here.
    /// The caller is responsible for resolving the returned callsign
    /// to a network address for routing.
    pub async fn handle_cpdlc_meta_message(
        &self,
        message: CpdlcMetaMessage,
        cpdlc: CpdlcEnvelope,
        acars: AcarsEnvelope,
    ) -> Result<(AcarsEndpointCallsign, Option<CPDLCSession>)> {
        let aircraft = acars.routing.aircraft.clone();

        debug!(
            source = %cpdlc.source,
            dest = %cpdlc.destination,
            "meta message routing"
        );

        let updated_session = match message {
            CpdlcMetaMessage::LogonRequest { station, .. } => {
                info!(aircraft = ?aircraft, station = ?station, "processing logon request");
                // The station callsign comes from the message field.
                // We build a minimal AcarsRoutingEndpoint for the session state machine.
                let station_endpoint = AcarsRoutingEndpoint::new(station.to_string(), "");
                self.get_and_update_session_for_aircraft(&aircraft, |maybe_session: Option<CPDLCSession>| {
                    let aircraft = aircraft.clone();
                    Box::pin(async move {
                        let mut session =
                            maybe_session.unwrap_or_else(|| CPDLCSession::new(aircraft));
                        session.logon_request(station_endpoint)?;
                        Ok(Some(session))
                    })
                })
                .await?
            }
            CpdlcMetaMessage::LogonResponse { accepted } => {
                let source_callsign = cpdlc.source.clone();
                info!(aircraft = ?aircraft, accepted, source = %source_callsign, "processing logon response");
                if accepted {
                    self.get_and_update_session_for_aircraft(&aircraft, |maybe_session: Option<CPDLCSession>| {
                        let aircraft = aircraft.clone();
                        Box::pin(async move {
                            let mut session =
                                maybe_session.unwrap_or_else(|| CPDLCSession::new(aircraft));
                            session.logon_accepted(&source_callsign)?;
                            Ok(Some(session))
                        })
                    })
                    .await?
                } else {
                    None
                }
            }
            CpdlcMetaMessage::ConnectionRequest => {
                let source_callsign = cpdlc.source.clone();
                info!(aircraft = ?aircraft, source = %source_callsign, "processing connection request");
                self.get_and_update_session_for_aircraft(&aircraft, |maybe_session: Option<CPDLCSession>| {
                    let aircraft = aircraft.clone();
                    Box::pin(async move {
                        let mut session =
                            maybe_session.unwrap_or_else(|| CPDLCSession::new(aircraft));
                        session.connection_request(&source_callsign)?;
                        Ok(Some(session))
                    })
                })
                .await?
            }
            CpdlcMetaMessage::ConnectionResponse { accepted } => {
                // ConnectionResponse is sent by the aircraft back to the ATC station.
                // The station that initiated the connection is the *destination* of this
                // response (not the source, which is the aircraft).
                let dest_callsign = cpdlc.destination.clone();
                info!(aircraft = ?aircraft, accepted, dest = %dest_callsign, "processing connection response");
                if accepted {
                    self.get_and_update_session_for_aircraft(&aircraft, |maybe_session: Option<CPDLCSession>| {
                        let aircraft = aircraft.clone();
                        Box::pin(async move {
                            let mut session =
                                maybe_session.unwrap_or_else(|| CPDLCSession::new(aircraft));
                            session.connection_accepted(&dest_callsign)?;
                            Ok(Some(session))
                        })
                    })
                    .await?
                } else {
                    None
                }
            }
            CpdlcMetaMessage::SessionUpdate { .. } => {
                // SessionUpdate is server-originated — ignore if received from a client.
                warn!("ignoring client-sent SessionUpdate");
                None
            }
            CpdlcMetaMessage::NextDataAuthority { nda } => {
                info!(aircraft = ?aircraft, nda = ?nda, "processing next data authority");
                self.get_and_update_session_for_aircraft(&aircraft, |maybe_session: Option<CPDLCSession>| {
                    let aircraft = aircraft.clone();
                    Box::pin(async move {
                        let mut session =
                            maybe_session.unwrap_or_else(|| CPDLCSession::new(aircraft));
                        session.next_data_authority(nda)?;
                        Ok(Some(session))
                    })
                })
                .await?
            }
            CpdlcMetaMessage::ContactRequest { station } => {
                info!(aircraft = ?aircraft, station = ?station, "processing contact request");
                // ContactRequest is forwarded to the aircraft — no session change here.
                // The aircraft will initiate a logon to the new station.
                None
            }
            CpdlcMetaMessage::EndService => {
                let source_callsign = cpdlc.source.clone();
                info!(aircraft = ?aircraft, source = %source_callsign, "processing end service");
                self.get_and_update_session_for_aircraft(&aircraft, |maybe_session: Option<CPDLCSession>| {
                    let aircraft = aircraft.clone();
                    Box::pin(async move {
                        let mut session =
                            maybe_session.unwrap_or_else(|| CPDLCSession::new(aircraft));
                        session.termination_request(&source_callsign)?;
                        Ok(Some(session))
                    })
                })
                .await?
            }
            CpdlcMetaMessage::LogonForward { flight, new_station, .. } => {
                info!(aircraft = ?aircraft, flight = ?flight, new_station = ?new_station, "processing logon forward");
                // LogonForward is a station-to-station message: route to the new station.
                // The new station should then send a ConnectionRequest to the aircraft.
                None
            }
            CpdlcMetaMessage::ContactResponse { .. } | CpdlcMetaMessage::ContactComplete => {
                // Forward as-is to the destination.
                None
            }
        };

        // Return the destination callsign for routing — the caller will
        // resolve it to a network address via the station registry.
        Ok((cpdlc.destination.clone(), updated_session))
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

    /// Return all sessions relevant to a participant callsign.
    ///
    /// A session is considered relevant if the callsign is either:
    /// - the aircraft callsign owning the session,
    /// - the active station callsign,
    /// - the inactive station callsign.
    pub async fn list_sessions_for_callsign(
        &self,
        callsign: &AcarsEndpointCallsign,
    ) -> Result<Vec<CPDLCSession>> {
        let mut keys = self.kv_sessions_store.keys().await?;
        let mut sessions = Vec::new();

        while let Some(key) = keys.try_next().await? {
            if let Some(content) = self.kv_sessions_store.get(&key).await? {
                let session: CPDLCSession = serde_json::from_slice(content.as_ref())?;
                let relevant = session.aircraft.callsign == *callsign
                    || session
                        .active_connection
                        .as_ref()
                        .is_some_and(|c| c.station.callsign == *callsign)
                    || session
                        .inactive_connection
                        .as_ref()
                        .is_some_and(|c| c.station.callsign == *callsign);
                if relevant {
                    sessions.push(session);
                }
            }
        }

        Ok(sessions)
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

        let _ = session.logon_accepted(&station1.callsign);
        assert!(session.active_connection.as_ref().unwrap().logon);
        assert!(!session.active_connection.as_ref().unwrap().ready_exchange());

        let _ = session.connection_request(&station1.callsign);
        assert!(!session.active_connection.as_ref().unwrap().ready_exchange());

        let _ = session.connection_accepted(&station1.callsign);
        assert!(session.active_connection.as_ref().unwrap().ready_exchange());

        let _ = session.termination_request(&station1.callsign);
        assert!(session.active_connection.is_none());
    }

    #[test]
    fn test_cpdlc_session_switch() {
        let mut session = CPDLCSession::new(AcarsRoutingEndpoint::new("TEST123", "abc"));
        let station1 = AcarsRoutingEndpoint::new("STATION1", "def");
        let station2 = AcarsRoutingEndpoint::new("STATION2", "ghi");

        let _ = session.logon_request(station1.clone());
        let _ = session.logon_accepted(&station1.callsign);
        let _ = session.connection_request(&station1.callsign);
        let _ = session.connection_accepted(&station1.callsign);

        let _ = session.logon_request(station2.clone());
        let _ = session.logon_accepted(&station2.callsign);
        let _ = session.connection_request(&station2.callsign);
        let _ = session.connection_accepted(&station2.callsign);

        assert!(session.active_connection.as_ref().unwrap().ready_exchange());
        assert!(session.inactive_connection.as_ref().unwrap().ready_exchange());

        let _ = session.termination_request(&station1.callsign);
        assert!(session.active_connection.as_ref().unwrap().station == station2);
        assert!(session.inactive_connection.is_none());
    }

    #[test]
    fn test_cpdlc_session_without_logon() {
        let mut session = CPDLCSession::new(AcarsRoutingEndpoint::new("TEST123", "abc"));
        let station1 = AcarsRoutingEndpoint::new("STATION1", "def");

        let _ = session.logon_accepted(&station1.callsign);
        assert!(session.active_connection.is_none());

        let _ = session.connection_request(&station1.callsign);
        assert!(session.active_connection.is_none());
        let _ = session.connection_accepted(&station1.callsign);
        assert!(session.active_connection.is_none());
    }

    #[test]
    fn test_cpdlc_session_with_nda() {
        let mut session = CPDLCSession::new(AcarsRoutingEndpoint::new("TEST123", "abc"));
        let station1 = AcarsRoutingEndpoint::new("STATION1", "def");

        let _ = session.next_data_authority(station1.clone());

        let _ = session.connection_request(&station1.callsign);
        assert_eq!(session.active_connection.as_ref().unwrap().station.callsign, station1.callsign);

        let _ = session.connection_accepted(&station1.callsign);
        assert_eq!(session.active_connection.as_ref().unwrap().station.callsign, station1.callsign);
    }

    #[test]
    fn test_cpdlc_session_with_nda_transfer() {
        let mut session = CPDLCSession::new(AcarsRoutingEndpoint::new("TEST123", "abc"));
        let station1 = AcarsRoutingEndpoint::new("STATION1", "def");
        let station2 = AcarsRoutingEndpoint::new("STATION2", "ghi");

        let _ = session.logon_request(station1.clone());
        let _ = session.logon_accepted(&station1.callsign);
        let _ = session.connection_request(&station1.callsign);
        let _ = session.connection_accepted(&station1.callsign);
        let _ = session.next_data_authority(station2.clone());
        let _ = session.connection_request(&station2.callsign);
        let _ = session.connection_accepted(&station2.callsign);
        assert_eq!(session.active_connection.as_ref().unwrap().station, station1);
        assert_eq!(session.inactive_connection.as_ref().unwrap().station.callsign, station2.callsign);
        let _ = session.termination_request(&station1.callsign);
        assert_eq!(session.active_connection.as_ref().unwrap().station.callsign, station2.callsign);
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