use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Clear, List, ListItem, ListState},
    Frame,
};
use openlink_models::{OpenLinkEnvelope, OpenLinkEnvelopeMeta, OpenLinkEnvelopeRouting, OpenLinkEnvelopeRoutingNetwork, CpdlcLogonRequest, CpdlcLogonResponse, CpdlcContactRequest, CpdlcContactResponse, CpdlcContactComplete, CpdlcMessage};
use crate::app_state::{AppController, ChatMessage, InputMode};
use crate::tui::Action;
use openlink_sdk::OpenLinkClient;
use std::collections::VecDeque;
use crossterm::event::KeyCode;
use chrono::Utc;
use uuid::Uuid;
use tokio::sync::mpsc::UnboundedSender;
use std::borrow::Cow;

#[derive(PartialEq, Clone)]
enum PilotStatus {
    Disconnected,
    LogonSent,
    LogonAccepted(String), // Accepted by ATC ID, waiting for connection request
    ConnectionRequested(String), // Received CR1, waiting for pilot confirmation
    Connected(String, Option<String>), // (Connected To ATC ID, Next Data Authority)
    ContactReceived(String, String), // (CurrentATC, NextATC)
    // Switching stores: Current Active ATC, New Target ATC, and flags for the handshake with New
    Switching { 
        current: String, 
        next: String, 
        next_logon_sent: bool,
        next_conn_req: bool, 
        next_connected: bool,
        is_forwarding: bool, // New flag: True if initiated via Connection Request (Logon Forwarding), False if via Contact
    },
}

#[derive(Clone)]
struct PilotCommand {
    label: String,
    action: CommandAction,
}

#[derive(Clone)]
enum CommandAction {
    Simple(String),
    Response(String), // WILCO, UNABLE, ROGER (Requires MRN)
    WithArg {
        prompt: String,
        template: String, // use {} marker
        is_logon: bool,
    },
    ConfirmConnection,
    AcceptContact, // Send WILCO to Contact Request
    LogonToNext, // Send Logon to New Station
    SendContactComplete, // Send Complete to Old Station
}

pub struct PilotApp {
    callsign: String,
    network_id: String,
    client: OpenLinkClient,
    tx: UnboundedSender<Action>,
    should_quit: bool,
    
    // UI State
    status: PilotStatus,
    menu_items: Vec<PilotCommand>,
    list_state: ListState,
    
    // Modal / Input State
    input_mode: InputMode,
    active_command_idx: Option<usize>, // Which command triggered the modal
    input_buffer: String,

    // Data
    messages: VecDeque<ChatMessage>,
    
    // Notifications
    notification: Option<(String, std::time::Instant)>,

    // Messaging State
    min_counter: u8,
    last_received_min: Option<u8>,
}

impl PilotApp {
    pub fn new(callsign: String, client: OpenLinkClient, tx: UnboundedSender<Action>) -> Self {
        let network_id = client.get_cid();
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        
        let status = PilotStatus::Disconnected;
        let menu_items = Self::get_menu_for_status(&status);

        Self {
            callsign,
            network_id,
            client,
            tx,
            should_quit: false,
            status,
            menu_items,
            list_state,
            input_mode: InputMode::Normal,
            active_command_idx: None,
            input_buffer: String::new(),
            messages: VecDeque::new(),
            notification: None,
            min_counter: 0,
            last_received_min: None,
        }
    }

