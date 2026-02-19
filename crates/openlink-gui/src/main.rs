mod state;
mod nats_client;
mod components;
mod persistence;
mod i18n;

use dioxus::prelude::*;
use uuid::Uuid;
use state::{AppState, NatsClients, StationType, AtcLinkedFlight, SetupFields};
use components::tab_bar::TabBar;
use components::station_setup::StationSetup;
use components::dcdu_view::DcduView;
use components::atc_view::AtcView;
use openlink_models::{AcarsEndpointAddress, CpdlcConnectionPhase, CpdlcEnvelope, CpdlcMetaMessage, OpenLinkMessage, AcarsMessage, SerializedMessagePayload};

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut app_state = use_signal(|| AppState::new());
    let mut nats_clients = use_signal(|| NatsClients::default());
    let locale_signal = use_signal(|| i18n::Locale::En);
    use_context_provider(|| locale_signal);

    // ── Detect tabs that need an inbox listener and spawn them ──
    // This runs at the App level so the spawned tasks survive component unmounting.
    use_effect(move || {
        // Collect tabs that are Connected but have no active listener
        let tabs_needing_listener: Vec<(Uuid, SetupFields)> = {
            let state = app_state.read();
            state.tabs.iter()
                .filter(|t| matches!(t.phase, state::TabPhase::Connected(_)) && !t.nats_task_active)
                .map(|t| (t.id, t.setup.clone()))
                .collect()
        };

        for (tab_id, setup) in tabs_needing_listener {
            // Mark as active immediately so we don't re-spawn
            {
                let mut state = app_state.write();
                if let Some(t) = state.tab_mut_by_id(tab_id) {
                    t.nats_task_active = true;
                }
            }

            // Get the client clone
            let client = {
                let clients = nats_clients.read();
                clients.get(&tab_id).cloned()
            };

            if let Some(client) = client {
                // Spawn the inbox listener at App scope — it will live as long as the app
                spawn(async move {
                    spawn_inbox_listener(tab_id, setup, client, app_state).await;
                });
            }
        }
    });

    rsx! {
        style { {include_str!("style.css")} }
        div { class: "app-root",
            TabBar {
                tabs: app_state.read().tabs.clone(),
                active_tab: app_state.read().active_tab,
                on_select: move |idx: usize| {
                    app_state.write().active_tab = idx;
                },
                on_close: move |idx: usize| {
                    // Remove NATS client for this tab
                    let tab_id = app_state.read().tabs.get(idx).map(|t| t.id);
                    if let Some(id) = tab_id {
                        nats_clients.write().remove(&id);
                    }
                    app_state.write().close_tab(idx);
                },
                on_new: move |_| {
                    app_state.write().add_tab();
                },
            }

            // Active tab content
            {
                let state = app_state.read();
                if let Some(tab) = state.tabs.get(state.active_tab) {
                    let tab_id = tab.id;
                    let phase = tab.phase.clone();
                    drop(state);

                    match phase {
                        state::TabPhase::Setup => rsx! {
                            StationSetup {
                                tab_id,
                                app_state,
                                nats_clients,
                            }
                        },
                        state::TabPhase::Connected(ref station_type) => {
                            match station_type {
                                state::StationType::Aircraft => rsx! {
                                    DcduView {
                                        tab_id,
                                        app_state,
                                        nats_clients,
                                    }
                                },
                                state::StationType::Atc => rsx! {
                                    AtcView {
                                        tab_id,
                                        app_state,
                                        nats_clients,
                                    }
                                },
                            }
                        }
                    }
                } else {
                    let tr = i18n::t(*locale_signal.read());
                    rsx! {
                        div { class: "empty-state",
                            p { "{tr.create_tab_prompt}" }
                        }
                    }
                }
            }
        }
    }
}

