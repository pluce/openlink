use chrono::{DateTime, Utc};
use openlink_models::{
    AcarsEndpointAddress, CpdlcConnectionPhase, CpdlcSessionView, MessageElement,
    OpenLinkEnvelope, ResponseAttribute,
};
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

// ── Connection status for DCDU ────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum GroundStationStatus {
    /// Not connected to any ground station
    Disconnected,
    /// Logon request sent, waiting for response
    LogonPending(String),
    /// Logon accepted, connection request received + confirmed
    Connected(String),
}

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
    /// Optional destination callsign for outgoing messages.
    pub to_callsign: Option<String>,
    /// Human-readable serialized message text (from SerializedMessagePayload)
    pub display_text: Option<String>,
    /// Whether this message is locally-originated.
    pub is_outgoing: bool,
    /// Server-assigned Message Identification Number.
    pub min: Option<u8>,
    /// Message Reference Number (links a response to a prior MIN).
    pub mrn: Option<u8>,
    /// Effective CPDLC response attribute for incoming application messages.
    pub response_attr: Option<ResponseAttribute>,
    /// Whether a dialogue has been closed from the UI perspective.
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

    // DCDU state
    pub ground_station: GroundStationStatus,
    pub logon_input: String,
    pub session: Option<CpdlcSessionView>,
    pub pilot_downlink_open: bool,
    pub pending_downlink_cmd: Option<String>,

    // ATC state
    pub linked_flights: Vec<AtcLinkedFlight>,
    pub selected_flight_idx: Option<usize>,
    pub atc_sessions: HashMap<String, CpdlcSessionView>,
    pub conn_mgmt_open: bool,
    pub atc_uplink_open: bool,
    pub contact_input: String,
    pub pending_conn_mgmt_cmd: Option<String>,
    pub pending_uplink_cmd: Option<String>,
    pub suggested_uplink_ids: Vec<String>,

    // Generic command/composition state (used by both DCDU and ATC views)
    pub cmd_arg_inputs: Vec<String>,
    pub cmd_search_query: String,
    pub compose_mode: bool,
    pub compose_elements: Vec<MessageElement>,
    pub compose_mrn: Option<u8>,
    pub compose_send_after_param: bool,

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
            network_id: "vatsim".to_string(),
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
            ground_station: GroundStationStatus::Disconnected,
            logon_input: String::new(),
            session: None,
            pilot_downlink_open: false,
            pending_downlink_cmd: None,
            linked_flights: Vec::new(),
            selected_flight_idx: None,
            atc_sessions: HashMap::new(),
            conn_mgmt_open: false,
            atc_uplink_open: false,
            contact_input: String::new(),
            pending_conn_mgmt_cmd: None,
            pending_uplink_cmd: None,
            suggested_uplink_ids: Vec::new(),
            cmd_arg_inputs: Vec::new(),
            cmd_search_query: String::new(),
            compose_mode: false,
            compose_elements: Vec::new(),
            compose_mrn: None,
            compose_send_after_param: false,
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
