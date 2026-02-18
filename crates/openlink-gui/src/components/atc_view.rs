use dioxus::prelude::*;
use uuid::Uuid;

use crate::state::{AppState, NatsClients, AtcFlightLinkStatus, ReceivedMessage};
use crate::nats_client;
use crate::i18n::{use_locale, t};
use crate::components::shared::{MessageList, StatusBadge};

/// ATC controller view
#[component]
pub fn AtcView(
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

    let linked_flights = tab.linked_flights.clone();
    let selected_idx = tab.selected_flight_idx;
    let messages = tab.messages.clone();
    let callsign = tab.setup.callsign.clone();
    let acars_address = tab.setup.acars_address.clone();
    drop(state);

    // Filter messages for the selected flight
    let selected_flight = selected_idx.and_then(|idx| linked_flights.get(idx).cloned());
    let filtered_messages: Vec<ReceivedMessage> = if let Some(ref flight) = selected_flight {
        messages
            .iter()
            .filter(|m| {
                m.from_callsign
                    .as_ref()
                    .map(|c| c == &flight.callsign || c == &flight.aircraft_callsign)
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    } else {
        Vec::new()
    };

    rsx! {
        div { class: "atc-container",
            // Header
            div { class: "atc-header",
                div { class: "atc-title",
                    span { class: "atc-label", "ATC" }
                    span { class: "atc-callsign", "{callsign}" }
                }
            }

            div { class: "atc-body",
                // Left panel: flight list
                div { class: "atc-flights-panel",
                    h3 { "{tr.flights}" }
                    div { class: "flights-list",
                        if linked_flights.is_empty() {
                            div { class: "no-flights", "{tr.no_flights_connected}" }
                        }
                        for (idx, flight) in linked_flights.iter().enumerate() {
                            div {
                                class: {
                                    let base = "flight-item";
                                    let status_class = match flight.status {
                                        AtcFlightLinkStatus::LogonRequested => "logon-requested",
                                        AtcFlightLinkStatus::Connected => "flight-connected",
                                    };
                                    let selected = if selected_idx == Some(idx) { "selected" } else { "" };
                                    format!("{base} {status_class} {selected}")
                                },
                                onclick: {
                                    let idx = idx;
                                    move |_| {
                                        let mut state = app_state.write();
                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                            tab.selected_flight_idx = Some(idx);
                                        }
                                    }
                                },
                                StatusBadge {
                                    status: match flight.status {
                                        AtcFlightLinkStatus::LogonRequested => "logon".to_string(),
                                        AtcFlightLinkStatus::Connected => "connected".to_string(),
                                    }
                                }
                                span { class: "flight-callsign", "{flight.callsign}" }
                            }
                        }
                    }
                }

                // Right panel: messages & commands for selected flight
                div { class: "atc-detail-panel",
                    if let Some(ref flight) = selected_flight {
                        div { class: "atc-detail",
                            h3 { "{tr.messages_for} — {flight.callsign}" }
                            MessageList { messages: filtered_messages }

                            // Commands for the selected flight
                            div { class: "atc-commands",
                                h3 { "{tr.actions}" }
                                match flight.status {
                                    AtcFlightLinkStatus::LogonRequested => rsx! {
                                        div { class: "command-buttons",
                                            button {
                                                class: "cmd-accept",
                                                onclick: {
                                                    let flight = flight.clone();
                                                    let callsign = callsign.clone();
                                                    let acars_address = acars_address.clone();
                                                    move |_| {
                                                        let flight = flight.clone();
                                                        let callsign = callsign.clone();
                                                        let _acars_address = acars_address.clone();
                                                        // Accept logon
                                                        let mut state = app_state.write();
                                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                            if let Some(idx) = selected_idx {
                                                                if let Some(f) = tab.linked_flights.get_mut(idx) {
                                                                    f.status = AtcFlightLinkStatus::Connected;
                                                                }
                                                            }
                                                        }
                                                        drop(state);
                                                        // Send logon response + connection request via NATS
                                                        let clients = nats_clients.read();
                                                        if let Some(client) = clients.get(&tab_id) {
                                                            let client = client.clone();
                                                            let logon_resp = nats_client::build_logon_response(
                                                                &callsign,
                                                                &flight.aircraft_callsign,
                                                                &flight.aircraft_address,
                                                                true,
                                                            );
                                                            let conn_req = nats_client::build_connection_request(
                                                                &callsign,
                                                                &flight.aircraft_callsign,
                                                                &flight.aircraft_address,
                                                            );
                                                            spawn(async move {
                                                                let _ = client.send_to_server(logon_resp).await;
                                                                let _ = client.send_to_server(conn_req).await;
                                                            });
                                                        }
                                                    }
                                                },
                                                "{tr.accept_logon}"
                                            }
                                            button {
                                                class: "cmd-reject",
                                                onclick: {
                                                    let flight = flight.clone();
                                                    let callsign = callsign.clone();
                                                    let selected_idx = selected_idx;
                                                    move |_| {
                                                        let flight = flight.clone();
                                                        let callsign = callsign.clone();
                                                        // Send rejection via NATS
                                                        let clients = nats_clients.read();
                                                        if let Some(client) = clients.get(&tab_id) {
                                                            let client = client.clone();
                                                            let logon_resp = nats_client::build_logon_response(
                                                                &callsign,
                                                                &flight.aircraft_callsign,
                                                                &flight.aircraft_address,
                                                                false,
                                                            );
                                                            spawn(async move {
                                                                let _ = client.send_to_server(logon_resp).await;
                                                            });
                                                        }
                                                        drop(clients);
                                                        // Remove from list
                                                        let mut state = app_state.write();
                                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                            if let Some(idx) = selected_idx {
                                                                tab.linked_flights.remove(idx);
                                                                tab.selected_flight_idx = None;
                                                            }
                                                        }
                                                    }
                                                },
                                                "{tr.reject}"
                                            }
                                        }
                                    },
                                    AtcFlightLinkStatus::Connected => rsx! {
                                        div { class: "command-buttons",
                                            span { class: "connected-info", "{tr.flight_connected}" }
                                        }
                                    },
                                }
                            }
                        }
                    } else {
                        div { class: "no-selection",
                            "{tr.select_flight}"
                        }
                    }
                }
            }
        }
    }
}
