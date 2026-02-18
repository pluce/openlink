use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Clear},
    Frame,
};
use openlink_models::{OpenLinkEnvelope, OpenLinkEnvelopeMeta, OpenLinkEnvelopeRouting, OpenLinkEnvelopeRoutingNetwork, CpdlcConnectionRequest, CpdlcNextDataAuthority, CpdlcLogonResponse, CpdlcTransferRequest, CpdlcContactRequest, CpdlcMessage};
use crate::app_state::{AppController, ChatMessage, InputMode};
use crate::tui::Action;
use openlink_sdk::OpenLinkClient;
use std::collections::HashMap;
use crossterm::event::KeyCode;
use chrono::Utc;
use uuid::Uuid;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone, PartialEq, Debug)]
enum AtcConnectionState {
    Unknown,
    LogonReceived,
    LogonAccepted,
    ConnectionRequested,
    Connected,
    HandoffInitiated(String), // To whom
    TransferReceived(String), // From whom
}

#[derive(Clone)]
struct AtcCommand {
    label: String,
    action: AtcCommandAction,
}

#[derive(Clone)]
enum AtcCommandAction {
    AcceptLogon,
    RejectLogon,
    RequestConnection,
    PromptHandoff, // Ask for target (NDA)
    PromptTransfer, // Ask for target (Transfer Request)
    AcceptTransfer(String), // Accept transfer from Source ATC
    PromptContact, // New: Contact Request (Disconnected Handoff)
    SendNdaContactRequest(String), // Send Contact Request to specific target (NDA)
    SendMessage, // Prompt for text
    Terminate,
    UplinkResponse(String), // UNABLE, STANDBY, ROGER
    UplinkInstructionWithArg { template: String, prompt: String }, // CLIMB TO {}, DESCEND TO {}
}

#[derive(PartialEq)]
enum ActivePane {
    Flights,
    Commands,
}

pub struct AtcApp {
    station_name: String,
    network_id: String,
    client: OpenLinkClient,
    tx: UnboundedSender<Action>,
    should_quit: bool,
    
    // UI State
    input: String,
    input_mode: InputMode,
    active_pane: ActivePane,
    
    flights: Vec<String>,
    states: HashMap<String, AtcConnectionState>, // Key: Pilot CID
    
    list_state: ListState,     // For Flights
    command_state: ListState,  // For Commands
    
    // Data
    messages: HashMap<String, Vec<ChatMessage>>, // Key: Pilot Network ID (CID)
    flight_callsigns: HashMap<String, String>,   // Key: Pilot CID, Value: Callsign
    
    // Messaging State
    min_counters: HashMap<String, u8>, // Key: Pilot CID, Value: Next MIN to send
    last_received_mins: HashMap<String, u8>, // Key: Pilot CID, Value: Last received MIN

    // Notifications
    notification: Option<(String, std::time::Instant)>,
    
    // Pending Command State
    pending_cmd: Option<AtcCommandAction>, // If we are typing an argument for a command
}

impl AtcApp {
    pub fn new(station_name: String, client: OpenLinkClient, tx: UnboundedSender<Action>) -> Self {
        let network_id = client.get_cid();
        
        let mut app = Self {
            station_name,
            network_id,
            client,
            tx,
            should_quit: false,
            input: String::new(),
            input_mode: InputMode::Normal,
            active_pane: ActivePane::Flights,
            flights: Vec::new(),
            states: HashMap::new(),
            list_state: ListState::default(),
            command_state: ListState::default(),
            messages: HashMap::new(),
            flight_callsigns: HashMap::new(),
            notification: None,
            pending_cmd: None,
            min_counters: HashMap::new(),
            last_received_mins: HashMap::new(),
        };
        app.list_state.select(Some(0));
        app
    }
    
    pub async fn listen(&self) {
        let client = self.client.clone();
        let my_cid = self.network_id.clone();
        let tx = self.tx.clone();
        
        // Subscribe using the Network ID (CID)
        let subject = format!("cpdlc.request.{}", my_cid);
        
        tokio::spawn(async move {
            if let Ok(mut sub) = client.subscribe(&subject).await {
                while let Some(msg) = futures::StreamExt::next(&mut sub).await {
                    if let Ok(env) = serde_json::from_slice::<OpenLinkEnvelope>(&msg.payload) {
                         // let _log = format!("MSG from {}: {}", env.routing.source, env.type_);
                         let _ = tx.send(Action::MessageReceived(serde_json::to_string(&env).unwrap()));
                    }
                }
            }
        });
    }