    fn get_menu_for_status(status: &PilotStatus) -> Vec<PilotCommand> {
        match status {
            PilotStatus::Disconnected => vec![
                 PilotCommand { label: "LOGON [STATION]".to_string(), action: CommandAction::WithArg { prompt: "Enter Station (e.g. LFPG):".to_string(), template: "{}".to_string(), is_logon: true } },
            ],
            PilotStatus::LogonSent => vec![
                 PilotCommand { label: "LOGON (Retry)".to_string(), action: CommandAction::WithArg { prompt: "Enter Station (e.g. LFPG):".to_string(), template: "{}".to_string(), is_logon: true } },
            ],
            PilotStatus::LogonAccepted(_) => vec![
                // User waits here. Maybe allow sending generic message? Or nothing.
                // Normally strictly waiting.
                 PilotCommand { label: "(Waiting for Connection Request...)".to_string(), action: CommandAction::Simple("NO_OP".to_string()) },
            ],
            PilotStatus::ConnectionRequested(_) => vec![
                PilotCommand { label: "CONFIRM CONNECTION".to_string(), action: CommandAction::ConfirmConnection },
                // Could add REJECT here too
            ],
            PilotStatus::Connected(_, _) => vec![
                PilotCommand { label: "REQUEST CLIMB [LEVEL]".to_string(), action: CommandAction::WithArg { prompt: "Enter Level (e.g. FL350):".to_string(), template: "REQUEST CLIMB TO {}".to_string(), is_logon: false } },
                PilotCommand { label: "REQUEST DESCENT [LEVEL]".to_string(), action: CommandAction::WithArg { prompt: "Enter Level (e.g. FL100):".to_string(), template: "REQUEST DESCENT TO {}".to_string(), is_logon: false } },
                PilotCommand { label: "REQUEST DIRECT [POINT]".to_string(), action: CommandAction::WithArg { prompt: "Enter Waypoint:".to_string(), template: "REQUEST DIRECT TO {}".to_string(), is_logon: false } },
                PilotCommand { label: "WILCO (Resp)".to_string(), action: CommandAction::Response("WILCO".to_string()) },
                PilotCommand { label: "UNABLE (Resp)".to_string(), action: CommandAction::Response("UNABLE".to_string()) },
                PilotCommand { label: "ROGER (Resp)".to_string(), action: CommandAction::Response("ROGER".to_string()) },
            ],
            PilotStatus::ContactReceived(_current, next) => vec![
                PilotCommand { label: format!("ACCEPT CONTACT TO {} (WILCO)", next), action: CommandAction::AcceptContact },
                PilotCommand { label: "UNABLE".to_string(), action: CommandAction::Simple("UNABLE".to_string()) },
            ],
            PilotStatus::Switching { current: _, next, next_logon_sent, next_conn_req, next_connected, is_forwarding } => {
                let mut cmds = Vec::new();
                
                // If NOT forwarding (Contact method), we need to manually Logon
                if !is_forwarding && !next_logon_sent {
                    cmds.push(PilotCommand { label: format!("LOGON TO {}", next), action: CommandAction::LogonToNext });
                }

                if *next_conn_req && !next_connected {
                    cmds.push(PilotCommand { label: format!("CONFIRM CONNECTION FROM {}", next), action: CommandAction::ConfirmConnection });
                }
                
                // If NOT forwarding, we manually send Contact Complete to old station
                // In forwarding, the system handles the termination of the old link automatically
                if !is_forwarding && *next_connected {
                    cmds.push(PilotCommand { label: "SEND CONTACT COMPLETE".to_string(), action: CommandAction::SendContactComplete });
                }
                
                // Fallback / always available
                cmds.push(PilotCommand { label: "REQUEST DIRECT [POINT]".to_string(), action: CommandAction::WithArg { prompt: "Enter Waypoint:".to_string(), template: "REQUEST DIRECT TO {}".to_string(), is_logon: false } });
                cmds
            },
        }
    }
    
    fn update_menu(&mut self) {
        self.menu_items = Self::get_menu_for_status(&self.status);
        self.list_state.select(Some(0));
    }
    
    pub async fn listen(&self) {
        let client = self.client.clone();
        let my_cid = self.network_id.clone();
        let tx = self.tx.clone();
        
        // Listen on Network ID
        let subject = format!("cpdlc.response.{}", my_cid);
        
        tokio::spawn(async move {
            if let Ok(mut sub) = client.subscribe(&subject).await {
                while let Some(msg) = futures::StreamExt::next(&mut sub).await {
                     if let Ok(env) = serde_json::from_slice::<OpenLinkEnvelope>(&msg.payload) {
                         let _ = tx.send(Action::MessageReceived(serde_json::to_string(&env).unwrap()));
                    }
                }
            }
        });
    }


    fn set_status(&mut self, status: PilotStatus) {
        if self.status != status {
            self.status = status;
            self.update_menu();
        }
    }

