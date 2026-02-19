use dioxus::prelude::*;
use uuid::Uuid;

use openlink_models::{CpdlcConnectionPhase, MessageElement, CpdlcArgument, FlightLevel};
use crate::state::{AppState, NatsClients, ReceivedMessage};
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
    let conn_mgmt_open = tab.conn_mgmt_open;
    let atc_uplink_open = tab.atc_uplink_open;
    let contact_input = tab.contact_input.clone();
    let fl_input = tab.fl_input.clone();
    drop(state);

    // Filter messages for the selected flight
    let selected_flight = selected_idx.and_then(|idx| linked_flights.get(idx).cloned());
    let filtered_messages: Vec<ReceivedMessage> = if let Some(ref flight) = selected_flight {
        messages
            .iter()
            .filter(|m| {
                // Incoming: from_callsign matches the flight
                let from_match = m.from_callsign
                    .as_ref()
                    .map(|c| c == &flight.callsign || c == &flight.aircraft_callsign)
                    .unwrap_or(false);
                // Outgoing: to_callsign matches the flight
                let to_match = m.is_outgoing && m.to_callsign
                    .as_ref()
                    .map(|c| c == &flight.callsign || c == &flight.aircraft_callsign)
                    .unwrap_or(false);
                from_match || to_match
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
                                    let status_class = match flight.phase {
                                        CpdlcConnectionPhase::LogonPending | CpdlcConnectionPhase::LoggedOn => "logon-requested",
                                        CpdlcConnectionPhase::Connected => "flight-connected",
                                        CpdlcConnectionPhase::Terminated => "flight-terminated",
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
                                    status: match flight.phase {
                                        CpdlcConnectionPhase::LogonPending | CpdlcConnectionPhase::LoggedOn => "logon".to_string(),
                                        CpdlcConnectionPhase::Connected => "connected".to_string(),
                                        CpdlcConnectionPhase::Terminated => "terminated".to_string(),
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
                            MessageList {
                                messages: filtered_messages.clone(),
                                on_respond: {
                                    let flight = flight.clone();
                                    let callsign = callsign.clone();
                                    EventHandler::new(move |(min, msg_id): (u8, String)| {
                                        // ATC receives DM* codes from shared component, remap to UM equivalents
                                        let (um_id, label, closes_dialogue) = match msg_id.as_str() {
                                            "DM0" => ("UM3", "ROGER", true),
                                            "DM3" => ("UM3", "ROGER", true),
                                            "DM2" => ("UM1", "STANDBY", false),
                                            "DM1" => ("UM0", "UNABLE", true),
                                            "DM4" => ("UM3", "ROGER", true),
                                            "DM5" => ("UM0", "UNABLE", true),
                                            other => (other, other, false),
                                        };
                                        let elements = vec![MessageElement::new(um_id, vec![])];
                                        let msg = nats_client::build_uplink_message(
                                            &callsign, &flight.aircraft_callsign, &flight.aircraft_address, elements, Some(min),
                                        );
                                        let clients = nats_clients.read();
                                        if let Some(client) = clients.get(&tab_id) {
                                            let client = client.clone();
                                            spawn(async move {
                                                if let Err(e) = client.send_to_server(msg).await {
                                                    eprintln!("Erreur envoi uplink: {e}");
                                                }
                                            });
                                        }
                                        crate::push_outgoing_message_to(&mut app_state.clone(), tab_id, label, Some(&flight.aircraft_callsign));
                                        // Close the dialogue: hide response buttons on the original message
                                        if closes_dialogue {
                                            let mut state = app_state.write();
                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                if let Some(m) = tab.messages.iter_mut().find(|m| m.min == Some(min) && !m.is_outgoing) {
                                                    m.responded = true;
                                                }
                                            }
                                        }
                                    })
                                },
                            }

                            // Commands for the selected flight
                            div { class: "atc-commands",
                                h3 { "{tr.actions}" }
                                match flight.phase {
                                    CpdlcConnectionPhase::LogonPending | CpdlcConnectionPhase::LoggedOn => rsx! {
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
                                                                    f.phase = CpdlcConnectionPhase::Connected;
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
                                                        crate::push_outgoing_message_to(&mut app_state.clone(), tab_id, &format!("LOGON ACCEPT + CONNECT → {}", flight.callsign), Some(&flight.aircraft_callsign));
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
                                                        crate::push_outgoing_message_to(&mut app_state.clone(), tab_id, &format!("LOGON REJECT → {}", flight.callsign), Some(&flight.aircraft_callsign));
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
                                    CpdlcConnectionPhase::Connected => rsx! {
                                        div { class: "command-buttons",
                                            div { class: "conn-mgmt-wrapper",
                                                span { class: "connected-info", "{tr.flight_connected}" }
                                                button {
                                                    class: "cmd-conn-mgmt",
                                                    onclick: move |_| {
                                                        let mut state = app_state.write();
                                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                            tab.conn_mgmt_open = !tab.conn_mgmt_open;
                                                            tab.atc_uplink_open = false;
                                                            tab.contact_input.clear();
                                                        }
                                                    },
                                                    "{tr.conn_management} ▾"
                                                }

                                                if conn_mgmt_open {
                                                    div { class: "conn-mgmt-popover",
                                                        // CONTACT option
                                                        div {
                                                            class: "conn-mgmt-option",
                                                            "📡 {tr.contact_station}"
                                                        }
                                                        div { class: "conn-mgmt-input",
                                                            input {
                                                                r#type: "text",
                                                                class: "conn-mgmt-dest",
                                                                maxlength: "4",
                                                                placeholder: "{tr.target_station_placeholder}",
                                                                value: "{contact_input}",
                                                                oninput: move |evt: Event<FormData>| {
                                                                    let mut state = app_state.write();
                                                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                        tab.contact_input = evt.value().to_uppercase();
                                                                    }
                                                                },
                                                            }
                                                            button {
                                                                class: "conn-mgmt-send",
                                                                disabled: contact_input.trim().is_empty(),
                                                                onclick: {
                                                                    let flight = flight.clone();
                                                                    let callsign = callsign.clone();
                                                                    move |_| {
                                                                        let target = {
                                                                            let state = app_state.read();
                                                                            state.tab_by_id(tab_id)
                                                                                .map(|t| t.contact_input.trim().to_string())
                                                                                .unwrap_or_default()
                                                                        };
                                                                        if target.is_empty() { return; }
                                                                        let clients = nats_clients.read();
                                                                        if let Some(client) = clients.get(&tab_id) {
                                                                            let client = client.clone();
                                                                            let nda_msg = nats_client::build_next_data_authority(
                                                                                &callsign,
                                                                                &flight.aircraft_callsign,
                                                                                &flight.aircraft_address,
                                                                                &target,
                                                                            );
                                                                            let contact_msg = nats_client::build_contact_request(
                                                                                &callsign,
                                                                                &flight.aircraft_callsign,
                                                                                &flight.aircraft_address,
                                                                                &target,
                                                                            );
                                                                            spawn(async move {
                                                                                let _ = client.send_to_server(nda_msg).await;
                                                                                let _ = client.send_to_server(contact_msg).await;
                                                                            });
                                                                        }
                                                                        crate::push_outgoing_message_to(&mut app_state.clone(), tab_id, &format!("CONTACT → {target}"), Some(&flight.aircraft_callsign));
                                                                        // Close popover
                                                                        let mut state = app_state.write();
                                                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                            tab.conn_mgmt_open = false;
                                                                            tab.contact_input.clear();
                                                                        }
                                                                    }
                                                                },
                                                                "CONTACT"
                                                            }
                                                        }

                                                        // TRANSFER option
                                                        div {
                                                            class: "conn-mgmt-option",
                                                            "↗ {tr.transfer_to}"
                                                        }
                                                        div { class: "conn-mgmt-input",
                                                            input {
                                                                r#type: "text",
                                                                class: "conn-mgmt-dest",
                                                                maxlength: "4",
                                                                placeholder: "{tr.target_station_placeholder}",
                                                                value: "{contact_input}",
                                                                oninput: move |evt: Event<FormData>| {
                                                                    let mut state = app_state.write();
                                                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                        tab.contact_input = evt.value().to_uppercase();
                                                                    }
                                                                },
                                                            }
                                                            button {
                                                                class: "conn-mgmt-send",
                                                                disabled: contact_input.trim().is_empty(),
                                                                onclick: {
                                                                    let flight = flight.clone();
                                                                    let callsign = callsign.clone();
                                                                    move |_| {
                                                                        let target = {
                                                                            let state = app_state.read();
                                                                            state.tab_by_id(tab_id)
                                                                                .map(|t| t.contact_input.trim().to_string())
                                                                                .unwrap_or_default()
                                                                        };
                                                                        if target.is_empty() { return; }
                                                                        let clients = nats_clients.read();
                                                                        if let Some(client) = clients.get(&tab_id) {
                                                                            let client = client.clone();
                                                                            let nda_msg = nats_client::build_next_data_authority(
                                                                                &callsign,
                                                                                &flight.aircraft_callsign,
                                                                                &flight.aircraft_address,
                                                                                &target,
                                                                            );
                                                                            let forward_msg = nats_client::build_logon_forward(
                                                                                &callsign,
                                                                                &flight.aircraft_callsign,
                                                                                &flight.aircraft_address,
                                                                                &target,
                                                                            );
                                                                            spawn(async move {
                                                                                let _ = client.send_to_server(nda_msg).await;
                                                                                let _ = client.send_to_server(forward_msg).await;
                                                                            });
                                                                        }
                                                                        crate::push_outgoing_message_to(&mut app_state.clone(), tab_id, &format!("TRANSFER → {target}"), Some(&flight.aircraft_callsign));
                                                                        // Close popover
                                                                        let mut state = app_state.write();
                                                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                            tab.conn_mgmt_open = false;
                                                                            tab.contact_input.clear();
                                                                        }
                                                                    }
                                                                },
                                                                "TRANSFER"
                                                            }
                                                        }

                                                        // Separator
                                                        div { class: "conn-mgmt-separator" }

                                                        // END SERVICE option
                                                        div {
                                                            class: "conn-mgmt-option end-service",
                                                            onclick: {
                                                                let flight = flight.clone();
                                                                let callsign = callsign.clone();
                                                                move |_| {
                                                                    let clients = nats_clients.read();
                                                                    if let Some(client) = clients.get(&tab_id) {
                                                                        let client = client.clone();
                                                                        let end_msg = nats_client::build_end_service(
                                                                            &callsign,
                                                                            &flight.aircraft_callsign,
                                                                            &flight.aircraft_address,
                                                                        );
                                                                        spawn(async move {
                                                                            let _ = client.send_to_server(end_msg).await;
                                                                        });
                                                                    }
                                                                    crate::push_outgoing_message_to(&mut app_state.clone(), tab_id, &format!("END SERVICE → {}", flight.callsign), Some(&flight.aircraft_callsign));
                                                                    // Close popover
                                                                    let mut state = app_state.write();
                                                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                        tab.conn_mgmt_open = false;
                                                                    }
                                                                }
                                                            },
                                                            "⏹ {tr.end_service}"
                                                        }
                                                    }
                                                }
                                            }

                                            // UPLINK menu
                                            div { class: "atc-uplink-wrapper",
                                                button {
                                                    class: "cmd-atc-uplink",
                                                    onclick: move |_| {
                                                        let mut state = app_state.write();
                                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                            tab.atc_uplink_open = !tab.atc_uplink_open;
                                                            tab.conn_mgmt_open = false;
                                                            if !tab.atc_uplink_open {
                                                                tab.pending_uplink_cmd = None;
                                                                tab.fl_input.clear();
                                                            }
                                                        }
                                                    },
                                                    "{tr.atc_uplink} ▾"
                                                }

                                                if atc_uplink_open {
                                                    {
                                                        let pending_cmd = {
                                                            let state = app_state.read();
                                                            state.tab_by_id(tab_id).and_then(|t| t.pending_uplink_cmd.clone())
                                                        };
                                                        rsx! {
                                                            div { class: "atc-uplink-popover",
                                                                if let Some(ref cmd) = pending_cmd {
                                                                    // ── Parameter input form for the selected command ──
                                                                    {
                                                                        let cmd_label = match cmd.as_str() {
                                                                            "UM20" => "CLIMB TO FL",
                                                                            "UM23" => "DESCEND TO FL",
                                                                            _ => "COMMAND",
                                                                        };
                                                                        rsx! {
                                                                            div { class: "param-form",
                                                                                div { class: "param-form-header",
                                                                                    span { class: "param-form-title", "{cmd_label}" }
                                                                                    button {
                                                                                        class: "param-form-cancel",
                                                                                        onclick: move |_| {
                                                                                            let mut state = app_state.write();
                                                                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                                tab.pending_uplink_cmd = None;
                                                                                                tab.fl_input.clear();
                                                                                            }
                                                                                        },
                                                                                        "✕"
                                                                                    }
                                                                                }
                                                                                div { class: "param-form-body",
                                                                                    span { class: "param-form-label", "FL" }
                                                                                    input {
                                                                                        r#type: "number",
                                                                                        class: "param-form-input",
                                                                                        placeholder: "350",
                                                                                        min: "0",
                                                                                        max: "600",
                                                                                        value: "{fl_input}",
                                                                                        oninput: move |evt: Event<FormData>| {
                                                                                            let mut state = app_state.write();
                                                                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                                tab.fl_input = evt.value();
                                                                                            }
                                                                                        },
                                                                                    }
                                                                                }
                                                                                {
                                                                                    let fl_val: Option<u16> = fl_input.parse().ok();
                                                                                    let has_fl = fl_val.is_some();
                                                                                    let flight = flight.clone();
                                                                                    let callsign = callsign.clone();
                                                                                    let cmd_id = cmd.clone();
                                                                                    let fl_input_c = fl_input.clone();
                                                                                    rsx! {
                                                                                        button {
                                                                                            class: if has_fl { "param-form-send" } else { "param-form-send disabled" },
                                                                                            disabled: !has_fl,
                                                                                            onclick: move |_| {
                                                                                                if let Some(fl) = fl_input_c.parse::<u16>().ok() {
                                                                                                    let elements = vec![MessageElement::new(
                                                                                                        &cmd_id,
                                                                                                        vec![CpdlcArgument::Level(FlightLevel::new(fl))],
                                                                                                    )];
                                                                                                    let msg = nats_client::build_uplink_message(
                                                                                                        &callsign, &flight.aircraft_callsign, &flight.aircraft_address, elements, None,
                                                                                                    );
                                                                                                    let clients = nats_clients.read();
                                                                                                    if let Some(client) = clients.get(&tab_id) {
                                                                                                        let client = client.clone();
                                                                                                        spawn(async move {
                                                                                                            if let Err(e) = client.send_to_server(msg).await {
                                                                                                                eprintln!("Erreur envoi uplink: {e}");
                                                                                                            }
                                                                                                        });
                                                                                                    }
                                                                                                    let label = match cmd_id.as_str() {
                                                                                                        "UM20" => format!("CLIMB TO FL{fl}"),
                                                                                                        "UM23" => format!("DESCEND TO FL{fl}"),
                                                                                                        _ => format!("{} FL{fl}", cmd_id),
                                                                                                    };
                                                                                                    crate::push_outgoing_message_to(&mut app_state.clone(), tab_id, &label, Some(&flight.aircraft_callsign));
                                                                                                }
                                                                                                let mut state = app_state.write();
                                                                                                if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                                    tab.atc_uplink_open = false;
                                                                                                    tab.pending_uplink_cmd = None;
                                                                                                    tab.fl_input.clear();
                                                                                                }
                                                                                            },
                                                                                            "SEND"
                                                                                        }
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                } else {
                                                                    // ── Command menu ──
                                                                    div {
                                                                        class: "atc-uplink-option",
                                                                        onclick: move |_| {
                                                                            let mut state = app_state.write();
                                                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                tab.pending_uplink_cmd = Some("UM20".to_string());
                                                                            }
                                                                        },
                                                                        "CLIMB TO FL..."
                                                                    }
                                                                    div {
                                                                        class: "atc-uplink-option",
                                                                        onclick: move |_| {
                                                                            let mut state = app_state.write();
                                                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                tab.pending_uplink_cmd = Some("UM23".to_string());
                                                                            }
                                                                        },
                                                                        "DESCEND TO FL..."
                                                                    }
                                                                    div { class: "atc-uplink-separator" }
                                                                    div { class: "atc-uplink-option disabled", "TURN LEFT/RIGHT HDG..." }
                                                                    div { class: "atc-uplink-option disabled", "PROCEED DIRECT TO..." }
                                                                    div { class: "atc-uplink-separator" }
                                                                    div { class: "atc-uplink-option disabled", "SQUAWK..." }
                                                                    div { class: "atc-uplink-option disabled", "MONITOR..." }
                                                                    div { class: "atc-uplink-option disabled", "REPORT..." }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    CpdlcConnectionPhase::Terminated => rsx! {
                                        div { class: "command-buttons",
                                            span { class: "terminated-info", "TERMINATED" }
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