    fn handle_incoming_envelope(&mut self, env: OpenLinkEnvelope) {
        // Prevent echo/duplication of own messages
        if env.routing.source == self.network_id {
            return;
        }

        let source_cid = env.routing.source.clone();
        
        // Try to extract callsign if it's a Logon
        if env.type_ == "cpdlc.logon.request" {
            if let Ok(val) = serde_json::to_value(&env.payload) {
                if let Some(callsign) = val.get("callsign").and_then(|v| v.as_str()) {
                    self.flight_callsigns.insert(source_cid.clone(), callsign.to_string());
                }
            }
            self.states.insert(source_cid.clone(), AtcConnectionState::LogonReceived);
        } else if env.type_ == "cpdlc.message" {
            if let Ok(val) = serde_json::to_value(&env.payload) {
                // Track MIN
                 if let Some(min) = val.get("min").and_then(|v| v.as_i64()) {
                     self.last_received_mins.insert(source_cid.clone(), min as u8);
                 }
            }
        } else if env.type_ == "cpdlc.connection.confirm" {
            self.states.insert(source_cid.clone(), AtcConnectionState::Connected);
        } else if env.type_ == "cpdlc.transfer.response" {
             let acc = env.payload.get("accepted").and_then(|v| v.as_bool()).unwrap_or(false);
             let msg = format!("TRANSFER {}", if acc { "ACCEPTED" } else { "REJECTED" });
             self.messages.entry(source_cid.clone()).or_default().push(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
                source: source_cid.clone(),
                content: msg,
                timestamp: Utc::now().format("%H:%M:%S").to_string(),
                is_incoming: true,
            });
             return;
        } else if env.type_ == "cpdlc.contact.response" {
             let accepted = env.payload.get("accepted").and_then(|v| v.as_bool()).unwrap_or(false);
             let msg = format!("CONTACT {}", if accepted { "WILCO" } else { "UNABLE" });
             self.messages.entry(source_cid.clone()).or_default().push(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
                source: source_cid.clone(),
                content: msg,
                timestamp: Utc::now().format("%H:%M:%S").to_string(),
                is_incoming: true,
            });
            return;
        } else if env.type_ == "cpdlc.contact.complete" {
             self.messages.entry(source_cid.clone()).or_default().push(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
                source: source_cid.clone(),
                content: "CONTACT COMPLETE".to_string(),
                timestamp: Utc::now().format("%H:%M:%S").to_string(),
                is_incoming: true,
            });
            
            // Auto-send Termination in response to Contact Complete
            let env = OpenLinkEnvelope {
                 meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
                 routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: source_cid.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
                 type_: "cpdlc.termination.request".to_string(),
                 payload: std::collections::HashMap::new(),
            };
            let client = self.client.clone();
            tokio::spawn(async move {
                let _ = client.publish_envelope("cpdlc.session.control", &env).await;
            });

            self.messages.entry(source_cid.clone()).or_default().push(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
                source: "ME".to_string(),
                content: "TERMINATION SENT (AUTO)".to_string(),
                timestamp: Utc::now().format("%H:%M:%S").to_string(),
                is_incoming: false,
            });

            // Cleanup
            self.states.remove(&source_cid);
            if let Some(idx) = self.flights.iter().position(|r| *r == source_cid) {
                self.flights.remove(idx);
                // Adjust selection if needed
                if self.list_state.selected() == Some(idx) {
                    self.list_state.select(None); // Simplified
                }
            }
            self.show_notification(format!("{} Disconnected (Handover Complete)", source_cid));
            return;
        } else if env.type_ == "cpdlc.transfer.request" {
             if let Ok(val) = serde_json::to_value(&env.payload) {
                if let Some(callsign) = val.get("callsign").and_then(|v| v.as_str()) {
                    // Extract Pilot CID if available (inserted by sender), else fallback to callsign
                    let real_cid = val.get("pilot_cid").and_then(|v| v.as_str()).unwrap_or(callsign).to_string();

                    self.flight_callsigns.insert(real_cid.clone(), callsign.to_string());
                    self.states.insert(real_cid.clone(), AtcConnectionState::TransferReceived(source_cid.clone()));

                    if !self.flights.contains(&real_cid) {
                        self.flights.push(real_cid.clone());
                    }
                    
                    self.messages.entry(real_cid.clone()).or_default().push(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
                        source: source_cid.clone(),
                        content: format!("TRANSFER REQUEST from {}", source_cid),
                        timestamp: Utc::now().format("%H:%M:%S").to_string(),
                        is_incoming: true,
                    });
                    self.show_notification(format!("Transfer Request for {} from {}", callsign, source_cid));
                    return; 
                }
             }
        }
        
        let display_name = self.flight_callsigns.get(&source_cid).cloned().unwrap_or_else(|| source_cid.clone());

        // Add to flight list if new
        if !self.flights.contains(&source_cid) {
            self.flights.push(source_cid.clone());
            if self.flights.len() == 1 {
                self.list_state.select(Some(0));
            }
            self.states.entry(source_cid.clone()).or_insert(AtcConnectionState::Unknown);
        }
        
        let msg_display = match env.type_.as_str() {
            "cpdlc.logon.request" => "LOGON REQUEST".to_string(),
            "cpdlc.connection.confirm" => "CONNECTION CONFIRMED".to_string(),
            _ => format!("REQ: {}", env.type_),
        };

        // Parse specific fields for the message logic
        let mut min = None;
        let mut mrn = None;
        let mut requires_response = false;
        let mut response_attribute = None;

        if let Ok(val) = serde_json::to_value(&env.payload) {
             if let Some(m) = val.get("min").and_then(|v| v.as_i64()) {
                 min = Some(m as u8);
             }
             if let Some(m) = val.get("mrn").and_then(|v| v.as_i64()) {
                 mrn = Some(m as u8);
             }
             
             // Check elements for priority using our helper
             if let Ok(msg_obj) = serde_json::from_value::<openlink_models::CpdlcMessage>(val.clone()) {
                 let elements = msg_obj.elements.as_ref().unwrap_or(&vec![]);
                 let (req, attr) = ChatMessage::calculate_priority(elements);
                 requires_response = req;
                 response_attribute = attr;
             }
             // For Level Change Request -> Implicit W/U
             if env.type_ == "cpdlc.level_change_request" {
                 requires_response = true;
                 response_attribute = Some("W/U".to_string());
             }
        }
        
        let chat_msg = ChatMessage {
            min,
            mrn,
            requires_response,
            response_attribute,
            source: display_name.clone(),
            content: msg_display,
            timestamp: Utc::now().format("%H:%M:%S").to_string(),
            is_incoming: true,
        };
        
        self.messages.entry(source_cid.clone()).or_default().push(chat_msg);
        self.show_notification(format!("New message from {}", display_name));
    }

    fn show_notification(&mut self, msg: String) {
        self.notification = Some((msg, std::time::Instant::now()));
    }
    
    fn get_selected_pilot_cid(&self) -> Option<String> {
        self.list_state.selected().and_then(|i| self.flights.get(i).cloned())
    }
    
    fn get_commands_for_flight(&self, cid: &str) -> Vec<AtcCommand> {
        let state = self.states.get(cid).unwrap_or(&AtcConnectionState::Unknown);
        match state {
            AtcConnectionState::Unknown => vec![],
            AtcConnectionState::LogonReceived => vec![
                AtcCommand { label: "ACCEPT LOGON".to_string(), action: AtcCommandAction::AcceptLogon },
                AtcCommand { label: "REJECT LOGON".to_string(), action: AtcCommandAction::RejectLogon },
            ],
            AtcConnectionState::LogonAccepted => vec![
                AtcCommand { label: "REQUEST CONNECTION".to_string(), action: AtcCommandAction::RequestConnection },
            ],
            AtcConnectionState::ConnectionRequested => vec![
                 AtcCommand { label: "(Waiting for Confirm...)".to_string(), action: AtcCommandAction::SendMessage },
            ],
            AtcConnectionState::Connected => vec![
                AtcCommand { label: "CLIMB TO [LEVEL]".to_string(), action: AtcCommandAction::UplinkInstructionWithArg { template: "CLIMB TO {}".to_string(), prompt: "Enter Level (e.g. FL350):".to_string() } },
                AtcCommand { label: "DESCEND TO [LEVEL]".to_string(), action: AtcCommandAction::UplinkInstructionWithArg { template: "DESCEND TO {}".to_string(), prompt: "Enter Level (e.g. FL100):".to_string() } },
                AtcCommand { label: "UNABLE (Respond)".to_string(), action: AtcCommandAction::UplinkResponse("UNABLE".to_string()) },
                AtcCommand { label: "STANDBY (Respond)".to_string(), action: AtcCommandAction::UplinkResponse("STANDBY".to_string()) },
                AtcCommand { label: "ROGER (Respond)".to_string(), action: AtcCommandAction::UplinkResponse("ROGER".to_string()) },
                AtcCommand { label: "SEND HANDOFF (NDA)".to_string(), action: AtcCommandAction::PromptHandoff },
                AtcCommand { label: "SEND CONTACT (DISC)".to_string(), action: AtcCommandAction::PromptContact },
                AtcCommand { label: "TRANSFER TO ATSU".to_string(), action: AtcCommandAction::PromptTransfer },
                AtcCommand { label: "SEND TEXT MESSAGE".to_string(), action: AtcCommandAction::SendMessage },
                AtcCommand { label: "TERMINATE SERVICE".to_string(), action: AtcCommandAction::Terminate },
            ],
            AtcConnectionState::HandoffInitiated(target) => vec![
                AtcCommand { label: format!("SEND NDA CONTACT REQUEST ({})", target), action: AtcCommandAction::SendNdaContactRequest(target.clone()) },
                AtcCommand { label: "SEND TEXT MESSAGE".to_string(), action: AtcCommandAction::SendMessage },
                AtcCommand { label: format!("CANCEL HANDOFF TO {}", target), action: AtcCommandAction::Terminate }, // Placeholder
                AtcCommand { label: "TERMINATE SERVICE".to_string(), action: AtcCommandAction::Terminate },
            ],
            AtcConnectionState::TransferReceived(from) => vec![
                AtcCommand { label: format!("ACCEPT TRANSFER FROM {}", from), action: AtcCommandAction::AcceptTransfer(from.clone()) },
                AtcCommand { label: "REJECT TRANSFER".to_string(), action: AtcCommandAction::RejectLogon }, // Reuse reject
            ],
        }
    }

    fn send_logon_response(&mut self, accepted: bool) {
        if let Some(pilot_cid) = self.get_selected_pilot_cid() {
            let req = CpdlcLogonResponse {
                accepted,
                info_message: if accepted { Some("Logon Accepted".to_string()) } else { Some("Logon Rejected".to_string()) },
            };
            let payload = serde_json::to_value(req).unwrap().as_object().unwrap().clone().into_iter().collect();
            
            let env = OpenLinkEnvelope {
                 meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
                 routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: pilot_cid.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
                 type_: "cpdlc.logon.response".to_string(),
                 payload,
            };
            
            let client = self.client.clone();
            
            tokio::spawn(async move {
                let _ = client.publish_envelope("cpdlc.session.logon_response", &env).await;
            });
            
            let state = if accepted { AtcConnectionState::LogonAccepted } else { AtcConnectionState::Unknown };
            self.states.insert(pilot_cid.clone(), state);

            self.messages.entry(pilot_cid.clone()).or_default().push(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
                source: "ME".to_string(),
                content: format!("LOGON RESPONSE ({}) -> {}", if accepted { "ACC" } else { "REJ" }, pilot_cid),
                timestamp: Utc::now().format("%H:%M:%S").to_string(),
                is_incoming: false,
            });
            self.show_notification(format!("Logon Response sent to {}", pilot_cid));
        }
    }

    fn send_connect(&mut self) {
        if let Some(pilot_cid) = self.get_selected_pilot_cid() {
            let req = CpdlcConnectionRequest {
                facility_name: self.station_name.clone(),
                logon_data: None,
            };
            let payload = serde_json::to_value(req).unwrap().as_object().unwrap().clone().into_iter().collect();
            
            let env = OpenLinkEnvelope {
                 meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
                 routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: pilot_cid.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
                 type_: "cpdlc.connection.request".to_string(),
                 payload,
            };
            
            let client = self.client.clone();
            
            tokio::spawn(async move {
                let _ = client.publish_envelope("cpdlc.session.connect", &env).await;
            });
            
            self.states.insert(pilot_cid.clone(), AtcConnectionState::ConnectionRequested);

            self.messages.entry(pilot_cid.clone()).or_default().push(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
                source: "ME".to_string(),
                content: format!("SENT CONNECTION REQUEST -> {}", pilot_cid),
                timestamp: Utc::now().format("%H:%M:%S").to_string(),
                is_incoming: false,
            });
            self.show_notification(format!("Connection Request sent to {}", pilot_cid));
        }
    }
    
    fn send_handoff(&mut self, next_atc: String) {
        if let Some(pilot_cid) = self.get_selected_pilot_cid() {
             let req = CpdlcNextDataAuthority {
                 next_authority: next_atc.clone(),
             };
             let payload = serde_json::to_value(req).unwrap().as_object().unwrap().clone().into_iter().collect();
             
             let env = OpenLinkEnvelope {
                 meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
                 routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: pilot_cid.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
                 type_: "cpdlc.next_data_authority".to_string(),
                 payload,
             };
             
             let client = self.client.clone();
             let tx = self.tx.clone();
             tokio::spawn(async move {
                 if let Err(e) = client.publish_envelope("cpdlc.session.nda", &env).await {
                     let _ = tx.send(Action::Error(format!("NDA Failed: {}", e)));
                 }
            });
            
            self.states.insert(pilot_cid.clone(), AtcConnectionState::HandoffInitiated(next_atc.clone()));

            self.messages.entry(pilot_cid.clone()).or_default().push(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
                source: "ME".to_string(),
                content: format!("HANDOFF -> {}", next_atc),
                timestamp: Utc::now().format("%H:%M:%S").to_string(),
                is_incoming: false,
            });
             self.show_notification(format!("Handoff sent to {}", pilot_cid));
        }
    }

    fn send_transfer_request(&mut self, target_atc: String) {
        if let Some(pilot_cid) = self.get_selected_pilot_cid() {
            // 1. Set NDA on Aircraft (Pre-requisite for Transfer)
            self.send_handoff(target_atc.clone());

            // 2. Send Transfer Request to Target ATC (Logon Forwarding)
            let req = CpdlcTransferRequest {
                 callsign: self.flight_callsigns.get(&pilot_cid).cloned().unwrap_or(pilot_cid.clone()),
                 aircraft_type: "UNKNOWN".to_string(), // In real app would come from Flight Plan
                 origin: "UNKNOWN".to_string(),
                 destination: "UNKNOWN".to_string(),
                 current_level: "FL350".to_string(),
                 assigned_level: "FL350".to_string(),
            };
            
            // Serialize to Object Map and inject the Pilot CID (Network ID) which is crucial for the receiver to session-match
            let mut payload_map = serde_json::to_value(req).unwrap().as_object().unwrap().clone();
            payload_map.insert("pilot_cid".to_string(), serde_json::Value::String(pilot_cid.clone()));

            let payload: HashMap<String, serde_json::Value> = payload_map.into_iter().collect();
            
            let env = OpenLinkEnvelope {
                 meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
                 // Routing: Target is the OTHER ATC
                 routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: target_atc.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
                 type_: "cpdlc.transfer.request".to_string(),
                 payload,
            };
            
             let client = self.client.clone();
             tokio::spawn(async move {
                 let _ = client.publish_envelope("cpdlc.session.transfer_req", &env).await;
            });
            
             self.messages.entry(pilot_cid.clone()).or_default().push(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
                source: "ME".to_string(),
                content: format!("TRANSFER REQUEST -> {}", target_atc),
                timestamp: Utc::now().format("%H:%M:%S").to_string(),
                is_incoming: false,
            });
        }
    }

    fn send_contact_request(&mut self, next_atc: String) {
        if let Some(pilot_cid) = self.get_selected_pilot_cid() {
            let req = CpdlcContactRequest {
                 facility: next_atc.clone(),
                 frequency: Some("123.450".to_string()),
            };
            let payload = serde_json::to_value(req).unwrap().as_object().unwrap().clone().into_iter().collect();
            
            let env = OpenLinkEnvelope {
                 meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
                 routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: pilot_cid.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
                 type_: "cpdlc.contact.request".to_string(),
                 payload,
            };
            let client = self.client.clone();
            let tx = self.tx.clone();
            tokio::spawn(async move {
                 if let Err(e) = client.publish_envelope("cpdlc.session.contact", &env).await {
                     let _ = tx.send(Action::Error(format!("Contact Req Failed: {}", e)));
                 }
            });

            self.messages.entry(pilot_cid.clone()).or_default().push(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
                source: "ME".to_string(),
                content: format!("CONTACT REQUEST -> {}", next_atc),
                timestamp: Utc::now().format("%H:%M:%S").to_string(),
                is_incoming: false,
            });
            self.show_notification(format!("Contact Request sent to {}", pilot_cid));
        }
    }
    
    fn send_transfer_accept(&mut self, source_atc: String) {
        if let Some(pilot_cid) = self.get_selected_pilot_cid() {
            // 1. Send Response to Source ATC
             let req = openlink_models::CpdlcTransferResponse {
                 accepted: true,
                 reason: None,
            };
            let payload = serde_json::to_value(req).unwrap().as_object().unwrap().clone().into_iter().collect();
            
            let env = OpenLinkEnvelope {
                 meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
                 routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: source_atc.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
                 type_: "cpdlc.transfer.response".to_string(),
                 payload,
            };
            
            let client = self.client.clone();
            tokio::spawn(async move {
                let _ = client.publish_envelope("cpdlc.session.transfer_res", &env).await;
            });
            
             self.messages.entry(pilot_cid.clone()).or_default().push(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
                source: "ME".to_string(),
                content: format!("TRANSFER ACCEPTED -> {}", source_atc),
                timestamp: Utc::now().format("%H:%M:%S").to_string(),
                is_incoming: false,
            });
            
            // 2. Initiate Connection with Pilot
            self.send_connect();
        }
    }
    
    fn send_uplink(&mut self, instruction: &str, arg: Option<String>) {
        if let Some(pilot_cid) = self.get_selected_pilot_cid() {
             let min_counter = self.min_counters.entry(pilot_cid.clone()).or_insert(0);
             let min = *min_counter;
             *min_counter = (*min_counter + 1) % 64;
             
             let mrn = self.last_received_mins.get(&pilot_cid).cloned();

             let (element_id, mut content) = match instruction {
                 "UNABLE" => (66, "UNABLE".to_string()),
                 "STANDBY" => (6, "STANDBY".to_string()),
                 "ROGER" => (3, "ROGER".to_string()),
                 _ => {
                     if instruction.starts_with("CLIMB TO") {
                         let val = arg.clone().unwrap_or("0".to_string());
                         (20, format!("CLIMB TO AND MAINTAIN {}", val))
                     } else if instruction.starts_with("DESCEND TO") {
                         let val = arg.clone().unwrap_or("0".to_string());
                         (23, format!("DESCEND TO AND MAINTAIN {}", val))
                     } else {
                         (65, instruction.to_string())
                     }
                 }
             };

             // Construct Element data if needed
             let data = if element_id == 20 || element_id == 23 {
                  // Parse Level
                  let val_str = arg.unwrap_or("0".to_string());
                  let (val, unit) = if val_str.starts_with("FL") {
                      (val_str[2..].parse::<i64>().unwrap_or(0), "FL")
                  } else {
                      (val_str.parse::<i64>().unwrap_or(0), "FT")
                  };
                  Some(serde_json::json!({ "value": val, "unit": unit }))
             } else {
                  None
             };

             let elements = vec![
                openlink_models::CpdlcMessageElementsItem {
                    id: Some(element_id.into()),
                    data, 
                    attribute: Some("W/U".to_string()), // Instructions usually require Wilco/Unable response
                }
             ];
             
             // Override attribute for specific types like UNABLE/STANDBY/ROGER which close the loop
             let final_elements = if element_id == 66 || element_id == 6 || element_id == 3 {
                 vec![ openlink_models::CpdlcMessageElementsItem { id: Some(element_id.into()), data: None, attribute: None } ]
             } else {
                 elements
             };

             let msg_struct = openlink_models::CpdlcMessage {
                min: min.into(),
                mrn: mrn.map(|v| v.into()),
                content: Some(content.clone()),
                elements: final_elements,
             };
             
             let payload = serde_json::to_value(msg_struct).unwrap().as_object().unwrap().clone().into_iter().collect();
             
             let env = OpenLinkEnvelope {
                 meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
                 routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: pilot_cid.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
                 type_: "cpdlc.message".to_string(),
                 payload,
             };
             
             let client = self.client.clone();
             tokio::spawn(async move {
                 let _ = client.publish_envelope("cpdlc.session.message", &env).await;
             });

             self.messages.entry(pilot_cid).or_default().push(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
                source: "ME".to_string(),
                content: format!("UPLINK: {} (MIN: {}, MRN: {:?})", content, min, mrn),
                timestamp: Utc::now().format("%H:%M:%S").to_string(),
                is_incoming: false,
            });
        }
    }
    
    fn send_custom_msg(&mut self, msg: String) {
        if let Some(pilot_cid) = self.get_selected_pilot_cid() {
             // Use Per-Connection MIN counter
             let min_counter = self.min_counters.entry(pilot_cid.clone()).or_insert(0);
             let min = *min_counter;
             *min_counter = (*min_counter + 1) % 64;
             let mrn = self.last_received_mins.get(&pilot_cid).cloned();
             
             // Construct CpdlcMessage
             let elements = vec![
                openlink_models::CpdlcMessageElementsItem {
                    id: Some(67), // DM67 Free Text
                    data: Some(serde_json::Value::String(msg.clone())), 
                    attribute: Some("Y".to_string()),
                }
             ];
             
             let msg_struct = openlink_models::CpdlcMessage {
                min: min.into(),
                mrn: mrn.map(|v| v.into()),
                content: Some(msg.clone()),
                elements,
             };
             
             let payload = serde_json::to_value(msg_struct).unwrap().as_object().unwrap().clone().into_iter().collect();
             
             let env = OpenLinkEnvelope {
                 meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
                 routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: pilot_cid.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
                 type_: "cpdlc.message".to_string(),
                 payload,
             };
             
             let client = self.client.clone();
             tokio::spawn(async move {
                 let _ = client.publish_envelope("cpdlc.session.message", &env).await;
             });

             self.messages.entry(pilot_cid).or_default().push(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
                source: "ME".to_string(),
                content: format!("MSG: {} (MIN: {}, MRN: {:?})", msg, min, mrn),
                timestamp: Utc::now().format("%H:%M:%S").to_string(),
                is_incoming: false,
            });
        }
    }
    
    fn send_terminate(&mut self) {
        if let Some(pilot_cid) = self.get_selected_pilot_cid() {
            let env = OpenLinkEnvelope {
                 meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
                 routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: pilot_cid.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
                 type_: "cpdlc.termination.request".to_string(),
                 payload: std::collections::HashMap::new(),
            };
            
            let client = self.client.clone();
            tokio::spawn(async move {
                let _ = client.publish_envelope("cpdlc.session.control", &env).await;
            });

            self.messages.entry(pilot_cid.clone()).or_default().push(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
                source: "ME".to_string(),
                content: "TERMINATION SENT".to_string(),
                timestamp: Utc::now().format("%H:%M:%S").to_string(),
                is_incoming: false,
            });
            self.show_notification(format!("Service Terminated for {}", pilot_cid));
            
            // Cleanup
            self.states.remove(&pilot_cid);
            if let Some(idx) = self.flights.iter().position(|r| *r == pilot_cid) {
                self.flights.remove(idx);
                // Reset selection
                if !self.flights.is_empty() {
                    self.list_state.select(Some(0));
                } else {
                    self.list_state.select(None);
                }
            }
        }
    }
    
    fn execute_command(&mut self) {
        if let Some(pilot_cid) = self.get_selected_pilot_cid() {
             let cmds = self.get_commands_for_flight(&pilot_cid);
             if let Some(idx) = self.command_state.selected() {
                 if let Some(cmd) = cmds.get(idx) {
                     match &cmd.action {
                         AtcCommandAction::AcceptLogon => self.send_logon_response(true),
                         AtcCommandAction::RejectLogon => self.send_logon_response(false),
                         AtcCommandAction::RequestConnection => self.send_connect(),
                         AtcCommandAction::Terminate => {
                             self.send_terminate();
                         },
                         AtcCommandAction::PromptHandoff => {
                             self.input_mode = InputMode::Editing;
                             self.pending_cmd = Some(AtcCommandAction::PromptHandoff);
                             self.input.clear();
                         },
                         AtcCommandAction::PromptTransfer => {
                             self.input_mode = InputMode::Editing;
                             self.pending_cmd = Some(AtcCommandAction::PromptTransfer);
                             self.input.clear();
                         },
                         AtcCommandAction::PromptContact => {
                             self.input_mode = InputMode::Editing;
                             self.pending_cmd = Some(AtcCommandAction::PromptContact);
                             self.input.clear();
                         },
                         AtcCommandAction::SendNdaContactRequest(target) => {
                             self.send_contact_request(target.clone());
                         },
                         AtcCommandAction::AcceptTransfer(from) => {
                             self.send_transfer_accept(from.clone());
                         },
                         AtcCommandAction::SendMessage => {
                             self.input_mode = InputMode::Editing;
                             self.pending_cmd = Some(AtcCommandAction::SendMessage);
                             self.input.clear();
                         },
                         AtcCommandAction::UplinkResponse(msg) => {
                             self.send_uplink(msg.as_str(), None);
                         },
                         AtcCommandAction::UplinkInstructionWithArg { template, prompt: _ } => {
                             self.input_mode = InputMode::Editing;
                             self.pending_cmd = Some(AtcCommandAction::UplinkInstructionWithArg { template: template.clone(), prompt: "".to_string() });
                             self.input.clear();
                         },
                     }
                 }
             }
        }
    }
}

