mod state;
mod nats_client;
mod components;
mod persistence;
mod i18n;

use dioxus::prelude::*;
use std::time::Duration;
use uuid::Uuid;
use state::{AppState, NatsClients, StationType, SetupFields};
use components::tab_bar::TabBar;
use components::station_setup::StationSetup;
use components::dcdu_view::DcduView;
use components::atc_view::AtcView;
use openlink_models::{AcarsEndpointAddress, AcarsMessage, CpdlcApplicationMessage, CpdlcArgument, CpdlcEnvelope, CpdlcMetaMessage, OpenLinkMessage, SerializedMessagePayload};

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
        document::Title { "Openlink Demonstrator GUI" }
        style { {include_str!("style.css")} }
        div { class: "app-root",
            TabBar {
                tabs: app_state.read().tabs.clone(),
                active_tab: app_state.read().active_tab,
                on_select: move |idx: usize| {
                    app_state.write().active_tab = idx;
                },
                on_close: move |idx: usize| {
                    // Best-effort graceful offline before removing the client.
                    let tab_info = {
                        let state = app_state.read();
                        state.tabs.get(idx).map(|t| (t.id, t.setup.clone(), t.phase.clone()))
                    };
                    if let Some((id, setup, phase)) = tab_info {
                        if matches!(phase, state::TabPhase::Connected(_)) {
                            let client = nats_clients.read().get(&id).cloned();
                            if let Some(client) = client {
                                spawn(async move {
                                    let _ = nats_client::send_offline_status(
                                        &client,
                                        &setup.network_id,
                                        &setup.network_address,
                                        &setup.callsign,
                                        &setup.acars_address,
                                    )
                                    .await;
                                });
                            }
                        }
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
    let mut heartbeat = tokio::time::interval(Duration::from_secs(25));
    // First tick fires immediately by default; skip it because we already sent ONLINE at connect time.
    heartbeat.tick().await;

    loop {
        // Check if tab still exists (may have been closed)
        let tab_exists = app_state.read().tab_by_id(tab_id).is_some();
        if !tab_exists {
            println!("[{tab_id}] Tab closed, stopping inbox listener");
            break;
        }

        tokio::select! {
            _ = heartbeat.tick() => {
                // Application-level presence heartbeat lease refresh.
                let _ = nats_client::send_online_status(
                    &client,
                    &setup.network_id,
                    &setup.network_address,
                    &setup.callsign,
                    &setup.acars_address,
                )
                .await;
            }
            maybe_message = subscriber.next() => {
                let Some(message) = maybe_message else {
                    break;
                };

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
            if let Some((cpdlc, app, aircraft_address)) = nats_client::extract_cpdlc_application(env) {
                handle_incoming_application(app, cpdlc, &aircraft_address, tab_id, &mut app_state, &client, &setup).await;
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
                        Some(app.effective_response_attr()),
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
                let msg = client.cpdlc_connection_response(
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
        CpdlcMetaMessage::LogonRequest { .. } => {}
        // ATC: receive connection response — nothing to do locally,
        // the SessionUpdate will set the authoritative state.
        CpdlcMetaMessage::ConnectionResponse { .. } => {}
        // ATC: receive logon forward — auto-send connection request to the aircraft
        CpdlcMetaMessage::LogonForward { flight, .. } => {
            let is_atc = {
                let state = app_state.read();
                state.tab_by_id(tab_id)
                    .map(|t| t.setup.station_type == StationType::Atc)
                    .unwrap_or(false)
            };
            if is_atc {
                let msg = client.cpdlc_connection_request(
                    &setup.callsign,
                    &flight.to_string(),
                    aircraft_address,
                );
                let _ = client.send_to_server(msg).await;
                push_outgoing_message(app_state, tab_id, &format!("CONNECTION REQUEST → {flight}"));
            }
        }
        // Server-authoritative session update — replace local session state
        CpdlcMetaMessage::SessionUpdate { session } => {
            let mut state = app_state.write();
            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                tab.session = Some(session.clone());

                // For ATC: reconcile flights from server-authoritative session snapshots.
                if tab.setup.station_type == StationType::Atc {
                    let key = session
                        .aircraft
                        .as_ref()
                        .map(|c| c.to_string())
                        .or_else(|| session.active_connection.as_ref().map(|c| c.peer.to_string()))
                        .or_else(|| session.inactive_connection.as_ref().map(|c| c.peer.to_string()));

                    if let Some(key) = key {
                        if session.active_connection.is_none() && session.inactive_connection.is_none() {
                            tab.atc_sessions.remove(&key);
                        } else {
                            tab.atc_sessions.insert(key, session.clone());
                        }
                    }

                    if tab.selected_flight_idx.map_or(false, |idx| idx >= tab.atc_sessions.len()) {
                        tab.selected_flight_idx = None;
                    }
                }
            }
        }
    }
}

/// Handle incoming CPDLC application messages used for session flow helpers.
///
/// Specifically, aircraft auto-logon on `UM117 CONTACT [unit] [frequency]`.
async fn handle_incoming_application(
    app: &CpdlcApplicationMessage,
    cpdlc: &CpdlcEnvelope,
    aircraft_address: &AcarsEndpointAddress,
    tab_id: Uuid,
    app_state: &mut Signal<AppState>,
    client: &openlink_sdk::OpenLinkClient,
    setup: &SetupFields,
) {
    let is_aircraft = {
        let state = app_state.read();
        state.tab_by_id(tab_id)
            .map(|t| t.setup.station_type == StationType::Aircraft)
            .unwrap_or(false)
    };

    // Auto-send CPDLC logical acknowledgement for incoming application messages.
    // Do not acknowledge logical acknowledgements themselves to avoid loops.
    if openlink_sdk::should_auto_send_logical_ack(&app.elements, app.min)
        && cpdlc.source.to_string() != setup.callsign
    {
        let aircraft_callsign = if is_aircraft {
            setup.callsign.clone()
        } else {
            cpdlc.source.to_string()
        };
        let ack = client.cpdlc_logical_ack(
            &setup.callsign,
            &cpdlc.source.to_string(),
            &aircraft_callsign,
            aircraft_address,
            app.min,
        );
        let _ = client.send_to_server(ack).await;
    }

    if !is_aircraft {
        return;
    }

    for element in &app.elements {
        if element.id != "UM117" {
            continue;
        }

        let station = element
            .args
            .iter()
            .find_map(|arg| match arg {
                CpdlcArgument::UnitName(unit) => Some(unit.clone()),
                _ => None,
            })
            .unwrap_or_else(|| cpdlc.source.to_string());

        let addr: AcarsEndpointAddress = setup.acars_address.clone().into();
        let msg = client.cpdlc_logon_request(&setup.callsign, &addr, &station);
        let _ = client.send_to_server(msg).await;
        push_outgoing_message(app_state, tab_id, &format!("LOGON REQUEST → {station}"));
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