/// Persistent inbox listener spawned at App level.
/// Runs until the NATS subscription ends (e.g. client disconnected / tab closed).
async fn spawn_inbox_listener(
    tab_id: Uuid,
    setup: SetupFields,
    client: openlink_sdk::OpenLinkClient,
    mut app_state: Signal<AppState>,
) {
    let subscriber = match client.subscribe_inbox().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[{tab_id}] Erreur subscription inbox: {e}");
            return;
        }
    };

    println!("[{tab_id}] Inbox listener started for {}", setup.callsign);

    use futures::StreamExt;
    let mut subscriber = subscriber;
    while let Some(message) = subscriber.next().await {
        // Check if tab still exists (may have been closed)
        let tab_exists = app_state.read().tab_by_id(tab_id).is_some();
        if !tab_exists {
            println!("[{tab_id}] Tab closed, stopping inbox listener");
            break;
        }

        let raw = String::from_utf8_lossy(&message.payload).to_string();
        println!("[{tab_id}] Received: {}", &raw[..raw.len().min(120)]);

        let envelope = serde_json::from_slice::<openlink_models::OpenLinkEnvelope>(&message.payload).ok();

        let from_callsign = envelope.as_ref().and_then(|env| {
            nats_client::extract_cpdlc_meta(env)
                .map(|(cpdlc, _, _)| cpdlc.source.to_string())
                .or_else(|| {
                    nats_client::extract_cpdlc_application(env)
                        .map(|(cpdlc, _, _)| cpdlc.source.to_string())
                })
        });

        // Extract human-readable display text from the CPDLC message
        let display_text: Option<String> = envelope.as_ref().and_then(|env| {
            match &env.payload {
                OpenLinkMessage::Acars(acars_env) => {
                    match &acars_env.message {
                        AcarsMessage::CPDLC(cpdlc_env) => {
                            let serialized: SerializedMessagePayload = cpdlc_env.message.clone().into();
                            Some(serialized.to_string())
                        }
                    }
                }
                _ => None,
            }
        });

        // Handle auto-responses (logon/connection protocol)
        if let Some(ref env) = envelope {
            if let Some((cpdlc, meta, aircraft_address)) = nats_client::extract_cpdlc_meta(env) {
                handle_incoming_meta(meta, cpdlc, &aircraft_address, tab_id, &mut app_state, &client, &setup).await;
            }
        }

        // Don't store protocol-internal messages in the message list
        let is_internal_meta = envelope.as_ref().is_some_and(|env| {
            if let OpenLinkMessage::Acars(ref acars) = env.payload {
                let AcarsMessage::CPDLC(ref cpdlc) = acars.message;
                matches!(cpdlc.message, openlink_models::CpdlcMessageType::Meta(
                    CpdlcMetaMessage::SessionUpdate { .. }
                    | CpdlcMetaMessage::ConnectionRequest
                    | CpdlcMetaMessage::ConnectionResponse { .. }
                ))
            } else {
                false
            }
        });

        if !is_internal_meta {
            // Extract MIN/MRN/response_attr from application messages
            let (min, mrn, response_attr) = envelope
                .as_ref()
                .and_then(|env| nats_client::extract_cpdlc_application(env))
                .map(|(_, app, _)| {
                    (
                        Some(app.min),
                        app.mrn,
                        Some(format!("{:?}", app.effective_response_attr())),
                    )
                })
                .unwrap_or((None, None, None));

            let received = state::ReceivedMessage {
                timestamp: chrono::Utc::now(),
                raw_json: raw,
                envelope,
                from_callsign,
                to_callsign: None,
                display_text,
                is_outgoing: false,
                min,
                mrn,
                response_attr,
                responded: false,
            };

            let mut s = app_state.write();
            if let Some(t) = s.tab_mut_by_id(tab_id) {
                t.messages.push(received);
            }
        }
    }

    println!("[{tab_id}] Inbox listener stopped");
}