    fn handle_incoming_envelope(&mut self, env: OpenLinkEnvelope) {
        // Prevent echo/duplication of own messages
        if env.routing.source == self.network_id {
            return;
        }

        // Update connection status based on envelope type
        if env.type_ == "cpdlc.connection.request" {
             let source = env.routing.source.clone();
             
             // 1. Switching Logic
             if let PilotStatus::Switching { current, next, next_logon_sent, next_conn_req: _, next_connected, is_forwarding } = &self.status {
                 if &source == next || source.starts_with(next) {
                     // Update next to match source exactly if it was a prefix match
                     let new_next = if &source != next { source.clone() } else { next.clone() };

                     self.set_status(PilotStatus::Switching { 
                         current: current.clone(), 
                         next: new_next, 
                         next_logon_sent: *next_logon_sent, 
                         next_conn_req: true, // Mark request received
                         next_connected: *next_connected,
                         is_forwarding: *is_forwarding
                     });
                     self.show_notification(format!("Connection Request from NDA {}", source));
                     
                     self.messages.push_back(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
                        source: source,
                        content: "CONNECTION REQUEST (NDA)".to_string(),
                        timestamp: Utc::now().format("%H:%M:%S").to_string(),
                        is_incoming: true,
                    });
                     return;
                 }
             }

             // 2. Connected Logic (Logon Forwarding Handling)
             // If we are Connected to A, and receive request from B (which is not A)
             if let PilotStatus::Connected(current, _) = &self.status {
                 if &source != current {
                     self.set_status(PilotStatus::Switching {
                         current: current.clone(),
                         next: source.clone(),
                         next_logon_sent: true, // Implicitly "sent" since it is forwarding
                         next_conn_req: true,
                         next_connected: false,
                         is_forwarding: true // Enable forwarding mode (hides manual Logon/ContactComplete)
                     });
                     self.show_notification(format!("Connection Request from Forwarded ATC {}", source));
                     self.messages.push_back(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
                        source: source.clone(),
                        content: "CONNECTION REQUEST (FWD)".to_string(),
                        timestamp: Utc::now().format("%H:%M:%S").to_string(),
                        is_incoming: true,
                    });
                     return;
                 }
             }

