use dioxus::prelude::*;
use uuid::Uuid;

use openlink_models::{CpdlcConnectionPhase, MessageElement, CpdlcArgument, FlightLevel};
use crate::state::{AppState, NatsClients};
use crate::nats_client;
use crate::i18n::{use_locale, t};
use crate::components::shared::{MessageList, StatusBadge};

/// Derive the effective display state for the DCDU from the session view
/// and the optimistic `logon_pending` flag.
enum DcduConnectionState {
    Disconnected,
    Pending(String),
    Connected(String),
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

    // Derive connection display state from the authoritative session view,
    // falling back to the optimistic logon_pending when no session exists yet.
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
    } else if let Some(ref target) = tab.logon_pending {
        DcduConnectionState::Pending(target.clone())
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
    let fl_input = tab.fl_input.clone();

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
                                                        {
                                                            let mut state = app_state.write();
                                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                tab.logon_pending = Some(target.clone());
                                                            }
                                                        }
                                                        let msg = nats_client::build_logon_request(
                                                            &callsign,
                                                            &acars_address,
                                                            &target,
                                                        );
                                                        let target_display = target.clone();
                                                        let clients = nats_clients.read();
                                                        if let Some(client) = clients.get(&tab_id) {
                                                            let client = client.clone();
                                                            spawn(async move {
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
                                                tab.logon_pending = None;
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
                                                tab.fl_input.clear();
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
                                                if let Some(ref cmd) = pending_cmd {
                                                    // ── Parameter input form for the selected command ──
                                                    {
                                                        let cmd_label = match cmd.as_str() {
                                                            "DM6" => "REQUEST CLIMB TO FL",
                                                            "DM7" => "REQUEST DESCENT TO FL",
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
                                                                                tab.pending_downlink_cmd = None;
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
                                                                    let station_cs = station_callsign.clone();
                                                                    let callsign = callsign.clone();
                                                                    let acars_address = acars_address.clone();
                                                                    let cmd_id = cmd.clone();
                                                                    let fl_input_c = fl_input.clone();
                                                                    rsx! {
                                                                        button {
                                                                            class: if has_fl { "param-form-send" } else { "param-form-send disabled" },
                                                                            disabled: !has_fl,
                                                                            onclick: move |_| {
                                                                                if let (Some(fl), Some(ref station)) = (fl_input_c.parse::<u16>().ok(), &station_cs) {
                                                                                    let elements = vec![MessageElement::new(
                                                                                        &cmd_id,
                                                                                        vec![CpdlcArgument::Level(FlightLevel::new(fl))],
                                                                                    )];
                                                                                    let msg = nats_client::build_downlink_message(
                                                                                        &callsign, &acars_address, station.as_str(), elements, None,
                                                                                    );
                                                                                    let clients = nats_clients.read();
                                                                                    if let Some(client) = clients.get(&tab_id) {
                                                                                        let client = client.clone();
                                                                                        spawn(async move {
                                                                                            if let Err(e) = client.send_to_server(msg).await {
                                                                                                eprintln!("Erreur envoi downlink: {e}");
                                                                                            }
                                                                                        });
                                                                                    }
                                                                                    let label = match cmd_id.as_str() {
                                                                                        "DM6" => format!("REQUEST CLIMB TO FL{fl}"),
                                                                                        "DM7" => format!("REQUEST DESCENT TO FL{fl}"),
                                                                                        _ => format!("{} FL{fl}", cmd_id),
                                                                                    };
                                                                                    crate::push_outgoing_message(&mut app_state.clone(), tab_id, &label);
                                                                                }
                                                                                let mut state = app_state.write();
                                                                                if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                    tab.pilot_downlink_open = false;
                                                                                    tab.pending_downlink_cmd = None;
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
                                                        class: "dcdu-downlink-option",
                                                        onclick: move |_| {
                                                            let mut state = app_state.write();
                                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                tab.pending_downlink_cmd = Some("DM6".to_string());
                                                            }
                                                        },
                                                        "REQUEST CLIMB TO FL..."
                                                    }
                                                    div {
                                                        class: "dcdu-downlink-option",
                                                        onclick: move |_| {
                                                            let mut state = app_state.write();
                                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                tab.pending_downlink_cmd = Some("DM7".to_string());
                                                            }
                                                        },
                                                        "REQUEST DESCENT TO FL..."
                                                    }
                                                    div { class: "dcdu-downlink-separator" }
                                                    div { class: "dcdu-downlink-option disabled", "REQUEST DIRECT TO..." }
                                                    div { class: "dcdu-downlink-option disabled", "REQUEST HEADING..." }
                                                    div { class: "dcdu-downlink-option disabled", "REQUEST SPEED..." }
                                                    div { class: "dcdu-downlink-separator" }
                                                    div { class: "dcdu-downlink-option disabled", "POSITION REPORT" }
                                                    div { class: "dcdu-downlink-option disabled", "FREE TEXT..." }
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
                        EventHandler::new(move |(min, msg_id): (u8, String)| {
                            if let Some(ref station) = station_callsign {
                                // Map DM code to human-readable label and whether it closes the dialogue
                                let (label, closes_dialogue) = match msg_id.as_str() {
                                    "DM0" => ("WILCO", true),
                                    "DM1" => ("UNABLE", true),
                                    "DM2" => ("STANDBY", false),
                                    "DM3" => ("ROGER", true),
                                    "DM4" => ("AFFIRM", true),
                                    "DM5" => ("NEGATIVE", true),
                                    other => (other, false),
                                };
                                let elements = vec![MessageElement::new(&msg_id, vec![])];
                                let msg = nats_client::build_downlink_message(
                                    &callsign, &acars_address, station.as_str(), elements, Some(min),
                                );
                                let clients = nats_clients.read();
                                if let Some(client) = clients.get(&tab_id) {
                                    let client = client.clone();
                                    spawn(async move {
                                        if let Err(e) = client.send_to_server(msg).await {
                                            eprintln!("Erreur envoi downlink: {e}");
                                        }
                                    });
                                }
                                crate::push_outgoing_message(&mut app_state.clone(), tab_id, label);
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
                }
            }
        }
    }
}
