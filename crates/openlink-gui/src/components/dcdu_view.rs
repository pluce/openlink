use dioxus::prelude::*;
use uuid::Uuid;

use openlink_models::{
    find_definition, ArgType, CpdlcArgument, CpdlcConnectionPhase, CpdlcResponseIntent,
    FlightLevel, MessageDirection, MessageElement, closes_dialogue_response_elements,
    MESSAGE_REGISTRY,
};
use crate::state::{AppState, NatsClients};
use crate::i18n::{use_locale, t};
use crate::components::shared::{MessageList, StatusBadge};

/// Derive the effective display state for the DCDU from the authoritative session view.
enum DcduConnectionState {
    Disconnected,
    Pending(String),
    Connected(String),
}

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

    // Derive connection display state from the authoritative session view.
    let conn_state = if let Some(ref session) = tab.session {
        match &session.active_connection {
            Some(conn) => match conn.phase {
                CpdlcConnectionPhase::LogonPending | CpdlcConnectionPhase::LoggedOn => {
                    DcduConnectionState::Pending(conn.peer.to_string())
                }
                CpdlcConnectionPhase::Connected => {
                    DcduConnectionState::Connected(conn.peer.to_string())
                }
                CpdlcConnectionPhase::Terminated => DcduConnectionState::Disconnected,
            },
            None => DcduConnectionState::Disconnected,
        }
    } else {
        DcduConnectionState::Disconnected
    };

    let logon_input = tab.logon_input.clone();
    let messages = tab.messages.clone();
    let callsign = tab.setup.callsign.clone();
    let acars_address: openlink_models::AcarsEndpointAddress = tab.setup.acars_address.clone().into();
    let inactive_conn = tab.session.as_ref().and_then(|s| s.inactive_connection.clone());
    let nda = tab.session.as_ref().and_then(|s| s.next_data_authority.clone());
    let pilot_downlink_open = tab.pilot_downlink_open;
    let cmd_arg_inputs = tab.cmd_arg_inputs.clone();
    let cmd_search_query = tab.cmd_search_query.clone();
    let compose_mode = tab.compose_mode;
    let compose_elements = tab.compose_elements.clone();
    let compose_mrn = tab.compose_mrn;
    let compose_send_after_param = tab.compose_send_after_param;
    let has_compose_queue = compose_mode && (!compose_elements.is_empty() || compose_mrn.is_some());
    let downlink_defs: Vec<_> = MESSAGE_REGISTRY
        .iter()
        .filter(|d| d.direction == MessageDirection::Downlink)
        .collect();
    let mut downlink_defs = downlink_defs;
    downlink_defs.sort_by_key(|d| message_numeric_id(d.id));
    let filtered_downlink_defs: Vec<_> = downlink_defs
        .iter()
        .copied()
        .filter(|d| {
            cmd_search_query.is_empty()
                || d.template
                    .to_uppercase()
                    .contains(&cmd_search_query)
        })
        .collect();

    // Find connected station callsign from session
    let station_callsign = tab.session.as_ref()
        .and_then(|s| s.active_connection.as_ref())
        .map(|c| c.peer.clone());
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
                div { class: "gs-bar",
                    // Left: active connection
                    div { class: "gs-active",
                        h3 { "{tr.ground_station}" }
                        match &conn_state {
                            DcduConnectionState::Disconnected => rsx! {
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
                                                        let target_display = target.clone();
                                                        let clients = nats_clients.read();
                                                        if let Some(client) = clients.get(&tab_id) {
                                                            let client = client.clone();
                                                            let callsign_for_send = callsign.clone();
                                                            let acars_for_send = acars_address.clone();
                                                            let target_for_send = target.clone();
                                                            spawn(async move {
                                                                let msg = client.cpdlc_logon_request(
                                                                    &callsign_for_send,
                                                                    &acars_for_send,
                                                                    &target_for_send,
                                                                );
                                                                if let Err(e) = client.send_to_server(msg).await {
                                                                    eprintln!("Erreur envoi logon: {e}");
                                                                }
                                                            });
                                                        }
                                                        crate::push_outgoing_message(&mut app_state.clone(), tab_id, &format!("LOGON REQUEST → {target_display}"));
                                                    }
                                                }
                                            }
                                        },
                                        "LOGON"
                                    }
                                }
                            },
                            DcduConnectionState::Pending(station) => rsx! {
                                div { class: "ground-status pending",
                                    StatusBadge { status: "pending".to_string() }
                                    span { class: "station-code", "{station}" }
                                    button {
                                        class: "cancel-btn",
                                        onclick: move |_| {
                                            let mut state = app_state.write();
                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                tab.session = None;
                                                tab.logon_input.clear();
                                            }
                                        },
                                        "{tr.cancel}"
                                    }
                                }
                            },
                            DcduConnectionState::Connected(station) => rsx! {
                                div { class: "ground-status connected",
                                    StatusBadge { status: "connected".to_string() }
                                    span { class: "station-code", "{station}" }
                                }
                            },
                        }
                    }

                    // Right: inactive connection (discreet, dimmed)
                    if inactive_conn.is_some() || nda.is_some() {
                        div { class: "gs-inactive",
                            if let Some(ref nda_cs) = nda {
                                span { class: "gs-nda", "NDA: {nda_cs}" }
                            }
                            if let Some(ref conn) = inactive_conn {
                                div { class: "gs-inactive-detail",
                                    StatusBadge {
                                        status: match conn.phase {
                                            CpdlcConnectionPhase::LogonPending => "pending".to_string(),
                                            CpdlcConnectionPhase::LoggedOn => "logon".to_string(),
                                            CpdlcConnectionPhase::Connected => "connected".to_string(),
                                            CpdlcConnectionPhase::Terminated => "terminated".to_string(),
                                        }
                                    }
                                    span { class: "gs-inactive-peer", "{conn.peer}" }
                                }
                            }
                        }
                    }
                }
            }

            // Commands area
            div { class: "dcdu-commands",
                match &conn_state {
                    DcduConnectionState::Connected(_station) => rsx! {
                        div { class: "dcdu-downlink-bar",
                            h3 { "{tr.commands}" }
                            div { class: "dcdu-downlink-wrapper",
                                button {
                                    class: "cmd-dcdu-downlink",
                                    onclick: move |_| {
                                        let mut state = app_state.write();
                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                            tab.pilot_downlink_open = !tab.pilot_downlink_open;
                                            if !tab.pilot_downlink_open {
                                                tab.pending_downlink_cmd = None;
                                                tab.cmd_arg_inputs.clear();
                                                tab.cmd_search_query.clear();
                                                tab.compose_mode = false;
                                                tab.compose_elements.clear();
                                                tab.compose_mrn = None;
                                                tab.compose_send_after_param = false;
                                            }
                                        }
                                    },
                                    "{tr.pilot_downlink} ▾"
                                }

                                if pilot_downlink_open {
                                    {
                                        let pending_cmd = {
                                            let state = app_state.read();
                                            state.tab_by_id(tab_id).and_then(|t| t.pending_downlink_cmd.clone())
                                        };
                                        rsx! {
                                            div { class: "dcdu-downlink-popover",
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
                                                                    let acars_address = acars_address.clone();
                                                                    let station_cs = station_callsign.clone();
                                                                    let elements = compose_elements.clone();
                                                                    move |_| {
                                                                        let Some(ref station) = station_cs else { return; };
                                                                        if elements.is_empty() { return; }
                                                                        let clients = nats_clients.read();
                                                                        if let Some(client) = clients.get(&tab_id) {
                                                                            let msg = client.cpdlc_aircraft_application(
                                                                                &callsign,
                                                                                &acars_address,
                                                                                station.as_str(),
                                                                                elements.clone(),
                                                                                compose_mrn,
                                                                            );
                                                                            let client = client.clone();
                                                                            spawn(async move {
                                                                                if let Err(e) = client.send_to_server(msg).await {
                                                                                    eprintln!("Erreur envoi downlink: {e}");
                                                                                }
                                                                            });
                                                                        }
                                                                        let text = elements
                                                                            .iter()
                                                                            .map(render_element)
                                                                            .collect::<Vec<_>>()
                                                                            .join(" / ");
                                                                        crate::push_outgoing_message(&mut app_state.clone(), tab_id, &text);
                                                                        mark_dialogue_responded(app_state.clone(), tab_id, compose_mrn, &elements);
                                                                        let mut state = app_state.write();
                                                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                            tab.pilot_downlink_open = false;
                                                                            tab.pending_downlink_cmd = None;
                                                                            tab.cmd_arg_inputs.clear();
                                                                            tab.cmd_search_query.clear();
                                                                            tab.compose_mode = false;
                                                                            tab.compose_elements.clear();
                                                                            tab.compose_mrn = None;
                                                                            tab.compose_send_after_param = false;
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
                                                            let station_cs = station_callsign.clone();
                                                            let callsign = callsign.clone();
                                                            let acars_address = acars_address.clone();
                                                            let cmd_id = cmd.clone();
                                                            let rendered = parsed_args
                                                                .as_ref()
                                                                .map(|args| def.render(args))
                                                                .unwrap_or_else(|| def.template.to_string());

                                                            rsx! {
                                                                form {
                                                                    class: "param-form",
                                                                    key: "downlink-{cmd_id}",
                                                                    onsubmit: move |evt| evt.prevent_default(),
                                                                    div { class: "param-form-header",
                                                                        span { class: "param-form-title", "{def.template}" }
                                                                        button {
                                                                            r#type: "button",
                                                                            class: "param-form-cancel",
                                                                            onclick: move |_| {
                                                                                let mut state = app_state.write();
                                                                                if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                    tab.pending_downlink_cmd = None;
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
                                                                            let Some(ref station) = station_cs else { return; };
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
                                                                                            let msg = client.cpdlc_aircraft_application(
                                                                                                &callsign,
                                                                                                &acars_address,
                                                                                                station.as_str(),
                                                                                                elements.clone(),
                                                                                                mrn,
                                                                                            );
                                                                                            let client = client.clone();
                                                                                            spawn(async move {
                                                                                                if let Err(e) = client.send_to_server(msg).await {
                                                                                                    eprintln!("Erreur envoi downlink: {e}");
                                                                                                }
                                                                                            });
                                                                                        }
                                                                                        let text = elements
                                                                                            .iter()
                                                                                            .map(render_element)
                                                                                            .collect::<Vec<_>>()
                                                                                            .join(" / ");
                                                                                        crate::push_outgoing_message(&mut app_state.clone(), tab_id, &text);
                                                                                        mark_dialogue_responded(app_state.clone(), tab_id, mrn, &elements);
                                                                                        let mut state = app_state.write();
                                                                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                            tab.pilot_downlink_open = false;
                                                                                            tab.pending_downlink_cmd = None;
                                                                                            tab.cmd_arg_inputs.clear();
                                                                                            tab.cmd_search_query.clear();
                                                                                            tab.compose_mode = false;
                                                                                            tab.compose_elements.clear();
                                                                                            tab.compose_mrn = None;
                                                                                            tab.compose_send_after_param = false;
                                                                                        }
                                                                                    } else {
                                                                                        let mut state = app_state.write();
                                                                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                            tab.compose_elements.push(MessageElement::new(&cmd_id, elements_args));
                                                                                            tab.pending_downlink_cmd = None;
                                                                                            tab.cmd_arg_inputs.clear();
                                                                                            tab.compose_send_after_param = false;
                                                                                        }
                                                                                    }
                                                                                } else {
                                                                                    let elements = vec![MessageElement::new(&cmd_id, elements_args)];
                                                                                    let clients = nats_clients.read();
                                                                                    if let Some(client) = clients.get(&tab_id) {
                                                                                        let msg = client.cpdlc_aircraft_application(
                                                                                            &callsign,
                                                                                            &acars_address,
                                                                                            station.as_str(),
                                                                                            elements,
                                                                                            None,
                                                                                        );
                                                                                        let client = client.clone();
                                                                                        spawn(async move {
                                                                                            if let Err(e) = client.send_to_server(msg).await {
                                                                                                eprintln!("Erreur envoi downlink: {e}");
                                                                                            }
                                                                                        });
                                                                                    }
                                                                                    crate::push_outgoing_message(&mut app_state.clone(), tab_id, &rendered);
                                                                                    let mut state = app_state.write();
                                                                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                        tab.pilot_downlink_open = false;
                                                                                        tab.pending_downlink_cmd = None;
                                                                                        tab.cmd_arg_inputs.clear();
                                                                                        tab.compose_send_after_param = false;
                                                                                    }
                                                                                }
                                                                            }
                                                                        },
                                                                        if compose_mode && compose_send_after_param { "SEND ALL" } else if compose_mode { "ADD" } else { "SEND" }
                                                                    }
                                                                }
                                                            }
                                                            } else {
                                                                rsx! { div { class: "dcdu-downlink-option disabled", "UNKNOWN COMMAND" } }
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
                                                        for def in filtered_downlink_defs.iter() {
                                                            {
                                                                let def_id = def.id.to_string();
                                                                let arg_count = def.args.len();
                                                                let template = def.template.to_string();
                                                                rsx! {
                                                                    div { class: "command-option-row",
                                                                    button {
                                                                        class: "dcdu-downlink-option",
                                                                        onclick: {
                                                                            let def_id = def_id.clone();
                                                                            let callsign = callsign.clone();
                                                                            let acars_address = acars_address.clone();
                                                                            let station_cs = station_callsign.clone();
                                                                            move |_| {
                                                                        let Some(ref station) = station_cs else { return; };
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
                                                                                let msg = client.cpdlc_aircraft_application(
                                                                                    &callsign,
                                                                                    &acars_address,
                                                                                    station.as_str(),
                                                                                    elements.clone(),
                                                                                    mrn,
                                                                                );
                                                                                let client = client.clone();
                                                                                spawn(async move {
                                                                                    if let Err(e) = client.send_to_server(msg).await {
                                                                                        eprintln!("Erreur envoi downlink: {e}");
                                                                                    }
                                                                                });
                                                                            }
                                                                            let text = elements
                                                                                .iter()
                                                                                .map(render_element)
                                                                                .collect::<Vec<_>>()
                                                                                .join(" / ");
                                                                            crate::push_outgoing_message(&mut app_state.clone(), tab_id, &text);
                                                                            mark_dialogue_responded(app_state.clone(), tab_id, mrn, &elements);
                                                                            let mut state = app_state.write();
                                                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                tab.pilot_downlink_open = false;
                                                                                tab.pending_downlink_cmd = None;
                                                                                tab.cmd_arg_inputs.clear();
                                                                                tab.cmd_search_query.clear();
                                                                                tab.compose_mode = false;
                                                                                tab.compose_elements.clear();
                                                                                tab.compose_mrn = None;
                                                                                tab.compose_send_after_param = false;
                                                                            }
                                                                        } else if !has_compose_queue && arg_count == 0 {
                                                                            let elements = vec![MessageElement::new(def_id.clone(), vec![])];
                                                                            let clients = nats_clients.read();
                                                                            if let Some(client) = clients.get(&tab_id) {
                                                                                let msg = client.cpdlc_aircraft_application(
                                                                                    &callsign,
                                                                                    &acars_address,
                                                                                    station.as_str(),
                                                                                    elements,
                                                                                    None,
                                                                                );
                                                                                let client = client.clone();
                                                                                spawn(async move {
                                                                                    if let Err(e) = client.send_to_server(msg).await {
                                                                                        eprintln!("Erreur envoi downlink: {e}");
                                                                                    }
                                                                                });
                                                                            }
                                                                            crate::push_outgoing_message(&mut app_state.clone(), tab_id, &template);
                                                                            let mut state = app_state.write();
                                                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                tab.pilot_downlink_open = false;
                                                                                tab.pending_downlink_cmd = None;
                                                                                tab.cmd_arg_inputs.clear();
                                                                                tab.cmd_search_query.clear();
                                                                            }
                                                                        } else {
                                                                            let mut state = app_state.write();
                                                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                tab.pending_downlink_cmd = Some(def_id.clone());
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
                                                                                tab.pending_downlink_cmd = Some(def_id.clone());
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
                    _ => rsx! {
                        h3 { "{tr.commands}" }
                        div { class: "commands-placeholder",
                            "{tr.no_commands_available}"
                        }
                    },
                }
            }

            // Messages area
            div { class: "dcdu-messages",
                h3 { "{tr.received_messages}" }
                MessageList {
                    messages: messages,
                    on_respond: {
                        let callsign = callsign.clone();
                        let acars_address = acars_address.clone();
                        let station_callsign = station_callsign.clone();
                        EventHandler::new(move |(min, intent): (u8, CpdlcResponseIntent)| {
                            if let Some(ref station) = station_callsign {
                                let closes_dialogue = !matches!(intent, CpdlcResponseIntent::Standby);
                                let elements = vec![MessageElement::new(intent.downlink_id(), vec![])];
                                let clients = nats_clients.read();
                                if let Some(client) = clients.get(&tab_id) {
                                    let msg = client.cpdlc_aircraft_application(
                                        &callsign, &acars_address, station.as_str(), elements, Some(min),
                                    );
                                    let client = client.clone();
                                    spawn(async move {
                                        if let Err(e) = client.send_to_server(msg).await {
                                            eprintln!("Erreur envoi downlink: {e}");
                                        }
                                    });
                                }
                                crate::push_outgoing_message(&mut app_state.clone(), tab_id, intent.label());
                                // Close the dialogue: hide response buttons on the original message
                                if closes_dialogue {
                                    let mut state = app_state.write();
                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                        if let Some(m) = tab.messages.iter_mut().find(|m| m.min == Some(min) && !m.is_outgoing) {
                                            m.responded = true;
                                        }
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
                                tab.compose_elements.push(MessageElement::new(intent.downlink_id(), vec![]));
                                tab.pilot_downlink_open = true;
                                tab.pending_downlink_cmd = None;
                                tab.cmd_arg_inputs.clear();
                                tab.cmd_search_query.clear();
                                tab.compose_send_after_param = false;
                            }
                        })
                    },
                }
            }
        }
    }
}
