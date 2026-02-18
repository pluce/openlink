use dioxus::prelude::*;
use uuid::Uuid;

use crate::state::{AppState, NatsClients, GroundStationStatus};
use crate::nats_client;
use crate::i18n::{use_locale, t};
use crate::components::shared::{MessageList, StatusBadge};

/// Aircraft DCDU (Datalink Control & Display Unit) view
#[component]
pub fn DcduView(
    tab_id: Uuid,
    app_state: Signal<AppState>,
    nats_clients: Signal<NatsClients>,
) -> Element {
    let locale = use_locale();
    let tr = t(*locale.read());
    let state = app_state.read();
    let tab = match state.tab_by_id(tab_id) {
        Some(t) => t,
        None => return rsx! { p { "{tr.tab_not_found}" } },
    };

    let ground_station = tab.ground_station.clone();
    let logon_input = tab.logon_input.clone();
    let messages = tab.messages.clone();
    let callsign = tab.setup.callsign.clone();
    let acars_address = tab.setup.acars_address.clone();
    drop(state);

    rsx! {
        div { class: "dcdu-container",
            // Header bar
            div { class: "dcdu-header",
                div { class: "dcdu-title",
                    span { class: "dcdu-label", "DCDU" }
                    span { class: "dcdu-callsign", "{callsign}" }
                }
            }

            // Ground station connection area
            div { class: "dcdu-ground-station",
                h3 { "{tr.ground_station}" }
                match &ground_station {
                    GroundStationStatus::Disconnected => rsx! {
                        div { class: "logon-form",
                            input {
                                r#type: "text",
                                class: "logon-input",
                                maxlength: "4",
                                placeholder: "{tr.icao_placeholder}",
                                value: "{logon_input}",
                                oninput: move |evt: Event<FormData>| {
                                    let val = evt.value().to_uppercase();
                                    if val.len() <= 4 {
                                        let mut state = app_state.write();
                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                            tab.logon_input = val;
                                        }
                                    }
                                },
                            }
                            button {
                                class: "logon-btn",
                                disabled: logon_input.len() != 4,
                                onclick: {
                                    let callsign = callsign.clone();
                                    let acars_address = acars_address.clone();
                                    move |_| {
                                        let logon_target = {
                                            let state = app_state.read();
                                            state.tab_by_id(tab_id).map(|t| t.logon_input.clone())
                                        };
                                        if let Some(target) = logon_target {
                                            if target.len() == 4 {
                                                // Mark pending
                                                {
                                                    let mut state = app_state.write();
                                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                        tab.ground_station = GroundStationStatus::LogonPending(target.clone());
                                                    }
                                                }
                                                // Send logon request via NATS
                                                let msg = nats_client::build_logon_request(
                                                    &callsign,
                                                    &acars_address,
                                                    &target,
                                                );
                                                let clients = nats_clients.read();
                                                if let Some(client) = clients.get(&tab_id) {
                                                    let client = client.clone();
                                                    spawn(async move {
                                                        if let Err(e) = client.send_to_server(msg).await {
                                                            eprintln!("Erreur envoi logon: {e}");
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                    }
                                },
                                "LOGON"
                            }
                        }
                    },
                    GroundStationStatus::LogonPending(station) => rsx! {
                        div { class: "ground-status pending",
                            StatusBadge { status: "pending".to_string() }
                            span { class: "station-code", "{station}" }
                            button {
                                class: "cancel-btn",
                                onclick: move |_| {
                                    let mut state = app_state.write();
                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                        tab.ground_station = GroundStationStatus::Disconnected;
                                        tab.logon_input.clear();
                                    }
                                },
                                "{tr.cancel}"
                            }
                        }
                    },
                    GroundStationStatus::Connected(station) => rsx! {
                        div { class: "ground-status connected",
                            StatusBadge { status: "connected".to_string() }
                            span { class: "station-code", "{station}" }
                        }
                    },
                }
            }

            // Commands area (placeholder for future)
            div { class: "dcdu-commands",
                h3 { "{tr.commands}" }
                div { class: "commands-placeholder",
                    "{tr.no_commands_available}"
                }
            }

            // Messages area
            div { class: "dcdu-messages",
                h3 { "{tr.received_messages}" }
                MessageList { messages: messages }
            }
        }
    }
}