/// Handle incoming CPDLC meta messages (logon/connection protocol)
async fn handle_incoming_meta(
    meta: &CpdlcMetaMessage,
    cpdlc: &CpdlcEnvelope,
    aircraft_address: &AcarsEndpointAddress,
    tab_id: Uuid,
    app_state: &mut Signal<AppState>,
    client: &openlink_sdk::OpenLinkClient,
    setup: &SetupFields,
) {
    match meta {
        // Aircraft: auto-accept connection requests
        CpdlcMetaMessage::ConnectionRequest => {
            let is_aircraft = {
                let state = app_state.read();
                state.tab_by_id(tab_id)
                    .map(|t| t.setup.station_type == StationType::Aircraft)
                    .unwrap_or(false)
            };
            if is_aircraft {
                let addr: AcarsEndpointAddress = setup.acars_address.clone().into();
                let msg = nats_client::build_connection_response(
                    &setup.callsign,
                    &addr,
                    &cpdlc.source.to_string(),
                    true,
                );
                let _ = client.send_to_server(msg).await;
            }
        }
        // Aircraft: receive logon response — nothing to do locally,
        // the SessionUpdate will set the authoritative state.
        CpdlcMetaMessage::LogonResponse { .. } => {}
        // ATC: receive logon request from aircraft — add to linked flights
        CpdlcMetaMessage::LogonRequest { .. } => {
            let aircraft_callsign = cpdlc.source.to_string();
            let mut state = app_state.write();
            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                if tab.setup.station_type == StationType::Atc {
                    if !tab.linked_flights.iter().any(|f| f.callsign == aircraft_callsign) {
                        tab.linked_flights.push(AtcLinkedFlight {
                            callsign: aircraft_callsign.clone(),
                            aircraft_callsign: aircraft_callsign.clone(),
                            aircraft_address: aircraft_address.clone(),
                            phase: CpdlcConnectionPhase::LogonPending,
                        });
                    }
                }
            }
        }
        // ATC: receive connection response — nothing to do locally,
        // the SessionUpdate will set the authoritative state.
        CpdlcMetaMessage::ConnectionResponse { .. } => {}
        // Aircraft: receive contact request — auto-logon to the new station
        CpdlcMetaMessage::ContactRequest { station } => {
            let is_aircraft = {
                let state = app_state.read();
                state.tab_by_id(tab_id)
                    .map(|t| t.setup.station_type == StationType::Aircraft)
                    .unwrap_or(false)
            };
            if is_aircraft {
                let addr: AcarsEndpointAddress = setup.acars_address.clone().into();
                let msg = nats_client::build_logon_request(
                    &setup.callsign,
                    &addr,
                    &station.to_string(),
                );
                let _ = client.send_to_server(msg).await;
                push_outgoing_message(app_state, tab_id, &format!("LOGON REQUEST → {station}"));
            }
        }
        // ATC: receive logon forward — auto-send connection request to the aircraft
        CpdlcMetaMessage::LogonForward { flight, .. } => {
            let is_atc = {
                let state = app_state.read();
                state.tab_by_id(tab_id)
                    .map(|t| t.setup.station_type == StationType::Atc)
                    .unwrap_or(false)
            };
            if is_atc {
                let msg = nats_client::build_connection_request(
                    &setup.callsign,
                    &flight.to_string(),
                    aircraft_address,
                );
                let _ = client.send_to_server(msg).await;
                push_outgoing_message(app_state, tab_id, &format!("CONNECTION REQUEST → {flight}"));
                // Also add to linked flights
                let flight_cs = flight.to_string();
                let mut state = app_state.write();
                if let Some(tab) = state.tab_mut_by_id(tab_id) {
                    if !tab.linked_flights.iter().any(|f| f.callsign == flight_cs) {
                        tab.linked_flights.push(AtcLinkedFlight {
                            callsign: flight_cs.clone(),
                            aircraft_callsign: flight_cs.clone(),
                            aircraft_address: aircraft_address.clone(),
                            phase: CpdlcConnectionPhase::LogonPending,
                        });
                    }
                }
            }
        }
        // Server-authoritative session update — replace local session state
        CpdlcMetaMessage::SessionUpdate { session } => {
            let mut state = app_state.write();
            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                tab.session = Some(session.clone());

                // Clear optimistic logon_pending when session arrives
                if tab.setup.station_type == StationType::Aircraft {
                    tab.logon_pending = None;
                }

                // For ATC: update/add the linked flight entry
                if tab.setup.station_type == StationType::Atc {
                    // Collect callsigns still present in the session
                    let active_peer = session.active_connection.as_ref().map(|c| c.peer.to_string());
                    let inactive_peer = session.inactive_connection.as_ref().map(|c| c.peer.to_string());

                    // Update or add flights from active connection
                    if let Some(ref conn) = session.active_connection {
                        let aircraft_callsign = conn.peer.to_string();
                        if let Some(flight) = tab.linked_flights.iter_mut().find(|f| f.callsign == aircraft_callsign) {
                            flight.phase = conn.phase;
                        }
                    }
                    // Update flights from inactive connection
                    if let Some(ref conn) = session.inactive_connection {
                        let aircraft_callsign = conn.peer.to_string();
                        if let Some(flight) = tab.linked_flights.iter_mut().find(|f| f.callsign == aircraft_callsign) {
                            flight.phase = conn.phase;
                        }
                    }

                    // Remove flights whose callsign is no longer in either connection
                    tab.linked_flights.retain(|f| {
                        active_peer.as_ref().is_some_and(|p| *p == f.callsign)
                            || inactive_peer.as_ref().is_some_and(|p| *p == f.callsign)
                    });
                    if tab.selected_flight_idx.map_or(false, |idx| idx >= tab.linked_flights.len()) {
                        tab.selected_flight_idx = None;
                    }
                }
            }
        }
        _ => {}
    }
}

/// Push an outgoing message to the tab's message list for display.
/// `to_callsign` should be set when the ATC sends a message to a specific flight.
pub fn push_outgoing_message(app_state: &mut Signal<AppState>, tab_id: Uuid, display_text: &str) {
    push_outgoing_message_to(app_state, tab_id, display_text, None);
}

pub fn push_outgoing_message_to(app_state: &mut Signal<AppState>, tab_id: Uuid, display_text: &str, to_callsign: Option<&str>) {
    let msg = state::ReceivedMessage {
        timestamp: chrono::Utc::now(),
        raw_json: String::new(),
        envelope: None,
        from_callsign: None,
        to_callsign: to_callsign.map(|s| s.to_string()),
        display_text: Some(display_text.to_string()),
        is_outgoing: true,
        min: None,
        mrn: None,
        response_attr: None,
        responded: false,
    };
    let mut s = app_state.write();
    if let Some(t) = s.tab_mut_by_id(tab_id) {
        t.messages.push(msg);
    }
}
