mod state;
mod nats_client;
mod components;
mod persistence;
mod i18n;

use dioxus::prelude::*;
use uuid::Uuid;
use state::{AppState, NatsClients, StationType, GroundStationStatus, AtcLinkedFlight, AtcFlightLinkStatus, SetupFields};
use components::tab_bar::TabBar;
use components::station_setup::StationSetup;
use components::dcdu_view::DcduView;
use components::atc_view::AtcView;
use openlink_models::{CpdlcEnvelope, CpdlcMetaMessage, OpenLinkMessage, AcarsMessage, SerializedMessagePayload};

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
            nats_client::extract_cpdlc_meta(env).map(|(cpdlc, _)| cpdlc.source.to_string())
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
            if let Some((cpdlc, meta)) = nats_client::extract_cpdlc_meta(env) {
                handle_incoming_meta(meta, cpdlc, tab_id, &mut app_state, &client, &setup).await;
            }
        }

        let received = state::ReceivedMessage {
            timestamp: chrono::Utc::now(),
            raw_json: raw,
            envelope,
            from_callsign,
            display_text,
        };

        let mut s = app_state.write();
        if let Some(t) = s.tab_mut_by_id(tab_id) {
            t.messages.push(received);
        }
    }

    println!("[{tab_id}] Inbox listener stopped");
}

/// Handle incoming CPDLC meta messages (logon/connection protocol)
async fn handle_incoming_meta(
    meta: &CpdlcMetaMessage,
    cpdlc: &CpdlcEnvelope,
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
                let msg = nats_client::build_connection_response(
                    &setup.callsign,
                    &setup.acars_address,
                    &cpdlc.source.to_string(),
                    true,
                );
                let _ = client.send_to_server(msg).await;

                let mut state = app_state.write();
                if let Some(tab) = state.tab_mut_by_id(tab_id) {
                    if let GroundStationStatus::LogonPending(ref station) = tab.ground_station {
                        tab.ground_station = GroundStationStatus::Connected(station.clone());
                    }
                }
            }
        }
        // Aircraft: receive logon response
        CpdlcMetaMessage::LogonResponse { accepted } => {
            let mut state = app_state.write();
            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                if tab.setup.station_type == StationType::Aircraft && !accepted {
                    tab.ground_station = GroundStationStatus::Disconnected;
                }
            }
        }
        // ATC: receive logon request from aircraft — source is the aircraft callsign
        CpdlcMetaMessage::LogonRequest { .. } => {
            let aircraft_callsign = cpdlc.source.to_string();
            let mut state = app_state.write();
            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                if tab.setup.station_type == StationType::Atc {
                    if !tab.linked_flights.iter().any(|f| f.callsign == aircraft_callsign) {
                        tab.linked_flights.push(AtcLinkedFlight {
                            callsign: aircraft_callsign.clone(),
                            aircraft_callsign: aircraft_callsign.clone(),
                            aircraft_address: String::new(), // filled when connection completes
                            status: AtcFlightLinkStatus::LogonRequested,
                        });
                    }
                }
            }
        }
        // ATC: receive connection response from aircraft
        CpdlcMetaMessage::ConnectionResponse { accepted } => {
            if *accepted {
                let mut state = app_state.write();
                if let Some(tab) = state.tab_mut_by_id(tab_id) {
                    if tab.setup.station_type == StationType::Atc {
                        // Mark the most recent LogonRequested flight as Connected
                        for f in tab.linked_flights.iter_mut() {
                            if f.status == AtcFlightLinkStatus::LogonRequested {
                                f.status = AtcFlightLinkStatus::Connected;
                                break;
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}