             self.set_status(PilotStatus::ConnectionRequested(env.routing.source.clone())); // For UI "Confirm" trigger
             self.show_notification(format!("Connection Request from {}", env.routing.source));
        } else if env.type_ == "cpdlc.logon.response" {
             if let Ok(val) = serde_json::to_value(env.payload.clone()) {
                 if let Ok(resp) = serde_json::from_value::<CpdlcLogonResponse>(val) {
                     if resp.accepted {
                         // START MODIFICATION: If switching, just notify, don't change main status (wait for conn req)
                         let source = env.routing.source.clone();
                         if let PilotStatus::Switching { .. } = &self.status {
                              self.show_notification(format!("NDA LOGON ACCEPTED by {}", source));
                              // Maybe update log
                         } else {
                             // Wait for connection request
                             self.set_status(PilotStatus::LogonAccepted(source.clone()));
                             self.show_notification(format!("Logon Accepted by {}", source));
                         }
                         // END MODIFICATION
                     } else {
                         self.set_status(PilotStatus::Disconnected);
                         self.show_notification(format!("Logon REJECTED by {}", env.routing.source));
                     }
                 }
             }
        } else if env.type_ == "cpdlc.connection.confirm" {
            // Unlikely in v3 Ground->Air direction but kept for robustness
            self.set_status(PilotStatus::Connected(env.routing.source.clone(), None));
        } else if env.type_ == "cpdlc.next_data_authority" {
             self.show_notification(format!("Received Handoff instruction from {}", env.routing.source));
             // Update State with NDA
             if let PilotStatus::Connected(cda, _) = &self.status {
                 if cda == &env.routing.source {
                      let nda = if let Ok(val) = serde_json::to_value(&env.payload) {
                          val.get("next_authority").and_then(|v| v.as_str()).unwrap_or("UNKNOWN").to_string()
                      } else { "UNKNOWN".to_string() };
                      
                      self.set_status(PilotStatus::Connected(cda.clone(), Some(nda.clone())));
                      self.show_notification(format!("NDA Updated: {}", nda));
                 }
             }
        } else if env.type_ == "cpdlc.contact.request" {
             if let Ok(val) = serde_json::to_value(&env.payload) {
                 if let Ok(req) = serde_json::from_value::<CpdlcContactRequest>(val) {
                     self.set_status(PilotStatus::ContactReceived(env.routing.source.clone(), req.facility.clone()));
                     self.show_notification(format!("CONTACT REQUEST: Contact {}", req.facility));
                 } else {
                     self.show_notification("Invalid Contact Request Payload".to_string());
                 }
             } else {
                 self.show_notification("Invalid Contact Request JSON".to_string());
             }
        }
        
        // Add to log
        let msg_display = match env.type_.as_str() {
            "cpdlc.connection.request" => "CONNECTION REQUEST".to_string(),
            "cpdlc.connection.confirm" => "CONNECTION ESTABLISHED".to_string(),
            "cpdlc.next_data_authority" => "NEXT DATA AUTHORITY".to_string(),
            // START MODIFICATION: Handle Termination
            "cpdlc.termination.request" => {
                 // Check if we are switching. If so, move to Next.
                 let next_connected_state = if let PilotStatus::Switching { next, next_connected, .. } = &self.status {
                     Some((next.clone(), *next_connected))
                 } else {
                     None
                 };

                 if let Some((next, next_connected)) = next_connected_state {
                     if next_connected {
                         self.set_status(PilotStatus::Connected(next.clone(), None)); // NDA becomes CDA
                         self.show_notification(format!("Service Terminated. Connected to {}", next));
                     } else {
                         self.set_status(PilotStatus::Disconnected);
                         self.show_notification("Service Terminated (Incomplete Handoff)".to_string());
                     }
                 } else {
                     self.set_status(PilotStatus::Disconnected);
                     self.show_notification("Service Terminated".to_string());
                 }
                 "TERMINATION REQUEST".to_string()
            },
            // END MODIFICATION
            "cpdlc.logon.response" => "LOGON RESPONSE".to_string(),
             "cpdlc.contact.request" => {
                 if let Ok(val) = serde_json::to_value(&env.payload) {
                    if let Ok(req) = serde_json::from_value::<CpdlcContactRequest>(val) {
                        format!("CONTACT {} ({})", req.facility, req.frequency.unwrap_or("---".to_string()))
                    } else { "CONTACT REQ".to_string() }
                 } else { "CONTACT REQ".to_string() }
            },
            "cpdlc.message" => {
                 // Logic handled below
                 if let Ok(val) = serde_json::to_value(&env.payload) {
                    val.get("content").and_then(|c| c.as_str()).unwrap_or("Empty Message").to_string()
                 } else {
                     "Msg Error".to_string()
                 }
            },
            _ => format!("MSG: {} (Src: {})", env.type_, env.routing.source),
        };
        
        // Parse specific fields for the message logic
        let mut min = None;
        let mut mrn = None;
        let mut requires_response = false;
        let mut response_attribute = None;

        if let Ok(val) = serde_json::to_value(&env.payload) {
             if let Some(m) = val.get("min").and_then(|v| v.as_i64()) {
                 min = Some(m as u8);
                 // Update Pilot State for subsequent replies
                 self.last_received_min = Some(m as u8);
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
        }

        self.messages.push_back(ChatMessage {
            min,
            mrn,
            requires_response,
            response_attribute,
            source: env.routing.source,
            content: msg_display,
            timestamp: Utc::now().format("%H:%M:%S").to_string(),
            is_incoming: true,
        });
    }
    
    fn show_notification(&mut self, msg: String) {
        self.notification = Some((msg, std::time::Instant::now()));
    }

    fn send_logon(&mut self, atc_target: String) {
        let req = CpdlcLogonRequest {
            callsign: self.callsign.clone(),
            aircraft_type: "A320".to_string(),
            origin: "KMIA".to_string(),
            destination: "EGLL".to_string(),
        };
        let payload = serde_json::to_value(req).unwrap().as_object().unwrap().clone().into_iter().collect();
        
        let env = OpenLinkEnvelope {
            meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
            routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: atc_target.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
            type_: "cpdlc.logon.request".to_string(),
            payload,
        };
        
        let client = self.client.clone();
        tokio::spawn(async move {
            let _ = client.publish_envelope("cpdlc.session.logon", &env).await;
        });

        self.messages.push_back(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
            source: "ME".to_string(),
            content: format!("LOGON SENT -> {}", atc_target),
            timestamp: Utc::now().format("%H:%M:%S").to_string(),
            is_incoming: false,
        });
        
        // START MODIFICATION: Handle switching state update
        if let PilotStatus::Switching { current, next, is_forwarding,  .. } = &self.status {
            // Update 'next' to the resolved target so incoming messages match
            if &atc_target == next {
                 self.set_status(PilotStatus::Switching { 
                     current: current.clone(), 
                     next: atc_target.clone(), 
                     next_logon_sent: true, 
                     next_conn_req: false, 
                     next_connected: false,
                     is_forwarding: *is_forwarding
                 });
                 return;
            }
        }
        // END MODIFICATION
        
        self.set_status(PilotStatus::LogonSent);
    }

    fn send_level_change_request(&mut self, level_str: String, is_climb: bool) {
        // Parse level (e.g. FL350, 35000)
        let (value, unit) = if level_str.starts_with("FL") {
            (level_str[2..].parse::<i64>().unwrap_or(0), "FL")
        } else {
            (level_str.parse::<i64>().unwrap_or(0), "FT")
        };

        let target = match &self.status {
            PilotStatus::Connected(atc, _) => atc.clone(),
            PilotStatus::Switching { current, .. } => current.clone(),
            _ => {
                self.show_notification("Not Connected!".to_string());
                return;
            }
        };

        let min = self.min_counter;
        self.min_counter = (self.min_counter + 1) % 64;

        // Construct standard CpdlcMessage with specific elements
        let element_id = if is_climb { 9 } else { 10 }; // 9=Request Climb, 10=Request Descent
        
        let elements = vec![
            openlink_models::CpdlcMessageElementsItem {
                id: Some(element_id.into()),
                data: Some(serde_json::json!({
                    "value": value,
                    "unit": unit
                })),
                attribute: Some("W/U".to_string()),
            }
        ];

        let content = if is_climb {
            format!("REQUEST CLIMB TO {}", level_str)
        } else {
             format!("REQUEST DESCENT TO {}", level_str)
        };

        let req = openlink_models::CpdlcMessage {
            min: min.into(),
            mrn: None,
            content: Some(content.clone()),
            elements: elements,
        };
        
        let payload = serde_json::to_value(req).unwrap().as_object().unwrap().clone().into_iter().collect();

        let env = OpenLinkEnvelope {
            meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
            routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: target.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
            type_: "cpdlc.message".to_string(),
            payload,
        };

        let client = self.client.clone();
        tokio::spawn(async move {
            let _ = client.publish_envelope("cpdlc.session.message", &env).await;
        });

        self.messages.push_back(ChatMessage {
            min: Some(min),
            mrn: None,
            requires_response: true,
            response_attribute: Some("W/U".to_string()),
            source: "ME".to_string(),
            content: content,
            timestamp: Utc::now().format("%H:%M:%S").to_string(),
            is_incoming: false,
        });
    }


    fn send_response(&mut self, response_type: &str) {
        let target = match &self.status {
            PilotStatus::Connected(atc, _) => atc.clone(),
            PilotStatus::Switching { current, .. } => current.clone(),
            _ => {
                self.show_notification("Not Connected!".to_string());
                return;
            }
        };

        // Determine min/mrn
        // If we are responding, we MUST have an MRN from the LAST received message that required a response.
        // In this simple implementation, we assume we are responding to the last message we received that had a MIN.
        
        let mrn = self.last_received_min;
        if mrn.is_none() {
             self.show_notification("No message to respond to!".to_string());
             // In real life we might just send it anyway if protocol allows unsolicited, but WILCO/UNABLE require reference
             return;
        }

        let min = self.min_counter;
        self.min_counter = (self.min_counter + 1) % 64;

        let (element_id, content) = match response_type {
            "WILCO" => (0, "WILCO"),
            "UNABLE" => (1, "UNABLE"),
            "ROGER" => (3, "ROGER"),
            _ => (67, response_type), // Fallback to free text
        };

        let elements = vec![
            openlink_models::CpdlcMessageElementsItem {
                id: Some(element_id.into()),
                data: None, // Standard responses don't have data usually
                attribute: None, // Responses don't usually require further response (Closure)
            }
        ];

        let msg_struct = openlink_models::CpdlcMessage {
            min: min.into(),
            mrn: mrn.map(|v| v.into()),
            content: Some(content.to_string()),
            elements: elements,
        };
        
        let payload = serde_json::to_value(msg_struct).unwrap().as_object().unwrap().clone().into_iter().collect();
         
        let env = OpenLinkEnvelope {
             meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
             routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: target.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
             type_: "cpdlc.message".to_string(),
             payload,
        };
         
        let client = self.client.clone();
        tokio::spawn(async move {
             let _ = client.publish_envelope("cpdlc.session.message", &env).await;
        });

        self.messages.push_back(ChatMessage {
            min: Some(min),
            mrn,
            requires_response: false,
            response_attribute: None,
            source: "ME".to_string(), 
            content: format!("RESP: {} (MIN: {}, MRN: {:?})", content, min, mrn),
            timestamp: Utc::now().format("%H:%M:%S").to_string(),
            is_incoming: false,
        });
    }

    fn send_message(&mut self, msg_content: String) {
        let target = match &self.status {
            PilotStatus::Connected(atc, _) => atc.clone(),
            PilotStatus::Switching { current, .. } => current.clone(),
            _ => {
                self.show_notification("Not Connected!".to_string());
                return;
            }
        };

        // Construct CpdlcMessage (DM67 Free Text)
        let min = self.min_counter;
        self.min_counter = (self.min_counter + 1) % 64;
        
        // If we represent a response (WILCO, UNABLE, ROGER), we MUST have an MRN.
        // For free text initiated by pilot, MRN is optional (usually None unless replying).
        // For simplicity here, if last_received_min is set, we treat it as a reply context.
        // In a real system, we'd distinguish "New Message" vs "Reply". 
        // Logic: If msg_content is WILCO/UNABLE/ROGER, use MRN. Else, maybe not?
        // Actually, the user spec says "The system must include MRN when responding".
        // Let's assume if we have a last_received_min, we are replying to it.
        let mrn = self.last_received_min;
        
        // Clear last_received_min after using it?
        // self.last_received_min = None; // Maybe? Or keep until next message replaces it?
        // Spec says "to close the dialogue".
        
        let elements = vec![
            openlink_models::CpdlcMessageElementsItem {
                id: Some(67), // DM67 Free Text
                data: Some(serde_json::Value::String(msg_content.clone())), 
                attribute: Some("Y".to_string()), 
            }
        ];

        let msg_struct = openlink_models::CpdlcMessage {
            min: min.into(),
            mrn: mrn.map(|v| v.into()),
            content: Some(msg_content.clone()),
            elements: elements,
        };
        
        let payload = serde_json::to_value(msg_struct).unwrap().as_object().unwrap().clone().into_iter().collect();
         
        let env = OpenLinkEnvelope {
             meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
             routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: target.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
             type_: "cpdlc.message".to_string(),
             payload,
        };
         
        let client = self.client.clone();
        tokio::spawn(async move {
             let _ = client.publish_envelope("cpdlc.session.message", &env).await;
        });

        self.messages.push_back(ChatMessage {
            min: Some(min),
            mrn,
            requires_response: false,
            response_attribute: None,
            source: "ME".to_string(),
            content: format!("MSG: {} (MIN: {}, MRN: {:?})", msg_content, min, mrn),
            timestamp: Utc::now().format("%H:%M:%S").to_string(),
            is_incoming: false,
        });
    }

    fn send_connection_confirm(&mut self) {
        // If we received a connection request, we confirm it
        // Or if we are just connecting
        // START MODIFICATION: Handle Switching TARGET
        let (target, is_switching_next) = match &self.status {
            PilotStatus::ConnectionRequested(atc) => (atc.clone(), false),
            PilotStatus::Switching { next, next_conn_req, .. } if *next_conn_req => (next.clone(), true),
            _ => {
                self.show_notification("Cannot Confirm: No Request!".to_string());
                return;
            }
        };
        // END MODIFICATION

        let env = OpenLinkEnvelope {
            meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
            routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: target.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
            type_: "cpdlc.connection.confirm".to_string(),
            payload: std::collections::HashMap::new(),
        };

        let client = self.client.clone();
        tokio::spawn(async move {
            let _ = client.publish_envelope("cpdlc.session.control", &env).await;
        });

        self.messages.push_back(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
            source: "ME".to_string(),
            content: format!("CONNECTION CONFIRM SENT -> {}", target),
            timestamp: Utc::now().format("%H:%M:%S").to_string(),
            is_incoming: false,
        });

        // Optimistically update status
        // START MODIFICATION
        if is_switching_next {
             let new_status = if let PilotStatus::Switching { current, next, next_logon_sent, next_conn_req, is_forwarding, .. } = &self.status {
                 Some(PilotStatus::Switching {
                     current: current.clone(),
                     next: next.clone(),
                     next_logon_sent: *next_logon_sent,
                     next_conn_req: *next_conn_req,
                     next_connected: true,
                     is_forwarding: *is_forwarding
                 })
             } else {
                 None
             };

             if let Some(status) = new_status {
                  // Extract next name for notification
                  let (next_name, is_fwd) = if let PilotStatus::Switching { next, is_forwarding, .. } = &status { (next.clone(), *is_forwarding) } else { ("UNKNOWN".to_string(), false) };
                  
                  self.set_status(status);
                  
                  if is_fwd {
                      self.show_notification(format!("Connected to {}. Waiting for Handoff Completion.", next_name));
                      // In forwarding, we wait for disconnect/termination from old ATC
                  } else {
                      self.show_notification(format!("Connected to {}. Waiting for Contact Complete.", next_name));
                  }
             }
        } else {
            self.set_status(PilotStatus::Connected(target, None));
        }
        // END MODIFICATION
    }

    fn send_contact_response(&mut self) {
        let (current, next) = match &self.status {
            PilotStatus::ContactReceived(curr, nxt) => (curr.clone(), nxt.clone()),
            _ => return,
        };

        let req = CpdlcContactResponse {
            wilco: true,
            reason: None,
        };
        let payload = serde_json::to_value(req).unwrap().as_object().unwrap().clone().into_iter().collect();

        let env = OpenLinkEnvelope {
            meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
            routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: current.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
            type_: "cpdlc.contact.response".to_string(),
            payload,
        };

        let client = self.client.clone();
        tokio::spawn(async move {
            let _ = client.publish_envelope("cpdlc.session.contact", &env).await;
        });

        self.messages.push_back(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
            source: "ME".to_string(),
            content: format!("CONTACT RESPONSE (WILCO) -> {}", current),
            timestamp: Utc::now().format("%H:%M:%S").to_string(),
            is_incoming: false,
        });

        // Move to switching state
        // Initial Switching State
        self.set_status(PilotStatus::Switching { 
            current: current, 
            next: next, 
            next_logon_sent: false, 
            next_conn_req: false, 
            next_connected: false,
            is_forwarding: false
        });
    }
    
    fn send_contact_complete(&mut self) {
         let (old_atc, _) = match &self.status {
            PilotStatus::Switching { current, next, .. } => (current.clone(), next.clone()),
            _ => return,
        };
        
        // Spec says: Once logon to new ATSU is successful, send Contact Complete to CURRENT (OLD) CDA.
        // It triggers termination.
        
        let req = CpdlcContactComplete {
            facility: old_atc.clone(), // Or maybe the one we contacted? Spec says "facility designation of the ATSU being contacted"
        };
        let payload = serde_json::to_value(req).unwrap().as_object().unwrap().clone().into_iter().collect();

        let env = OpenLinkEnvelope {
            meta: OpenLinkEnvelopeMeta { id: Uuid::new_v4(), timestamp: Utc::now(), correlation_id: None, version: "1.0".to_string() },
            routing: OpenLinkEnvelopeRouting { source: self.network_id.clone(), target: old_atc.clone(), network: OpenLinkEnvelopeRoutingNetwork::Vatsim },
            type_: "cpdlc.contact.complete".to_string(),
            payload,
        };

        let client = self.client.clone();
        tokio::spawn(async move {
            let _ = client.publish_envelope("cpdlc.session.contact", &env).await;
        });
        
         self.messages.push_back(ChatMessage { min: None, mrn: None, requires_response: false, response_attribute: None, 
            source: "ME".to_string(),
            content: format!("CONTACT COMPLETE -> {}", old_atc),
            timestamp: Utc::now().format("%H:%M:%S").to_string(),
            is_incoming: false,
        });
    }

    fn execute_selected_command(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            let cmd = &self.menu_items[idx];
            
            match &cmd.action {
                CommandAction::Simple(msg) => {
                    self.send_message(msg.clone());
                },
                CommandAction::Response(typ) => {
                    let t = typ.clone();
                    self.send_response(t.as_str());
                },
                CommandAction::ConfirmConnection => {
                    self.send_connection_confirm();
                },
                CommandAction::AcceptContact => {
                    self.send_contact_response();
                },
                CommandAction::LogonToNext => {
                    if let PilotStatus::Switching { next, .. } = &self.status {
                        self.send_logon(next.clone());
                    }
                },
                CommandAction::SendContactComplete => {
                    self.send_contact_complete();
                },
                CommandAction::WithArg { .. } => {
                     // Enter Input Mode
                     self.active_command_idx = Some(idx);
                     self.input_mode = InputMode::Editing;
                     self.input_buffer.clear();
                }
            }
        }
    }
    
    fn submit_input(&mut self) {
        if let Some(idx) = self.active_command_idx {
            let cmd = &self.menu_items[idx];
            if let CommandAction::WithArg { template, is_logon, .. } = &cmd.action {
                 let value = self.input_buffer.trim();
                 if !value.is_empty() {
                     if *is_logon {
                         self.send_logon(value.to_string());
                     } else if template.contains("CLIMB") || template.contains("DESCENT") {
                         // Hacky check but works for the current menu
                         self.send_level_change_request(value.to_string(), template.contains("CLIMB"));
                     } else {
                         let final_msg = template.replace("{}", value);
                         self.send_message(final_msg);
                     }
                 }
            }
        }
        // Reset
        self.input_mode = InputMode::Normal;
        self.active_command_idx = None;
        self.input_buffer.clear();
    }
}

