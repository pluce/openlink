use dioxus::prelude::*;
use std::collections::HashSet;
use uuid::Uuid;

use openlink_models::{
    find_definition, constrained_closing_reply_ids, AcarsEndpointAddress, AcarsMessage,
    ArgType, CpdlcArgument, CpdlcConnectionPhase, CpdlcMessageType, CpdlcResponseIntent,
    FlightLevel, MessageDirection, MessageElement, OpenLinkMessage, closes_dialogue_response_elements,
    MESSAGE_REGISTRY,
};
use crate::state::{AppState, NatsClients, ReceivedMessage, AtcLinkedFlight};
use crate::i18n::{use_locale, t};
use crate::components::shared::{MessageList, StatusBadge};

fn arg_label(arg: ArgType) -> &'static str {
    match arg {
        ArgType::Level => "Level",
        ArgType::Speed => "Speed",
        ArgType::Time => "Time",
        ArgType::Position => "Position",
        ArgType::Direction => "Direction",
        ArgType::Degrees => "Degrees",
        ArgType::Distance => "Distance",
        ArgType::RouteClearance => "Route",
        ArgType::ProcedureName => "Procedure",
        ArgType::UnitName => "Unit",
        ArgType::FacilityDesignation => "Facility",
        ArgType::Frequency => "Frequency",
        ArgType::Code => "Code",
        ArgType::AtisCode => "ATIS",
        ArgType::ErrorInfo => "Error",
        ArgType::FreeText => "Text",
        ArgType::VerticalRate => "V/S",
        ArgType::Altimeter => "Altimeter",
        ArgType::LegType => "Leg",
        ArgType::PositionReport => "Pos report",
        ArgType::RemainingFuel => "Fuel",
        ArgType::PersonsOnBoard => "POB",
        ArgType::SpeedType => "Speed type",
        ArgType::DepartureClearance => "Dep clr",
    }
}

fn parse_arg(arg: ArgType, value: &str) -> Option<CpdlcArgument> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    Some(match arg {
        ArgType::Level => CpdlcArgument::Level(FlightLevel::new(v.parse().ok()?)),
        ArgType::Degrees => CpdlcArgument::Degrees(v.parse().ok()?),
        ArgType::Speed => CpdlcArgument::Speed(v.to_string()),
        ArgType::Time => CpdlcArgument::Time(v.to_string()),
        ArgType::Position => CpdlcArgument::Position(v.to_string()),
        ArgType::Direction => CpdlcArgument::Direction(v.to_string()),
        ArgType::Distance => CpdlcArgument::Distance(v.to_string()),
        ArgType::RouteClearance => CpdlcArgument::RouteClearance(v.to_string()),
        ArgType::ProcedureName => CpdlcArgument::ProcedureName(v.to_string()),
        ArgType::UnitName => CpdlcArgument::UnitName(v.to_string()),
        ArgType::FacilityDesignation => CpdlcArgument::FacilityDesignation(v.to_string()),
        ArgType::Frequency => CpdlcArgument::Frequency(v.to_string()),
        ArgType::Code => CpdlcArgument::Code(v.to_string()),
        ArgType::AtisCode => CpdlcArgument::AtisCode(v.to_string()),
        ArgType::ErrorInfo => CpdlcArgument::ErrorInfo(v.to_string()),
        ArgType::FreeText => CpdlcArgument::FreeText(v.to_string()),
        ArgType::VerticalRate => CpdlcArgument::VerticalRate(v.to_string()),
        ArgType::Altimeter => CpdlcArgument::Altimeter(v.to_string()),
        ArgType::LegType => CpdlcArgument::LegType(v.to_string()),
        ArgType::PositionReport => CpdlcArgument::PositionReport(v.to_string()),
        ArgType::RemainingFuel => CpdlcArgument::RemainingFuel(v.to_string()),
        ArgType::PersonsOnBoard => CpdlcArgument::PersonsOnBoard(v.to_string()),
        ArgType::SpeedType => CpdlcArgument::SpeedType(v.to_string()),
        ArgType::DepartureClearance => CpdlcArgument::DepartureClearance(v.to_string()),
    })
}

fn message_numeric_id(id: &str) -> u16 {
    id.chars()
        .skip_while(|c| !c.is_ascii_digit())
        .collect::<String>()
        .parse::<u16>()
        .unwrap_or(u16::MAX)
}

fn render_element(element: &MessageElement) -> String {
    find_definition(&element.id)
        .map(|def| def.render(&element.args))
        .unwrap_or_else(|| element.id.clone())
}

