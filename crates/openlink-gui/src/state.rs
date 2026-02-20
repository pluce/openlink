use chrono::{DateTime, Utc};
use openlink_models::{AcarsEndpointAddress, CpdlcConnectionPhase, CpdlcSessionView, MessageElement, OpenLinkEnvelope, ResponseAttribute};
use openlink_sdk::OpenLinkClient;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashMap;

// ── Saved station presets ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StationType {
    Aircraft,
    Atc,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SavedStation {
    pub station_type: StationType,
    pub network_id: String,
    pub network_address: String,
    pub callsign: String,
    pub acars_address: String,
}

// ── Per-station (ATC) tracking ────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct AtcLinkedFlight {
    pub callsign: String,
    pub aircraft_callsign: String,
    pub aircraft_address: AcarsEndpointAddress,
    pub phase: CpdlcConnectionPhase,
}

// ── Received message wrapper ──────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ReceivedMessage {
    pub timestamp: DateTime<Utc>,
    pub raw_json: String,
    pub envelope: Option<OpenLinkEnvelope>,
    pub from_callsign: Option<String>,
    /// Target callsign (who the message is addressed to).
    pub to_callsign: Option<String>,
    /// Human-readable serialized message text (from SerializedMessagePayload)
    pub display_text: Option<String>,
    /// True if this message was sent by us (outgoing), false for received
    pub is_outgoing: bool,
    /// Message Identification Number (0–63) assigned by the server.
    pub min: Option<u8>,
    /// Message Reference Number — the MIN of the message this replies to.
    pub mrn: Option<u8>,
    /// Effective response attribute for this message.
    pub response_attr: Option<ResponseAttribute>,
    /// Whether a closing response (WILCO/UNABLE/etc.) has been sent to this message.
    pub responded: bool,
}

// ── Tab state ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum TabPhase {
    Setup,
    Connected(StationType),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TabState {
    pub id: Uuid,
    pub label: String,
    pub phase: TabPhase,

    // Setup fields
    pub setup: SetupFields,

    // Server-authoritative CPDLC session state
    pub session: Option<CpdlcSessionView>,

    // DCDU state
    pub logon_input: String,

    // ATC state (server-authoritative snapshots keyed by aircraft callsign)
    pub atc_sessions: HashMap<String, CpdlcSessionView>,
    pub selected_flight_idx: Option<usize>,
    pub contact_input: String,
    pub conn_mgmt_open: bool,
    /// Which connection management action is being parameterized (e.g. "CONTACT")
    pub pending_conn_mgmt_cmd: Option<String>,
    pub atc_uplink_open: bool,

    // Pilot state
    pub pilot_downlink_open: bool,
    /// Generic argument inputs for currently parameterized command.
    pub cmd_arg_inputs: Vec<String>,
    /// Search query used in command menus (uplink/downlink).
    pub cmd_search_query: String,
    /// Enable multi-element compose workflow.
    pub compose_mode: bool,
    /// Elements queued for the next composed application message.
    pub compose_elements: Vec<MessageElement>,
    /// Optional MRN used when composing a response with additional elements.
    pub compose_mrn: Option<u8>,
    /// In compose context, submitting the current parameter form sends all.
    pub compose_send_after_param: bool,
    /// Which downlink command is being parameterized (e.g. "DM6") — None = show menu
    pub pending_downlink_cmd: Option<String>,
    /// Which uplink command is being parameterized (e.g. "UM20") — None = show menu
    pub pending_uplink_cmd: Option<String>,
    /// Optional restriction set for uplink menu (suggested replies context).
    pub suggested_uplink_ids: Vec<String>,

    // Common
    pub messages: Vec<ReceivedMessage>,

    // Runtime (not cloneable — we use a channel id to communicate)
    pub nats_task_active: bool,
}

/// Holds the NATS clients for each connected tab (keyed by tab UUID).
/// Stored separately because OpenLinkClient is not Clone-friendly with signals.
#[derive(Clone, Default)]
pub struct NatsClients {
    pub clients: HashMap<Uuid, OpenLinkClient>,
}

impl NatsClients {
    pub fn insert(&mut self, id: Uuid, client: OpenLinkClient) {
        self.clients.insert(id, client);
    }

    pub fn get(&self, id: &Uuid) -> Option<&OpenLinkClient> {
        self.clients.get(id)
    }

    pub fn remove(&mut self, id: &Uuid) {
        self.clients.remove(id);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetupFields {
    pub station_type: StationType,
    pub network_id: String,
    pub network_address: String,
    pub callsign: String,
    pub acars_address: String,
}

impl Default for SetupFields {
    fn default() -> Self {
        Self {
            station_type: StationType::Aircraft,
            network_id: "demonetwork".to_string(),
            network_address: String::new(),
            callsign: String::new(),
            acars_address: String::new(),
        }
    }
}

impl TabState {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            label: "New".to_string(),
            phase: TabPhase::Setup,
            setup: SetupFields::default(),
            session: None,
            logon_input: String::new(),
            atc_sessions: HashMap::new(),
            selected_flight_idx: None,
            contact_input: String::new(),
            conn_mgmt_open: false,
            pending_conn_mgmt_cmd: None,
            atc_uplink_open: false,
            pilot_downlink_open: false,
            cmd_arg_inputs: Vec::new(),
            cmd_search_query: String::new(),
            compose_mode: false,
            compose_elements: Vec::new(),
            compose_mrn: None,
            compose_send_after_param: false,
            pending_downlink_cmd: None,
            pending_uplink_cmd: None,
            suggested_uplink_ids: Vec::new(),
            messages: Vec::new(),
            nats_task_active: false,
        }
    }
}

// ── Global app state ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct AppState {
    pub tabs: Vec<TabState>,
    pub active_tab: usize,
    pub saved_stations: Vec<SavedStation>,
}

impl AppState {
    pub fn new() -> Self {
        let saved_stations = crate::persistence::load_saved_stations();
        let mut s = Self {
            tabs: Vec::new(),
            active_tab: 0,
            saved_stations,
        };
        s.add_tab();
        s
    }

    pub fn add_tab(&mut self) {
        self.tabs.push(TabState::new());
        self.active_tab = self.tabs.len() - 1;
    }

    pub fn close_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.tabs.remove(idx);
            if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_tab = self.tabs.len() - 1;
            }
        }
    }

    pub fn tab_mut_by_id(&mut self, id: Uuid) -> Option<&mut TabState> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn tab_by_id(&self, id: Uuid) -> Option<&TabState> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn save_station(&mut self, station: SavedStation) {
        // Avoid duplicates
        if !self.saved_stations.iter().any(|s| {
            s.callsign == station.callsign
                && s.network_id == station.network_id
                && s.network_address == station.network_address
        }) {
            self.saved_stations.push(station);
            crate::persistence::save_saved_stations(&self.saved_stations);
        }
    }
}