impl AppController for PilotApp {
    fn update(&mut self, action: Action) {
         match action {
            Action::Key(key) => {
                 // Check Global Controls
                 if self.input_mode == InputMode::Normal && key.code == KeyCode::Char('q') {
                      self.should_quit = true;
                      return;
                 }
                 
                 match self.input_mode {
                    InputMode::Normal => {
                         match key.code {
                            KeyCode::Down => {
                                 let i = match self.list_state.selected() {
                                     Some(i) => if i >= self.menu_items.len() - 1 { 0 } else { i + 1 },
                                     None => 0,
                                 };
                                 self.list_state.select(Some(i));
                            },
                             KeyCode::Up => {
                                 let i = match self.list_state.selected() {
                                     Some(i) => if i == 0 { self.menu_items.len() - 1 } else { i - 1 },
                                     None => 0,
                                 };
                                 self.list_state.select(Some(i));
                             },
                             KeyCode::Enter => {
                                 self.execute_selected_command();
                             },
                             _ => {}
                         }
                    },
                    InputMode::Editing => {
                        match key.code {
                            KeyCode::Enter => self.submit_input(),
                            KeyCode::Esc => {
                                self.input_mode = InputMode::Normal;
                                self.active_command_idx = None;
                            },
                            KeyCode::Char(c) => self.input_buffer.push(c),
                            KeyCode::Backspace => {
                                self.input_buffer.pop();
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
            _ => {}
         }
         
          // Clear notification
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
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(f.area());
            
        // Top: Status
        let status_text: Cow<str> = match &self.status {
            PilotStatus::Disconnected => "DISCONNECTED".into(),
            PilotStatus::LogonSent => "LOGON SENT...".into(),
            PilotStatus::LogonAccepted(atc) => format!("LOGON ACCEPTED ({}) (Waiting...)", atc).into(),
            PilotStatus::ConnectionRequested(atc) => format!("CONN REQ ({})", atc).into(),
            PilotStatus::Connected(atc, nda) => if let Some(n) = nda {
                format!("CONNECTED {} (NDA: {})", atc, n).into()
            } else {
                format!("CONNECTED {}", atc).into()
            },
            PilotStatus::ContactReceived(curr, next) => format!("CONTACT {} (From {})", next, curr).into(),
            PilotStatus::Switching { current, next, next_connected, .. } => if *next_connected {
                format!("SWITCHING {} -> {} (Next: Connected)", current, next).into()
            } else {
                 format!("SWITCHING {} -> {} ...", current, next).into()
            },
        };
        
        let status_color = match self.status {
            PilotStatus::Disconnected => Color::Red,
            PilotStatus::LogonSent => Color::Yellow,
            PilotStatus::LogonAccepted(_) => Color::LightGreen,
            PilotStatus::ConnectionRequested(_) => Color::LightYellow,
            PilotStatus::Connected(_, _) => Color::Green,
            PilotStatus::ContactReceived(_,_) => Color::LightMagenta,
            PilotStatus::Switching { .. } => Color::LightBlue,
        };
        
        let header = Paragraph::new(Line::from(vec![
            Span::raw("Current ATC: "),
            Span::styled(status_text, Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
        ]))
        .block(Block::default().borders(Borders::ALL).title(format!("Pilot View - {} (CID: {})", self.callsign, self.network_id)));
        f.render_widget(header, chunks[0]);
        
        // Middle Split
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30), // Menu
                Constraint::Percentage(70), // Log
            ])
            .split(chunks[1]);

        // Left: Menu
        let items: Vec<ListItem> = self.menu_items.iter().map(|cmd| ListItem::new(cmd.label.as_str())).collect();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Commands"))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow))
             .highlight_symbol(">> ");
        f.render_stateful_widget(list, main_chunks[0], &mut self.list_state);

        // Right: Log
        let mut lines = Vec::new();
        for m in &self.messages {
             let prefix = if m.is_incoming { "<-" } else { "->" };
             let color = if m.is_incoming { Color::Cyan } else { Color::Green };
             lines.push(Line::from(vec![
                 Span::raw(format!("{} [{} {}]: ", m.timestamp, prefix, m.source)),
                 Span::styled(m.content.clone(), Style::default().fg(color)),
             ]));
        }
        let log = Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("CPDLC Log"));
        f.render_widget(log, main_chunks[1]);

         // Notification Overlay
        if let Some((msg, _)) = &self.notification {
            let area = centered_rect(60, 20, f.area());
            let block = Paragraph::new(msg.as_str())
                .block(Block::default().borders(Borders::ALL).title("Notification").style(Style::default().bg(Color::Blue).fg(Color::White)));
            f.render_widget(Clear, area);
            f.render_widget(block, area);
        }
        
        // MODAL OVERLAY for Input
        if self.input_mode == InputMode::Editing {
             if let Some(idx) = self.active_command_idx {
                 if let CommandAction::WithArg { prompt, .. } = &self.menu_items[idx].action {
                     let area = centered_rect(50, 20, f.area()); // Same helper as notification
                     
                     // Clear area first
                     f.render_widget(Clear, area);
                     
                     let input_block = Paragraph::new(self.input_buffer.as_str())
                        .style(Style::default().fg(Color::Yellow))
                        .block(Block::default().borders(Borders::ALL).title(prompt.as_str()));
                     
                     f.render_widget(input_block, area);
                 }
             }
        }
    }

    fn should_quit(&self) -> bool {
        self.should_quit
    }
}

// Helper (duplicated for simplicity, in real app move to utils)
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