impl AppController for AtcApp {
    fn update(&mut self, action: Action) {
        match action {
            Action::Key(key) => {
                match self.input_mode {
                    InputMode::Normal => {
                         match key.code {
                             KeyCode::Char('q') => self.should_quit = true,
                             KeyCode::Tab => {
                                 // Toggle Pane
                                 if self.active_pane == ActivePane::Flights {
                                     self.active_pane = ActivePane::Commands;
                                     self.command_state.select(Some(0));
                                 } else {
                                     self.active_pane = ActivePane::Flights;
                                 }
                             },
                             KeyCode::Down => {
                                 if self.active_pane == ActivePane::Flights {
                                    let i = match self.list_state.selected() {
                                        Some(i) => if i >= self.flights.len() - 1 { 0 } else { i + 1 },
                                        None => 0,
                                    };
                                    self.list_state.select(Some(i));
                                 } else {
                                     // Command list
                                     if let Some(cid) = self.get_selected_pilot_cid() {
                                         let len = self.get_commands_for_flight(&cid).len();
                                         if len > 0 {
                                            let i = match self.command_state.selected() {
                                                Some(i) => if i >= len - 1 { 0 } else { i + 1 },
                                                None => 0,
                                            };
                                            self.command_state.select(Some(i));
                                         }
                                     }
                                 }
                             },
                             KeyCode::Up => {
                                 if self.active_pane == ActivePane::Flights {
                                    let i = match self.list_state.selected() {
                                        Some(i) => if i == 0 { self.flights.len() - 1 } else { i - 1 },
                                        None => 0,
                                    };
                                    self.list_state.select(Some(i));
                                 } else {
                                     if let Some(cid) = self.get_selected_pilot_cid() {
                                         let len = self.get_commands_for_flight(&cid).len();
                                         if len > 0 {
                                             let i = match self.command_state.selected() {
                                                 Some(i) => if i == 0 { len - 1 } else { i - 1 },
                                                 None => 0,
                                             };
                                             self.command_state.select(Some(i));
                                         }
                                     }
                                 }
                             },
                             KeyCode::Enter => {
                                 if self.active_pane == ActivePane::Commands {
                                     self.execute_command();
                                 }
                             },
                             _ => {}
                         }
                    },
                    InputMode::Editing => {
                        match key.code {
                            KeyCode::Enter => {
                                let arg = self.input.drain(..).collect();
                                
                                if let Some(cmd) = self.pending_cmd.take() {
                                    match cmd {
                                        AtcCommandAction::PromptHandoff => {
                                            self.send_handoff(arg);
                                        },
                                        AtcCommandAction::PromptTransfer => {
                                            self.send_transfer_request(arg);
                                        },
                                        AtcCommandAction::PromptContact => {
                                            self.send_contact_request(arg);
                                        },
                                        AtcCommandAction::SendMessage => {
                                            self.send_custom_msg(arg);
                                        },
                                        AtcCommandAction::UplinkInstructionWithArg { template, .. } => {
                                             if template.contains("CLIMB") {
                                                 self.send_uplink("CLIMB TO", Some(arg));
                                             } else if template.contains("DESCEND") {
                                                 self.send_uplink("DESCEND TO", Some(arg));
                                             }
                                        },
                                        _ => {}
                                    }
                                }
                                self.input_mode = InputMode::Normal;
                            },
                            KeyCode::Esc => {
                                self.input_mode = InputMode::Normal;
                                self.pending_cmd = None;
                            },
                            KeyCode::Char(c) => {
                                self.input.push(c);
                            },
                            KeyCode::Backspace => {
                                self.input.pop();
                            },
                            _ => {}
                        }
                    }
                }
            },
            Action::MessageReceived(payload) => {
                if let Ok(env) = serde_json::from_str::<OpenLinkEnvelope>(&payload) {
                    self.handle_incoming_envelope(env);
                }
            },
            Action::Error(err) => {
                self.show_notification(format!("Error: {}", err));
            },
            _ => {}
        }
        
        if let Some((_, time)) = self.notification {
            if time.elapsed().as_secs() > 3 {
                self.notification = None;
            }
        }
    }