fn mark_dialogue_responded(
    mut app_state: Signal<AppState>,
    tab_id: Uuid,
    mrn: Option<u8>,
    elements: &[MessageElement],
) {
    let Some(mrn) = mrn else { return; };
    if !closes_dialogue_response_elements(elements) {
        return;
    }
    let mut state = app_state.write();
    if let Some(tab) = state.tab_mut_by_id(tab_id) {
        if let Some(message) = tab.messages.iter_mut().find(|m| m.min == Some(mrn) && !m.is_outgoing) {
            message.responded = true;
        }
    }
}

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

    let mut linked_flights: Vec<AtcLinkedFlight> = tab
        .atc_sessions
        .values()
        .filter_map(|session| {
            let callsign = session.aircraft.as_ref()?.to_string();
            let aircraft_address: AcarsEndpointAddress = session.aircraft_address.as_ref()?.clone();
            let phase = session
                .active_connection
                .as_ref()
                .map(|c| c.phase)
                .or_else(|| session.inactive_connection.as_ref().map(|c| c.phase))
                .unwrap_or(CpdlcConnectionPhase::Terminated);
            Some(AtcLinkedFlight {
                callsign: callsign.clone(),
                aircraft_callsign: callsign,
                aircraft_address,
                phase,
            })
        })
        .collect();
    linked_flights.sort_by(|a, b| a.callsign.cmp(&b.callsign));
    let selected_idx = tab.selected_flight_idx;
    let messages = tab.messages.clone();
    let callsign = tab.setup.callsign.clone();
    let acars_address = tab.setup.acars_address.clone();
    let conn_mgmt_open = tab.conn_mgmt_open;
    let atc_uplink_open = tab.atc_uplink_open;
    let contact_input = tab.contact_input.clone();
    let pending_conn_mgmt_cmd = tab.pending_conn_mgmt_cmd.clone();
    let cmd_arg_inputs = tab.cmd_arg_inputs.clone();
    let cmd_search_query = tab.cmd_search_query.clone();
    let compose_mode = tab.compose_mode;
    let compose_elements = tab.compose_elements.clone();
    let compose_mrn = tab.compose_mrn;
    let compose_send_after_param = tab.compose_send_after_param;
    let suggested_uplink_ids = tab.suggested_uplink_ids.clone();
    let has_compose_queue = compose_mode && (!compose_elements.is_empty() || compose_mrn.is_some());
    let uplink_defs: Vec<_> = MESSAGE_REGISTRY
        .iter()
        .filter(|d| d.direction == MessageDirection::Uplink)
        .collect();
    let mut uplink_defs = uplink_defs;
    uplink_defs.sort_by_key(|d| message_numeric_id(d.id));
    let suggested_uplink_set: HashSet<&str> = suggested_uplink_ids.iter().map(String::as_str).collect();
    let mut filtered_uplink_defs: Vec<_> = uplink_defs
        .iter()
        .copied()
        .filter(|d| {
            cmd_search_query.is_empty()
                || d.template
                    .to_uppercase()
                    .contains(&cmd_search_query)
        })
        .collect();
    if !suggested_uplink_set.is_empty() {
        filtered_uplink_defs.sort_by_key(|d| (!suggested_uplink_set.contains(d.id), message_numeric_id(d.id)));
    }
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
                                    EventHandler::new(move |(min, intent): (u8, CpdlcResponseIntent)| {
                                        let closes_dialogue = !matches!(intent, CpdlcResponseIntent::Standby);
                                        let elements = vec![MessageElement::new(intent.uplink_id(), vec![])];
                                        let clients = nats_clients.read();
                                        if let Some(client) = clients.get(&tab_id) {
                                            let msg = client.cpdlc_station_application(
                                                &callsign,
                                                &flight.aircraft_callsign,
                                                &flight.aircraft_address,
                                                elements,
                                                Some(min),
                                            );
                                            let client = client.clone();
                                            spawn(async move {
                                                if let Err(e) = client.send_to_server(msg).await {
                                                    eprintln!("Erreur envoi uplink: {e}");
                                                }
                                            });
                                        }
                                        crate::push_outgoing_message_to(&mut app_state.clone(), tab_id, intent.label(), Some(&flight.aircraft_callsign));
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
                                on_respond_compose: {
                                    EventHandler::new(move |(min, intent): (u8, CpdlcResponseIntent)| {
                                        let mut state = app_state.write();
                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                            tab.compose_mode = true;
                                            tab.compose_mrn = Some(min);
                                            tab.compose_elements.push(MessageElement::new(intent.uplink_id(), vec![]));
                                            tab.atc_uplink_open = true;
                                            tab.pending_uplink_cmd = None;
                                            tab.cmd_arg_inputs.clear();
                                            tab.cmd_search_query.clear();
                                            tab.compose_send_after_param = false;
                                        }
                                    })
                                },
                                on_suggested_reply: {
                                    EventHandler::new(move |min: u8| {
                                        let ids: Vec<String> = {
                                            let state = app_state.read();
                                            let Some(tab) = state.tab_by_id(tab_id) else { return; };
                                            let Some(msg) = tab.messages.iter().find(|m| m.min == Some(min) && !m.is_outgoing) else { return; };
                                            let Some(env) = msg.envelope.as_ref() else { return; };
                                            let Some(request_id) = (match &env.payload {
                                                OpenLinkMessage::Acars(acars) => match &acars.message {
                                                    AcarsMessage::CPDLC(cpdlc) => match &cpdlc.message {
                                                        CpdlcMessageType::Application(app) => app.elements.first().map(|e| e.id.as_str()),
                                                        _ => None,
                                                    },
                                                },
                                                _ => None,
                                            }) else { return; };
                                            constrained_closing_reply_ids(request_id)
                                                .iter()
                                                .map(|id| (*id).to_string())
                                                .collect()
                                        };
                                        if ids.is_empty() { return; }
                                        let mut state = app_state.write();
                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                            tab.compose_mode = true;
                                            tab.compose_mrn = Some(min);
                                            tab.compose_elements.clear();
                                            tab.atc_uplink_open = true;
                                            tab.pending_uplink_cmd = None;
                                            tab.cmd_arg_inputs.clear();
                                            tab.cmd_search_query.clear();
                                            tab.compose_send_after_param = true;
                                            tab.suggested_uplink_ids = ids;
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
                                                        // Send logon response + connection request via NATS
                                                        let clients = nats_clients.read();
                                                        if let Some(client) = clients.get(&tab_id) {
                                                            let logon_resp = client.cpdlc_logon_response(
                                                                &callsign,
                                                                &flight.aircraft_callsign,
                                                                &flight.aircraft_address,
                                                                true,
                                                            );
                                                            let conn_req = client.cpdlc_connection_request(
                                                                &callsign,
                                                                &flight.aircraft_callsign,
                                                                &flight.aircraft_address,
                                                            );
                                                            let client = client.clone();
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
                                                    move |_| {
                                                        let flight = flight.clone();
                                                        let callsign = callsign.clone();
                                                        // Send rejection via NATS
                                                        let clients = nats_clients.read();
                                                        if let Some(client) = clients.get(&tab_id) {
                                                            let logon_resp = client.cpdlc_logon_response(
                                                                &callsign,
                                                                &flight.aircraft_callsign,
                                                                &flight.aircraft_address,
                                                                false,
                                                            );
                                                            let client = client.clone();
                                                            spawn(async move {
                                                                let _ = client.send_to_server(logon_resp).await;
                                                            });
                                                        }
                                                        crate::push_outgoing_message_to(&mut app_state.clone(), tab_id, &format!("LOGON REJECT → {}", flight.callsign), Some(&flight.aircraft_callsign));
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
                                                            tab.pending_uplink_cmd = None;
                                                            tab.cmd_arg_inputs.clear();
                                                            if !tab.conn_mgmt_open {
                                                                tab.pending_conn_mgmt_cmd = None;
                                                                tab.contact_input.clear();
                                                            }
                                                        }
                                                    },
                                                    "{tr.conn_management} ▾"
                                                }

                                                if conn_mgmt_open {
                                                    div { class: "conn-mgmt-popover",
                                                        if let Some(ref cmd) = pending_conn_mgmt_cmd {
                                                            {
                                                                let cmd_label = match cmd.as_str() {
                                                                    "CONTACT" => format!("{}", tr.contact_station),
                                                                    "TRANSFER" => format!("{}", tr.transfer_to),
                                                                    _ => "COMMAND".to_string(),
                                                                };
                                                                let send_label = cmd.clone();
                                                                let has_target = contact_input.trim().len() == 4;
                                                                rsx! {
                                                                    form {
                                                                        class: "param-form",
                                                                        key: "conn-mgmt-{send_label}",
                                                                        onsubmit: move |evt| evt.prevent_default(),
                                                                        div { class: "param-form-header",
                                                                            span { class: "param-form-title", "{cmd_label}" }
                                                                            button {
                                                                                r#type: "button",
                                                                                class: "param-form-cancel",
                                                                                onclick: move |_| {
                                                                                    let mut state = app_state.write();
                                                                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                        tab.pending_conn_mgmt_cmd = None;
                                                                                        tab.contact_input.clear();
                                                                                    }
                                                                                },
                                                                                "✕"
                                                                            }
                                                                        }
                                                                        div { class: "param-form-body",
                                                                            span { class: "param-form-label", "STN" }
                                                                            input {
                                                                                r#type: "text",
                                                                                class: "param-form-input conn-mgmt-dest",
                                                                                autofocus: true,
                                                                                onmounted: move |element| async move {
                                                                                    let _ = element.data().set_focus(true).await;
                                                                                },
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
                                                                        }
                                                                        button {
                                                                            r#type: "submit",
                                                                            class: if has_target { "param-form-send" } else { "param-form-send disabled" },
                                                                            disabled: !has_target,
                                                                            onclick: {
                                                                                let flight = flight.clone();
                                                                                let callsign = callsign.clone();
                                                                                move |_| {
                                                                                    let (target, cmd) = {
                                                                                        let state = app_state.read();
                                                                                        let target = state.tab_by_id(tab_id)
                                                                                            .map(|t| t.contact_input.trim().to_string())
                                                                                            .unwrap_or_default();
                                                                                        let cmd = state.tab_by_id(tab_id)
                                                                                            .and_then(|t| t.pending_conn_mgmt_cmd.clone())
                                                                                            .unwrap_or_default();
                                                                                        (target, cmd)
                                                                                    };
                                                                                    if target.len() != 4 { return; }

                                                                                    let clients = nats_clients.read();
                                                                                    if let Some(client) = clients.get(&tab_id) {
                                                                                        let nda_msg = client.cpdlc_next_data_authority(
                                                                                            &callsign,
                                                                                            &flight.aircraft_callsign,
                                                                                            &flight.aircraft_address,
                                                                                            &target,
                                                                                        );
                                                                                        let maybe_second = match cmd.as_str() {
                                                                                            "CONTACT" => Some(client.cpdlc_contact_request(
                                                                                                &callsign,
                                                                                                &flight.aircraft_callsign,
                                                                                                &flight.aircraft_address,
                                                                                                &target,
                                                                                            )),
                                                                                            "TRANSFER" => Some(client.cpdlc_logon_forward(
                                                                                                &callsign,
                                                                                                &flight.aircraft_callsign,
                                                                                                &flight.aircraft_address,
                                                                                                &target,
                                                                                            )),
                                                                                            _ => None,
                                                                                        };
                                                                                        let client = client.clone();
                                                                                        spawn(async move {
                                                                                            let _ = client.send_to_server(nda_msg).await;
                                                                                            if let Some(second) = maybe_second {
                                                                                                let _ = client.send_to_server(second).await;
                                                                                            }
                                                                                        });
                                                                                    }

                                                                                    let text = match cmd.as_str() {
                                                                                        "CONTACT" => format!("CONTACT → {target}"),
                                                                                        "TRANSFER" => format!("TRANSFER → {target}"),
                                                                                        _ => format!("{} → {target}", send_label),
                                                                                    };
                                                                                    crate::push_outgoing_message_to(&mut app_state.clone(), tab_id, &text, Some(&flight.aircraft_callsign));

                                                                                    let mut state = app_state.write();
                                                                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                        tab.conn_mgmt_open = false;
                                                                                        tab.pending_conn_mgmt_cmd = None;
                                                                                        tab.contact_input.clear();
                                                                                    }
                                                                                }
                                                                            },
                                                                            "SEND"
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        } else {
                                                            div {
                                                                class: "conn-mgmt-option",
                                                                onclick: move |_| {
                                                                    let mut state = app_state.write();
                                                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                        tab.pending_conn_mgmt_cmd = Some("CONTACT".to_string());
                                                                    }
                                                                },
                                                                "{tr.contact_station}"
                                                            }
                                                            div {
                                                                class: "conn-mgmt-option",
                                                                onclick: move |_| {
                                                                    let mut state = app_state.write();
                                                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                        tab.pending_conn_mgmt_cmd = Some("TRANSFER".to_string());
                                                                    }
                                                                },
                                                                "{tr.transfer_to}"
                                                            }
                                                            div { class: "conn-mgmt-separator" }
                                                            div {
                                                                class: "conn-mgmt-option end-service",
                                                                onclick: {
                                                                    let flight = flight.clone();
                                                                    let callsign = callsign.clone();
                                                                    move |_| {
                                                                        let clients = nats_clients.read();
                                                                        if let Some(client) = clients.get(&tab_id) {
                                                                            let end_msg = client.cpdlc_end_service(
                                                                                &callsign,
                                                                                &flight.aircraft_callsign,
                                                                                &flight.aircraft_address,
                                                                            );
                                                                            let client = client.clone();
                                                                            spawn(async move {
                                                                                let _ = client.send_to_server(end_msg).await;
                                                                            });
                                                                        }
                                                                        crate::push_outgoing_message_to(&mut app_state.clone(), tab_id, &format!("END SERVICE → {}", flight.callsign), Some(&flight.aircraft_callsign));
                                                                        let mut state = app_state.write();
                                                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                            tab.conn_mgmt_open = false;
                                                                            tab.pending_conn_mgmt_cmd = None;
                                                                            tab.contact_input.clear();
                                                                        }
                                                                    }
                                                                },
                                                                "{tr.end_service}"
                                                            }
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
                                                            tab.pending_conn_mgmt_cmd = None;
                                                            tab.contact_input.clear();
                                                            if !tab.atc_uplink_open {
                                                                tab.pending_uplink_cmd = None;
                                                                tab.cmd_arg_inputs.clear();
                                                                tab.cmd_search_query.clear();
                                                                tab.compose_mode = false;
                                                                tab.compose_elements.clear();
                                                                tab.compose_mrn = None;
                                                                tab.compose_send_after_param = false;
                                                                tab.suggested_uplink_ids.clear();
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
                                                                if has_compose_queue {
                                                                    div { class: "compose-panel",
                                                                        div { class: "compose-header",
                                                                            span { class: "compose-title", "COMPOSER ({compose_elements.len()})" }
                                                                            button {
                                                                                class: "compose-clear",
                                                                                onclick: move |_| {
                                                                                    let mut state = app_state.write();
                                                                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                        tab.compose_mode = false;
                                                                                        tab.compose_elements.clear();
                                                                                        tab.compose_mrn = None;
                                                                                        tab.compose_send_after_param = false;
                                                                                    }
                                                                                },
                                                                                "CLEAR"
                                                                            }
                                                                        }
                                                                        if compose_elements.is_empty() {
                                                                            div { class: "compose-empty", "No element added" }
                                                                        } else {
                                                                            for (idx, e) in compose_elements.iter().enumerate() {
                                                                                div { class: "compose-item", "{idx + 1}. {render_element(e)}" }
                                                                            }
                                                                            button {
                                                                                class: "compose-send-all",
                                                                                onclick: {
                                                                                    let callsign = callsign.clone();
                                                                                    let flight = flight.clone();
                                                                                    let elements = compose_elements.clone();
                                                                                    move |_| {
                                                                                        if elements.is_empty() { return; }
                                                                                        let clients = nats_clients.read();
                                                                                        if let Some(client) = clients.get(&tab_id) {
                                                                                            let msg = client.cpdlc_station_application(
                                                                                                &callsign,
                                                                                                &flight.aircraft_callsign,
                                                                                                &flight.aircraft_address,
                                                                                                elements.clone(),
                                                                                                compose_mrn,
                                                                                            );
                                                                                            let client = client.clone();
                                                                                            spawn(async move {
                                                                                                if let Err(e) = client.send_to_server(msg).await {
                                                                                                    eprintln!("Erreur envoi uplink: {e}");
                                                                                                }
                                                                                            });
                                                                                        }
                                                                                        let text = elements
                                                                                            .iter()
                                                                                            .map(render_element)
                                                                                            .collect::<Vec<_>>()
                                                                                            .join(" / ");
                                                                                        crate::push_outgoing_message_to(&mut app_state.clone(), tab_id, &text, Some(&flight.aircraft_callsign));
                                                                                        mark_dialogue_responded(app_state.clone(), tab_id, compose_mrn, &elements);
                                                                                        let mut state = app_state.write();
                                                                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                            tab.atc_uplink_open = false;
                                                                                            tab.pending_uplink_cmd = None;
                                                                                            tab.cmd_arg_inputs.clear();
                                                                                            tab.cmd_search_query.clear();
                                                                                            tab.compose_mode = false;
                                                                                            tab.compose_elements.clear();
                                                                                            tab.compose_mrn = None;
                                                                                            tab.compose_send_after_param = false;
                                                                                            tab.suggested_uplink_ids.clear();
                                                                                        }
                                                                                    }
                                                                                },
                                                                                "SEND ALL"
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                div { class: "command-menu-scroll",
                                                                    if let Some(ref cmd) = pending_cmd {
                                                                        {
                                                                            if let Some(def) = find_definition(cmd) {
                                                                            let parsed_args: Option<Vec<CpdlcArgument>> = def
                                                                                .args
                                                                                .iter()
                                                                                .enumerate()
                                                                                .map(|(idx, arg_type)| {
                                                                                    cmd_arg_inputs
                                                                                        .get(idx)
                                                                                        .and_then(|v| parse_arg(*arg_type, v))
                                                                                })
                                                                                .collect();
                                                                            let has_valid_args = parsed_args.is_some();
                                                                            let flight = flight.clone();
                                                                            let callsign = callsign.clone();
                                                                            let cmd_id = cmd.clone();
                                                                            let rendered = parsed_args
                                                                                .as_ref()
                                                                                .map(|args| def.render(args))
                                                                                .unwrap_or_else(|| def.template.to_string());
                                                                            rsx! {
                                                                                form {
                                                                                    class: "param-form",
                                                                                    key: "uplink-{cmd_id}",
                                                                                    onsubmit: move |evt| evt.prevent_default(),
                                                                                    div { class: "param-form-header",
                                                                                        span { class: "param-form-title", "{def.template}" }
                                                                                        button {
                                                                                            r#type: "button",
                                                                                            class: "param-form-cancel",
                                                                                            onclick: move |_| {
                                                                                                let mut state = app_state.write();
                                                                                                if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                                    tab.pending_uplink_cmd = None;
                                                                                                    tab.cmd_arg_inputs.clear();
                                                                                                }
                                                                                            },
                                                                                            "✕"
                                                                                        }
                                                                                    }
                                                                                    for (idx, arg_type) in def.args.iter().enumerate() {
                                                                                        {
                                                                                            let value = cmd_arg_inputs.get(idx).cloned().unwrap_or_default();
                                                                                            let label = arg_label(*arg_type);
                                                                                            rsx! {
                                                                                                div { class: "param-form-body",
                                                                                                    span { class: "param-form-label", "{label}" }
                                                                                                    input {
                                                                                                        r#type: if matches!(arg_type, ArgType::Level | ArgType::Degrees) { "number" } else { "text" },
                                                                                                        class: "param-form-input",
                                                                                                        autofocus: idx == 0,
                                                                                                        onmounted: move |element| async move {
                                                                                                            if idx == 0 {
                                                                                                                let _ = element.data().set_focus(true).await;
                                                                                                            }
                                                                                                        },
                                                                                                        value: "{value}",
                                                                                                        oninput: move |evt: Event<FormData>| {
                                                                                                            let mut state = app_state.write();
                                                                                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                                                if tab.cmd_arg_inputs.len() <= idx {
                                                                                                                    tab.cmd_arg_inputs.resize(idx + 1, String::new());
                                                                                                                }
                                                                                                                tab.cmd_arg_inputs[idx] = evt.value();
                                                                                                            }
                                                                                                        },
                                                                                                    }
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    }
                                                                                    button {
                                                                                        r#type: "submit",
                                                                                        class: if has_valid_args { "param-form-send" } else { "param-form-send disabled" },
                                                                                        disabled: !has_valid_args,
                                                                                        onclick: move |_| {
                                                                                            let args = {
                                                                                                let state = app_state.read();
                                                                                                let Some(tab) = state.tab_by_id(tab_id) else { return; };
                                                                                                let Some(def) = find_definition(&cmd_id) else { return; };
                                                                                                def.args
                                                                                                    .iter()
                                                                                                    .enumerate()
                                                                                                    .map(|(idx, arg_type)| {
                                                                                                        tab.cmd_arg_inputs
                                                                                                            .get(idx)
                                                                                                            .and_then(|v| parse_arg(*arg_type, v))
                                                                                                    })
                                                                                                    .collect::<Option<Vec<_>>>()
                                                                                            };
                                                                                            if let Some(elements_args) = args {
                                                                                                if compose_mode {
                                                                                                    if compose_send_after_param {
                                                                                                        let (elements, mrn) = {
                                                                                                            let mut state = app_state.write();
                                                                                                            let Some(tab) = state.tab_mut_by_id(tab_id) else { return; };
                                                                                                            tab.compose_elements.push(MessageElement::new(&cmd_id, elements_args));
                                                                                                            let elements = tab.compose_elements.clone();
                                                                                                            let mrn = tab.compose_mrn;
                                                                                                            (elements, mrn)
                                                                                                        };
                                                                                                        let clients = nats_clients.read();
                                                                                                        if let Some(client) = clients.get(&tab_id) {
                                                                                                            let msg = client.cpdlc_station_application(
                                                                                                                &callsign,
                                                                                                                &flight.aircraft_callsign,
                                                                                                                &flight.aircraft_address,
                                                                                                                elements.clone(),
                                                                                                                mrn,
                                                                                                            );
                                                                                                            let client = client.clone();
                                                                                                            spawn(async move {
                                                                                                                if let Err(e) = client.send_to_server(msg).await {
                                                                                                                    eprintln!("Erreur envoi uplink: {e}");
                                                                                                                }
                                                                                                            });
                                                                                                        }
                                                                                                        let text = elements
                                                                                                            .iter()
                                                                                                            .map(render_element)
                                                                                                            .collect::<Vec<_>>()
                                                                                                            .join(" / ");
                                                                                                        crate::push_outgoing_message_to(&mut app_state.clone(), tab_id, &text, Some(&flight.aircraft_callsign));
                                                                                                        mark_dialogue_responded(app_state.clone(), tab_id, mrn, &elements);
                                                                                                        let mut state = app_state.write();
                                                                                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                                            tab.atc_uplink_open = false;
                                                                                                            tab.pending_uplink_cmd = None;
                                                                                                            tab.cmd_arg_inputs.clear();
                                                                                                            tab.cmd_search_query.clear();
                                                                                                            tab.compose_mode = false;
                                                                                                            tab.compose_elements.clear();
                                                                                                            tab.compose_mrn = None;
                                                                                                            tab.compose_send_after_param = false;
                                                                                                            tab.suggested_uplink_ids.clear();
                                                                                                        }
                                                                                                    } else {
                                                                                                        let mut state = app_state.write();
                                                                                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                                            tab.compose_elements.push(MessageElement::new(&cmd_id, elements_args));
                                                                                                            tab.pending_uplink_cmd = None;
                                                                                                            tab.cmd_arg_inputs.clear();
                                                                                                            tab.compose_send_after_param = false;
                                                                                                        }
                                                                                                    }
                                                                                                } else {
                                                                                                    let elements = vec![MessageElement::new(&cmd_id, elements_args)];
                                                                                                    let clients = nats_clients.read();
                                                                                                    if let Some(client) = clients.get(&tab_id) {
                                                                                                        let msg = client.cpdlc_station_application(
                                                                                                            &callsign,
                                                                                                            &flight.aircraft_callsign,
                                                                                                            &flight.aircraft_address,
                                                                                                            elements,
                                                                                                            None,
                                                                                                        );
                                                                                                        let client = client.clone();
                                                                                                        spawn(async move {
                                                                                                            if let Err(e) = client.send_to_server(msg).await {
                                                                                                                eprintln!("Erreur envoi uplink: {e}");
                                                                                                            }
                                                                                                        });
                                                                                                    }
                                                                                                    crate::push_outgoing_message_to(&mut app_state.clone(), tab_id, &rendered, Some(&flight.aircraft_callsign));
                                                                                                    let mut state = app_state.write();
                                                                                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                                        tab.atc_uplink_open = false;
                                                                                                        tab.pending_uplink_cmd = None;
                                                                                                        tab.cmd_arg_inputs.clear();
                                                                                                        tab.compose_send_after_param = false;
                                                                                                        tab.suggested_uplink_ids.clear();
                                                                                                    }
                                                                                                }
                                                                                            }
                                                                                        },
                                                                                        if compose_mode && compose_send_after_param { "SEND ALL" } else if compose_mode { "ADD" } else { "SEND" }
                                                                                    }
                                                                                }
                                                                            }
                                                                            } else {
                                                                                rsx! { div { class: "atc-uplink-option disabled", "UNKNOWN COMMAND" } }
                                                                            }
                                                                        }
                                                                    } else {
                                                                        input {
                                                                            r#type: "text",
                                                                            class: "command-search-input",
                                                                            placeholder: "Search command...",
                                                                            value: "{cmd_search_query}",
                                                                            autofocus: true,
                                                                            oninput: move |evt: Event<FormData>| {
                                                                                let mut state = app_state.write();
                                                                                if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                    tab.cmd_search_query = evt.value().to_uppercase();
                                                                                }
                                                                            },
                                                                        }
                                                                        for def in filtered_uplink_defs.iter() {
                                                                            {
                                                                                let def_id = def.id.to_string();
                                                                                let arg_count = def.args.len();
                                                                                let template = def.template.to_string();
                                                                                rsx! {
                                                                                    div { class: "command-option-row",
                                                                                    button {
                                                                                        class: "atc-uplink-option",
                                                                                        onclick: {
                                                                                            let def_id = def_id.clone();
                                                                                            let callsign = callsign.clone();
                                                                                            let flight = flight.clone();
                                                                                            move |_| {
                                                                                        if has_compose_queue && arg_count == 0 {
                                                                                            let (elements, mrn) = {
                                                                                                let mut state = app_state.write();
                                                                                                let Some(tab) = state.tab_mut_by_id(tab_id) else { return; };
                                                                                                tab.compose_elements.push(MessageElement::new(def_id.clone(), vec![]));
                                                                                                let elements = tab.compose_elements.clone();
                                                                                                let mrn = tab.compose_mrn;
                                                                                                (elements, mrn)
                                                                                            };
                                                                                            let clients = nats_clients.read();
                                                                                            if let Some(client) = clients.get(&tab_id) {
                                                                                                let msg = client.cpdlc_station_application(
                                                                                                    &callsign,
                                                                                                    &flight.aircraft_callsign,
                                                                                                    &flight.aircraft_address,
                                                                                                    elements.clone(),
                                                                                                    mrn,
                                                                                                );
                                                                                                let client = client.clone();
                                                                                                spawn(async move {
                                                                                                    if let Err(e) = client.send_to_server(msg).await {
                                                                                                        eprintln!("Erreur envoi uplink: {e}");
                                                                                                    }
                                                                                                });
                                                                                            }
                                                                                            let text = elements
                                                                                                .iter()
                                                                                                .map(render_element)
                                                                                                .collect::<Vec<_>>()
                                                                                                .join(" / ");
                                                                                            crate::push_outgoing_message_to(&mut app_state.clone(), tab_id, &text, Some(&flight.aircraft_callsign));
                                                                                            mark_dialogue_responded(app_state.clone(), tab_id, mrn, &elements);
                                                                                            let mut state = app_state.write();
                                                                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                                tab.atc_uplink_open = false;
                                                                                                tab.pending_uplink_cmd = None;
                                                                                                tab.cmd_arg_inputs.clear();
                                                                                                tab.cmd_search_query.clear();
                                                                                                tab.compose_mode = false;
                                                                                                tab.compose_elements.clear();
                                                                                                tab.compose_mrn = None;
                                                                                                tab.compose_send_after_param = false;
                                                                                                tab.suggested_uplink_ids.clear();
                                                                                            }
                                                                                        } else if !has_compose_queue && arg_count == 0 {
                                                                                            let elements = vec![MessageElement::new(def_id.clone(), vec![])];
                                                                                            let clients = nats_clients.read();
                                                                                            if let Some(client) = clients.get(&tab_id) {
                                                                                                let msg = client.cpdlc_station_application(
                                                                                                    &callsign,
                                                                                                    &flight.aircraft_callsign,
                                                                                                    &flight.aircraft_address,
                                                                                                    elements,
                                                                                                    None,
                                                                                                );
                                                                                                let client = client.clone();
                                                                                                spawn(async move {
                                                                                                    if let Err(e) = client.send_to_server(msg).await {
                                                                                                        eprintln!("Erreur envoi uplink: {e}");
                                                                                                    }
                                                                                                });
                                                                                            }
                                                                                            crate::push_outgoing_message_to(&mut app_state.clone(), tab_id, &template, Some(&flight.aircraft_callsign));
                                                                                            let mut state = app_state.write();
                                                                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                                tab.atc_uplink_open = false;
                                                                                                tab.pending_uplink_cmd = None;
                                                                                                tab.cmd_arg_inputs.clear();
                                                                                                tab.cmd_search_query.clear();
                                                                                                tab.suggested_uplink_ids.clear();
                                                                                            }
                                                                                        } else {
                                                                                            let mut state = app_state.write();
                                                                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                                tab.pending_uplink_cmd = Some(def_id.clone());
                                                                                                tab.cmd_arg_inputs = vec![String::new(); arg_count];
                                                                                                tab.cmd_search_query.clear();
                                                                                                tab.compose_send_after_param = has_compose_queue;
                                                                                            }
                                                                                        }
                                                                                            }
                                                                                        },
                                                                                        "{template}"
                                                                                    }
                                                                                    button {
                                                                                        class: "command-option-plus",
                                                                                        title: "Add to composition",
                                                                                        onclick: {
                                                                                            let def_id = def_id.clone();
                                                                                            move |_| {
                                                                                        let mut state = app_state.write();
                                                                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                            tab.compose_mode = true;
                                                                                            if arg_count == 0 {
                                                                                                tab.compose_elements.push(MessageElement::new(def_id.clone(), vec![]));
                                                                                            } else {
                                                                                                tab.pending_uplink_cmd = Some(def_id.clone());
                                                                                                tab.cmd_arg_inputs = vec![String::new(); arg_count];
                                                                                            }
                                                                                            tab.cmd_search_query.clear();
                                                                                            tab.compose_send_after_param = false;
                                                                                        }
                                                                                            }
                                                                                        },
                                                                                        "+"
                                                                                    }
                                                                                }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
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