    fn render(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(f.area());
            
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25), // Flights
                Constraint::Percentage(50), // Chat
                Constraint::Percentage(25), // Commands
            ])
            .split(chunks[0]);
            
        // Left: Flights List
        let items: Vec<ListItem> = self.flights.iter().map(|cid| {
            let label = self.flight_callsigns.get(cid).unwrap_or(cid);
            let state = self.states.get(cid).unwrap_or(&AtcConnectionState::Unknown);
            let status_indicator = match state {
                AtcConnectionState::Connected => "[C]",
                AtcConnectionState::LogonReceived => "[L]",
                _ => "[ ]"
            };
            ListItem::new(format!("{} {}", status_indicator, label))
        }).collect();
        
        let flight_style = if self.active_pane == ActivePane::Flights {
             Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow)
        } else {
             Style::default().fg(Color::White)
        };

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(format!("Flights ({})", self.station_name)))
            .highlight_style(flight_style)
             .highlight_symbol(">> ");
        f.render_stateful_widget(list, main_chunks[0], &mut self.list_state);
        
        // Middle: Chat
        let selected_cid = self.get_selected_pilot_cid();
        let messages_block = Block::default().borders(Borders::ALL).title(match &selected_cid {
            Some(cid) => format!("Comm with {}", self.flight_callsigns.get(cid).unwrap_or(cid)),
            None => "Communication".to_string(),
        });
        
        let mut msg_lines = Vec::new();
        if let Some(p) = &selected_cid {
            if let Some(msgs) = self.messages.get(p) {
                for m in msgs {
                    let prefix = if m.is_incoming { "<-" } else { "->" };
                    let color = if m.is_incoming { Color::Cyan } else { Color::Green };
                    msg_lines.push(Line::from(vec![
                        Span::raw(format!("{} [{}]: ", m.timestamp, prefix)),
                        Span::styled(m.content.clone(), Style::default().fg(color)),
                    ]));
                }
            }
        }
        let msg_paragraph = Paragraph::new(msg_lines).block(messages_block);
        f.render_widget(msg_paragraph, main_chunks[1]);
        
        // Right: Commands
        let mut cmd_items = Vec::new();
        if let Some(cid) = &selected_cid {
            let cmds = self.get_commands_for_flight(cid);
            cmd_items = cmds.iter().map(|c| ListItem::new(c.label.clone())).collect();
        }
        
        let cmd_style = if self.active_pane == ActivePane::Commands {
             Style::default().add_modifier(Modifier::BOLD).fg(Color::LightGreen)
        } else {
             Style::default().fg(Color::Gray)
        };
        
        let cmd_list = List::new(cmd_items)
            .block(Block::default().borders(Borders::ALL).title("Commands [TAB to focus]"))
            .highlight_style(cmd_style)
            .highlight_symbol("> ");
        f.render_stateful_widget(cmd_list, main_chunks[2], &mut self.command_state);

        // Bottom: Input
        let input_style = match self.input_mode {
            InputMode::Normal => Style::default(),
            InputMode::Editing => Style::default().fg(Color::Yellow),
        };
        
        let prompt_text = if self.input_mode == InputMode::Editing {
            if let Some(cmd) = &self.pending_cmd {
                 match cmd {
                     AtcCommandAction::PromptHandoff => "Enter Next Authority (NDA):",
                     AtcCommandAction::PromptTransfer => "Enter Transfer Target ATSU:",
                     AtcCommandAction::PromptContact => "Enter Next ATSU (Contact):",
                     AtcCommandAction::SendMessage => "Enter Message:",
                     _ => "Input:"
                 }
            } else { "Input:" }
        } else {
            "[TAB] Switch Pane | [ENTER] Execute Command | 'q' Quit"
        };
        
        let title = format!("Input ({}) - {}", prompt_text, self.input_mode == InputMode::Editing);
        let input = Paragraph::new(self.input.as_str())
            .style(input_style)
            .block(Block::default().borders(Borders::ALL).title(title));
        f.render_widget(input, chunks[1]);
        
        // Notification Overlay
        if let Some((msg, _)) = &self.notification {
            let area = centered_rect(60, 20, f.area());
            let block = Paragraph::new(msg.as_str())
                .block(Block::default().borders(Borders::ALL).title("Notification").style(Style::default().bg(Color::Blue).fg(Color::White)));
            f.render_widget(Clear, area);
            f.render_widget(block, area);
        }
    }

    fn should_quit(&self) -> bool {
        self.should_quit
    }
}
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
