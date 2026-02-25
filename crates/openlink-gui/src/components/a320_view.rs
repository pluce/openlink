use std::collections::{HashMap, HashSet};
use std::time::Duration;

use dioxus::prelude::*;
use openlink_models::{
    find_definition, ArgType, CpdlcArgument, CpdlcResponseIntent, FlightLevel, MessageElement,
};
use openlink_sdk::{
    choose_short_response_intents, closes_dialogue_response_elements, response_attr_to_intents,
};
use uuid::Uuid;

use crate::i18n::{t, use_locale};
use crate::state::{AppState, NatsClients, ReceivedMessage};

#[derive(Clone, Copy, PartialEq, Eq)]
enum McduPage {
    AtcMenu,
    Notification,
    LatReq,
    VertReq,
    OtherReq,
    Text,
}

#[derive(Clone, Default)]
struct ResponseGrid {
    top_left: Option<CpdlcResponseIntent>,
    top_right: Option<CpdlcResponseIntent>,
    bot_left: Option<CpdlcResponseIntent>,
    bot_right: Option<CpdlcResponseIntent>,
}

fn dcdu_label(label: &str) -> &str {
    match label {
        "STANDBY" => "STBY",
        other => other,
    }
}

fn arrange_responses(intents: &[CpdlcResponseIntent]) -> ResponseGrid {
    let mut grid = ResponseGrid::default();
    if intents.is_empty() {
        return grid;
    }

    let find = |needle: CpdlcResponseIntent| intents.iter().find(|i| **i == needle).cloned();

    let wilco = find(CpdlcResponseIntent::Wilco);
    let unable = find(CpdlcResponseIntent::Unable);
    let standby = find(CpdlcResponseIntent::Standby);
    let roger = find(CpdlcResponseIntent::Roger);
    let affirm = find(CpdlcResponseIntent::Affirm);
    let negative = find(CpdlcResponseIntent::Negative);

    if wilco.is_some() {
        grid.top_left = unable;
        grid.top_right = standby;
        grid.bot_right = wilco;
        return grid;
    }
    if affirm.is_some() {
        grid.top_left = negative;
        grid.top_right = standby;
        grid.bot_right = affirm;
        return grid;
    }
    if roger.is_some() {
        grid.top_right = standby;
        grid.bot_right = roger;
        return grid;
    }

    for (slot, intent) in intents.iter().enumerate() {
        match slot {
            0 => grid.bot_right = Some(*intent),
            1 => grid.top_right = Some(*intent),
            2 => grid.top_left = Some(*intent),
            3 => grid.bot_left = Some(*intent),
            _ => break,
        }
    }
    grid
}

fn render_element(element: &MessageElement) -> String {
    find_definition(&element.id)
        .map(|d| d.render(&element.args))
        .unwrap_or_else(|| element.id.clone())
}

fn parse_arg(arg: ArgType, value: &str) -> Option<CpdlcArgument> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    Some(match arg {
        ArgType::Level => {
            let raw = v.to_uppercase().replace("FL", "");
            CpdlcArgument::Level(FlightLevel::new(raw.parse().ok()?))
        }
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

fn root_visible_messages(messages: &[ReceivedMessage]) -> Vec<ReceivedMessage> {
    messages
        .iter()
        .filter(|m| {
            if m.mrn.is_none() {
                return true;
            }
            let is_linked_to_uplink = messages.iter().any(|u| {
                !u.is_outgoing && u.min.is_some() && u.min == m.mrn && u.timestamp != m.timestamp
            });
            !is_linked_to_uplink
        })
        .cloned()
        .collect()
}

fn response_intents_for_message(msg: &ReceivedMessage) -> Vec<CpdlcResponseIntent> {
    if let Some(attr) = msg.response_attr {
        return response_attr_to_intents(attr);
    }
    msg.envelope
        .as_ref()
        .and_then(|env| {
            if let openlink_models::OpenLinkMessage::Acars(acars) = &env.payload {
                let openlink_models::AcarsMessage::CPDLC(cpdlc) = &acars.message;
                if let openlink_models::CpdlcMessageType::Application(app) = &cpdlc.message {
                    return Some(choose_short_response_intents(&app.elements));
                }
            }
            None
        })
        .unwrap_or_default()
}

fn dcdu_wrap_24x4(text: &str) -> [String; 4] {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for raw_word in text.split_whitespace() {
        let mut word = raw_word.to_uppercase();
        while word.chars().count() > 24 {
            let head: String = word.chars().take(24).collect();
            if !current.is_empty() {
                lines.push(current.trim_end().to_string());
                current.clear();
            }
            lines.push(head);
            word = word.chars().skip(24).collect();
            if lines.len() >= 4 {
                while lines.len() < 4 {
                    lines.push(String::new());
                }
                return [
                    format!("{:<24}", lines.first().cloned().unwrap_or_default()),
                    format!("{:<24}", lines.get(1).cloned().unwrap_or_default()),
                    format!("{:<24}", lines.get(2).cloned().unwrap_or_default()),
                    format!("{:<24}", lines.get(3).cloned().unwrap_or_default()),
                ];
            }
        }

        let candidate_len = if current.is_empty() {
            word.chars().count()
        } else {
            current.chars().count() + 1 + word.chars().count()
        };

        if candidate_len > 24 {
            lines.push(current.trim_end().to_string());
            current = word;
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(&word);
        }

        if lines.len() >= 4 {
            break;
        }
    }

    if lines.len() < 4 && !current.is_empty() {
        lines.push(current.trim_end().to_string());
    }

    while lines.len() < 4 {
        lines.push(String::new());
    }

    [
        format!("{:<24}", lines[0]),
        format!("{:<24}", lines[1]),
        format!("{:<24}", lines[2]),
        format!("{:<24}", lines[3]),
    ]
}

#[component]
pub fn A320View(
    tab_id: Uuid,
    app_state: Signal<AppState>,
    nats_clients: Signal<NatsClients>,
) -> Element {
    let locale = use_locale();
    let tr = t(*locale.read());

    let mut page = use_signal(|| McduPage::AtcMenu);
    let mut scratchpad = use_signal(String::new);
    let mut field_values = use_signal(HashMap::<String, String>::new);
    let mut pending_elements = use_signal(Vec::<MessageElement>::new);
    let atc_center = use_signal(String::new);
    let view_index = use_signal(|| -1_i32);
    let mut draft_elements = use_signal(Vec::<MessageElement>::new);
    let mut draft_mrn = use_signal(|| None::<u8>);
    let mut sending = use_signal(|| false);
    let dcdu_blanking = use_signal(|| false);
    let mut active_dcdu_lsk = use_signal(|| None::<String>);
    let mut dcdu_status_line = use_signal(String::new);
    let mut stby_sent_mins = use_signal(HashSet::<u8>::new);

    let state = app_state.read();
    let tab = match state.tab_by_id(tab_id) {
        Some(t) => t,
        None => return rsx! { p { "{tr.tab_not_found}" } },
    };

    let callsign = tab.setup.callsign.clone();
    let acars_address: openlink_models::AcarsEndpointAddress = tab.setup.acars_address.clone().into();
    let station_callsign = tab
        .session
        .as_ref()
        .and_then(|s| s.active_connection.as_ref())
        .map(|c| c.peer.clone());

    let all_messages = tab.messages.clone();
    drop(state);

    let visible = root_visible_messages(&all_messages);
    let effective_idx = {
        let idx = *view_index.read();
        if visible.is_empty() {
            -1
        } else if idx >= 0 && (idx as usize) < visible.len() {
            idx
        } else {
            (visible.len() as i32) - 1
        }
    };
    let current = if effective_idx >= 0 {
        visible.get(effective_idx as usize).cloned()
    } else {
        None
    };

    let linked: Vec<ReceivedMessage> = if let Some(ref cur) = current {
        if !cur.is_outgoing {
            all_messages
                .iter()
                .filter(|m| m.mrn.is_some() && m.mrn == cur.min)
                .cloned()
                .collect()
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    let current_min = current.as_ref().and_then(|m| m.min);
    let intents = current
        .as_ref()
        .map(response_intents_for_message)
        .unwrap_or_default();
    let effective_intents = if let Some(min) = current_min {
        if stby_sent_mins.read().contains(&min) {
            intents
                .iter()
                .copied()
                .filter(|i| !matches!(i, CpdlcResponseIntent::Standby))
                .collect::<Vec<_>>()
        } else {
            intents.clone()
        }
    } else {
        intents.clone()
    };
    let show_responses = current
        .as_ref()
        .map(|m| !m.is_outgoing && !m.responded && draft_elements.read().is_empty())
        .unwrap_or(false)
        && !effective_intents.is_empty();
    let response_grid = if show_responses {
        arrange_responses(&effective_intents)
    } else {
        ResponseGrid::default()
    };
    let current_open_min = current
        .as_ref()
        .and_then(|m| if !m.is_outgoing && !m.responded { m.min } else { None });

    let has_pending = !pending_elements.read().is_empty();
    let current_page = *page.read();
    let lower_status = if !dcdu_status_line.read().is_empty() {
        dcdu_status_line.read().clone()
    } else if *sending.read() {
        "SENDING".to_string()
    } else if current.is_some() {
        "SENT".to_string()
    } else {
        "".to_string()
    };
    let active_lsk_label = active_dcdu_lsk.read().clone();

    rsx! {
        div { class: "a320-native-container",
            div { class: "a320-native-cockpit",
                // DCDU (top)
                div { class: "a320-native-dcdu",
                    span { class: "a320-screw tl", "⊕" }
                    span { class: "a320-screw tr", "⊕" }
                    span { class: "a320-screw bl", "⊕" }
                    span { class: "a320-screw br", "⊕" }

                    div { class: "a320-native-dcdu-shell",
                        div { class: "a320-native-dcdu-buttons left",
                            button { class: "a320-dcdu-btn", "BRT" }
                            button { class: "a320-dcdu-btn", "DIM" }
                            div { class: "a320-dcdu-spacer" }
                            button {
                                class: "a320-dcdu-btn",
                                disabled: effective_idx <= 0,
                                onclick: move |_| {
                                    let idx = *view_index.read();
                                    if idx > 0 {
                                        let mut blanking = dcdu_blanking;
                                        let mut vi = view_index;
                                        spawn(async move {
                                            blanking.set(true);
                                            tokio::time::sleep(Duration::from_millis(90)).await;
                                            vi.set(idx - 1);
                                            blanking.set(false);
                                        });
                                    }
                                },
                                "MSG-"
                            }
                            button {
                                class: "a320-dcdu-btn",
                                disabled: visible.is_empty() || effective_idx >= (visible.len() as i32 - 1),
                                onclick: move |_| {
                                    let idx = *view_index.read();
                                    let max = (visible.len() as i32) - 1;
                                    if idx < max {
                                        let mut blanking = dcdu_blanking;
                                        let mut vi = view_index;
                                        spawn(async move {
                                            blanking.set(true);
                                            tokio::time::sleep(Duration::from_millis(90)).await;
                                            vi.set(idx + 1);
                                            blanking.set(false);
                                        });
                                    }
                                },
                                "MSG+"
                            }
                            div { class: "a320-dcdu-spacer" }
                            button {
                                class: if response_grid.top_left.map(|i| i.label().to_string()) == active_lsk_label {
                                    "a320-dcdu-btn action active"
                                } else {
                                    "a320-dcdu-btn action"
                                },
                                disabled: response_grid.top_left.is_none() || current_min.is_none(),
                                onclick: {
                                    let callsign = callsign.clone();
                                    let acars_address = acars_address.clone();
                                    let station_callsign = station_callsign.clone();
                                    let intent = response_grid.top_left;
                                    let min = current_min;
                                    move |_| {
                                        let Some(intent) = intent else { return; };
                                        let Some(station) = station_callsign.clone() else { return; };
                                        let Some(min) = min else { return; };
                                        let label = intent.label().to_string();
                                        let downlink_id = intent.downlink_id().to_string();
                                        let clients = nats_clients.read();
                                        if let Some(client) = clients.get(&tab_id) {
                                            let msg = client.cpdlc_aircraft_application(
                                                &callsign,
                                                &acars_address,
                                                station.as_str(),
                                                vec![MessageElement::new(downlink_id.clone(), vec![])],
                                                Some(min),
                                            );
                                            let client = client.clone();
                                            spawn(async move {
                                                let _ = client.send_to_server(msg).await;
                                            });
                                        }
                                        crate::push_outgoing_message(&mut app_state.clone(), tab_id, &label);
                                        active_dcdu_lsk.set(Some(label.clone()));
                                        if matches!(intent, CpdlcResponseIntent::Standby) {
                                            stby_sent_mins.write().insert(min);
                                            dcdu_status_line.set("STBY SENT".to_string());
                                            {
                                                let mut status = dcdu_status_line;
                                                spawn(async move {
                                                    tokio::time::sleep(Duration::from_secs(3)).await;
                                                    status.set(String::new());
                                                });
                                            }
                                        } else {
                                            stby_sent_mins.write().remove(&min);
                                            dcdu_status_line.set("SENT".to_string());
                                        }
                                        {
                                            let mut lsk = active_dcdu_lsk;
                                            spawn(async move {
                                                tokio::time::sleep(Duration::from_millis(600)).await;
                                                lsk.set(None);
                                            });
                                        }
                                        if !matches!(intent, CpdlcResponseIntent::Standby) {
                                            let mut s = app_state.write();
                                            if let Some(tab) = s.tab_mut_by_id(tab_id) {
                                                if let Some(m) = tab.messages.iter_mut().find(|m| m.min == Some(min) && !m.is_outgoing) {
                                                    m.responded = true;
                                                }
                                            }
                                        }
                                    }
                                },
                                "―"
                            }
                            button {
                                class: if response_grid.bot_left.map(|i| i.label().to_string()) == active_lsk_label {
                                    "a320-dcdu-btn action active"
                                } else {
                                    "a320-dcdu-btn action"
                                },
                                disabled: response_grid.bot_left.is_none() || current_min.is_none(),
                                onclick: {
                                    let callsign = callsign.clone();
                                    let acars_address = acars_address.clone();
                                    let station_callsign = station_callsign.clone();
                                    let intent = response_grid.bot_left;
                                    let min = current_min;
                                    move |_| {
                                        let Some(intent) = intent else { return; };
                                        let Some(station) = station_callsign.clone() else { return; };
                                        let Some(min) = min else { return; };
                                        let label = intent.label().to_string();
                                        let downlink_id = intent.downlink_id().to_string();
                                        let clients = nats_clients.read();
                                        if let Some(client) = clients.get(&tab_id) {
                                            let msg = client.cpdlc_aircraft_application(
                                                &callsign,
                                                &acars_address,
                                                station.as_str(),
                                                vec![MessageElement::new(downlink_id.clone(), vec![])],
                                                Some(min),
                                            );
                                            let client = client.clone();
                                            spawn(async move {
                                                let _ = client.send_to_server(msg).await;
                                            });
                                        }
                                        crate::push_outgoing_message(&mut app_state.clone(), tab_id, &label);
                                        active_dcdu_lsk.set(Some(label.clone()));
                                        if matches!(intent, CpdlcResponseIntent::Standby) {
                                            stby_sent_mins.write().insert(min);
                                            dcdu_status_line.set("STBY SENT".to_string());
                                            {
                                                let mut status = dcdu_status_line;
                                                spawn(async move {
                                                    tokio::time::sleep(Duration::from_secs(3)).await;
                                                    status.set(String::new());
                                                });
                                            }
                                        } else {
                                            stby_sent_mins.write().remove(&min);
                                            dcdu_status_line.set("SENT".to_string());
                                        }
                                        {
                                            let mut lsk = active_dcdu_lsk;
                                            spawn(async move {
                                                tokio::time::sleep(Duration::from_millis(600)).await;
                                                lsk.set(None);
                                            });
                                        }
                                        if !matches!(intent, CpdlcResponseIntent::Standby) {
                                            let mut s = app_state.write();
                                            if let Some(tab) = s.tab_mut_by_id(tab_id) {
                                                if let Some(m) = tab.messages.iter_mut().find(|m| m.min == Some(min) && !m.is_outgoing) {
                                                    m.responded = true;
                                                }
                                            }
                                        }
                                    }
                                },
                                "―"
                            }
                        }

                        div { class: "a320-native-dcdu-screen",
                        if *dcdu_blanking.read() {
                            div { class: "a320-native-blanking" }
                        } else if let Some(ref msg) = current {
                            {
                                let msg_ts = format!("{}Z", msg.timestamp.format("%H%M"));
                                let msg_to_from = if msg.is_outgoing { "TO" } else { "FROM" };
                                let msg_peer = if msg.is_outgoing {
                                    msg.to_callsign.clone().unwrap_or_else(|| "---".to_string())
                                } else {
                                    msg.from_callsign.clone().unwrap_or_else(|| "---".to_string())
                                };
                                let body_class = if msg.is_outgoing {
                                    "a320-native-msg-body preview"
                                } else {
                                    "a320-native-msg-body uplink"
                                };
                                let queue_hint = if visible.len() > 1 {
                                    format!("MSG {}/{}", effective_idx + 1, visible.len())
                                } else {
                                    String::new()
                                };
                                let body_text = msg.display_text.clone().unwrap_or_else(|| msg.raw_json.clone());
                                let wrapped = dcdu_wrap_24x4(&body_text);
                                rsx! {
                                    div { class: "a320-native-msg-meta",
                                        span { class: "meta-left", "{msg_ts} {msg_to_from} {msg_peer}" }
                                        span { class: "meta-right", "{queue_hint}" }
                                    }
                                    div { class: "a320-dcdu-body-rows" }
                                    div { class: "{body_class} a320-dcdu-body-line", "{wrapped[0]}" }
                                    div { class: "{body_class} a320-dcdu-body-line", "{wrapped[1]}" }
                                    div { class: "{body_class} a320-dcdu-body-line", "{wrapped[2]}" }
                                    div { class: "{body_class} a320-dcdu-body-line", "{wrapped[3]}" }

                                    if !linked.is_empty() {
                                        div { class: "a320-native-chain-sep", "..." }
                                    }
                                    if !draft_elements.read().is_empty() {
                                        div { class: "a320-native-chain-draft", "PREVIEW READY" }
                                    }
                                }
                            }
                        } else {
                            div { class: "a320-native-empty", "NO MESSAGE" }
                        }

                            {
                                let left_action = response_grid
                                    .top_left
                                    .or(response_grid.bot_left)
                                    .map(|i| format!("*{}", dcdu_label(i.label())))
                                    .unwrap_or_default();
                                let right_action = if !draft_elements.read().is_empty() {
                                    "SEND*".to_string()
                                } else if let Some(intent) = response_grid.bot_right.or(response_grid.top_right) {
                                    format!("{}*", dcdu_label(intent.label()))
                                } else {
                                    "CLOSE*".to_string()
                                };
                                let queue_text = if visible.len() > 1 {
                                    format!("{}/{}", effective_idx + 1, visible.len())
                                } else {
                                    String::new()
                                };
                                rsx! {
                                    div { class: "a320-native-dcdu-sep" }
                                    div { class: "a320-dcdu-line6-status", "{lower_status}" }
                                    div { class: "a320-dcdu-line7-actions",
                                        span { class: "a320-native-lbl left", "{left_action}" }
                                        span { class: "a320-native-center", "{queue_text}" }
                                        span { class: "a320-native-lbl right", "{right_action}" }
                                    }
                                }
                            }
                    }

                        div { class: "a320-native-dcdu-buttons right",
                            button { class: "a320-dcdu-btn", "PRINT" }
                            div { class: "a320-dcdu-spacer" }
                            button { class: "a320-dcdu-btn", "PGE-" }
                            button { class: "a320-dcdu-btn", "PGE+" }
                            div { class: "a320-dcdu-spacer" }
                            button {
                                class: if response_grid.top_right.map(|i| i.label().to_string()) == active_lsk_label {
                                    "a320-dcdu-btn action active"
                                } else {
                                    "a320-dcdu-btn action"
                                },
                                disabled: response_grid.top_right.is_none() || current_min.is_none(),
                                onclick: {
                                    let callsign = callsign.clone();
                                    let acars_address = acars_address.clone();
                                    let station_callsign = station_callsign.clone();
                                    let intent = response_grid.top_right;
                                    let min = current_min;
                                    move |_| {
                                        let Some(intent) = intent else { return; };
                                        let Some(station) = station_callsign.clone() else { return; };
                                        let Some(min) = min else { return; };
                                        let label = intent.label().to_string();
                                        let downlink_id = intent.downlink_id().to_string();
                                        let clients = nats_clients.read();
                                        if let Some(client) = clients.get(&tab_id) {
                                            let msg = client.cpdlc_aircraft_application(
                                                &callsign,
                                                &acars_address,
                                                station.as_str(),
                                                vec![MessageElement::new(downlink_id.clone(), vec![])],
                                                Some(min),
                                            );
                                            let client = client.clone();
                                            spawn(async move {
                                                let _ = client.send_to_server(msg).await;
                                            });
                                        }
                                        crate::push_outgoing_message(&mut app_state.clone(), tab_id, &label);
                                        active_dcdu_lsk.set(Some(label.clone()));
                                        if matches!(intent, CpdlcResponseIntent::Standby) {
                                            stby_sent_mins.write().insert(min);
                                            dcdu_status_line.set("STBY SENT".to_string());
                                            {
                                                let mut status = dcdu_status_line;
                                                spawn(async move {
                                                    tokio::time::sleep(Duration::from_secs(3)).await;
                                                    status.set(String::new());
                                                });
                                            }
                                        } else {
                                            stby_sent_mins.write().remove(&min);
                                            dcdu_status_line.set("SENT".to_string());
                                        }
                                        {
                                            let mut lsk = active_dcdu_lsk;
                                            spawn(async move {
                                                tokio::time::sleep(Duration::from_millis(600)).await;
                                                lsk.set(None);
                                            });
                                        }
                                        if !matches!(intent, CpdlcResponseIntent::Standby) {
                                            let mut s = app_state.write();
                                            if let Some(tab) = s.tab_mut_by_id(tab_id) {
                                                if let Some(m) = tab.messages.iter_mut().find(|m| m.min == Some(min) && !m.is_outgoing) {
                                                    m.responded = true;
                                                }
                                            }
                                        }
                                    }
                                },
                                "―"
                            }
                            if !draft_elements.read().is_empty() {
                                button {
                                    class: "a320-dcdu-btn action",
                                    disabled: *sending.read(),
                                    onclick: {
                                        let callsign = callsign.clone();
                                        let acars_address = acars_address.clone();
                                        let station_callsign = station_callsign.clone();
                                        move |_| {
                                            let Some(station) = station_callsign.clone() else { return; };
                                            let elements = draft_elements.read().clone();
                                            if elements.is_empty() {
                                                return;
                                            }
                                            let mrn = *draft_mrn.read();
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
                                                sending.set(true);
                                                dcdu_status_line.set("SENDING".to_string());
                                                spawn(async move {
                                                    let _ = client.send_to_server(msg).await;
                                                });
                                                let text = elements
                                                    .iter()
                                                    .map(render_element)
                                                    .collect::<Vec<_>>()
                                                    .join(" / ");
                                                crate::push_outgoing_message(&mut app_state.clone(), tab_id, &text);
                                                if closes_dialogue_response_elements(&elements) {
                                                    if let Some(mrn) = mrn {
                                                        let mut s = app_state.write();
                                                        if let Some(tab) = s.tab_mut_by_id(tab_id) {
                                                            if let Some(m) = tab.messages.iter_mut().find(|m| m.min == Some(mrn) && !m.is_outgoing) {
                                                                m.responded = true;
                                                            }
                                                        }
                                                    }
                                                }
                                                draft_elements.set(vec![]);
                                                draft_mrn.set(None);
                                                sending.set(false);
                                                dcdu_status_line.set("SENT".to_string());
                                            }
                                        }
                                    },
                                    "―"
                                }
                            } else {
                                button {
                                    class: if response_grid.bot_right.map(|i| i.label().to_string()) == active_lsk_label {
                                        "a320-dcdu-btn action active"
                                    } else {
                                        "a320-dcdu-btn action"
                                    },
                                    disabled: response_grid.bot_right.is_none() || current_min.is_none(),
                                    onclick: {
                                        let callsign = callsign.clone();
                                        let acars_address = acars_address.clone();
                                        let station_callsign = station_callsign.clone();
                                        let intent = response_grid.bot_right;
                                        let min = current_min;
                                        move |_| {
                                            let Some(intent) = intent else { return; };
                                            let Some(station) = station_callsign.clone() else { return; };
                                            let Some(min) = min else { return; };
                                            let label = intent.label().to_string();
                                            let downlink_id = intent.downlink_id().to_string();
                                            let clients = nats_clients.read();
                                            if let Some(client) = clients.get(&tab_id) {
                                                let msg = client.cpdlc_aircraft_application(
                                                    &callsign,
                                                    &acars_address,
                                                    station.as_str(),
                                                    vec![MessageElement::new(downlink_id.clone(), vec![])],
                                                    Some(min),
                                                );
                                                let client = client.clone();
                                                spawn(async move {
                                                    let _ = client.send_to_server(msg).await;
                                                });
                                            }
                                            crate::push_outgoing_message(&mut app_state.clone(), tab_id, &label);
                                            active_dcdu_lsk.set(Some(label.clone()));
                                            if matches!(intent, CpdlcResponseIntent::Standby) {
                                                stby_sent_mins.write().insert(min);
                                                dcdu_status_line.set("STBY SENT".to_string());
                                                {
                                                    let mut status = dcdu_status_line;
                                                    spawn(async move {
                                                        tokio::time::sleep(Duration::from_secs(3)).await;
                                                        status.set(String::new());
                                                    });
                                                }
                                            } else {
                                                stby_sent_mins.write().remove(&min);
                                                dcdu_status_line.set("SENT".to_string());
                                            }
                                            {
                                                let mut lsk = active_dcdu_lsk;
                                                spawn(async move {
                                                    tokio::time::sleep(Duration::from_millis(600)).await;
                                                    lsk.set(None);
                                                });
                                            }
                                            if !matches!(intent, CpdlcResponseIntent::Standby) {
                                                let mut s = app_state.write();
                                                if let Some(tab) = s.tab_mut_by_id(tab_id) {
                                                    if let Some(m) = tab.messages.iter_mut().find(|m| m.min == Some(min) && !m.is_outgoing) {
                                                        m.responded = true;
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    "―"
                                }
                            }
                        }
                    }
                }

                // MCDU (bottom)
                div { class: "a320-native-mcdu",
                    div { class: "a320-native-annunciators",
                        span { class: "a320-ann on", "FM1" }
                        span { class: "a320-ann on", "IND" }
                        span { class: "a320-ann on", "RDY" }
                        span { class: "a320-ann on", "FM2" }
                    }
                    div { class: "a320-native-mcdu-shell",
                        div { class: "a320-native-lsk-col",
                            for _i in 0..6 {
                                button { class: "a320-native-lsk", "" }
                            }
                        }
                        div { class: "a320-native-mcdu-screen",
                            match current_page {
                                McduPage::AtcMenu => rsx! {
                                    div { class: "a320-mcdu-grid14",
                                        div { class: "a320-mcdu-title", "ATC MENU" }

                                        div { class: "a320-mcdu-label-row" }
                                        div { class: "a320-mcdu-data-row",
                                            button { class: "a320-mcdu-lsk-text left", onclick: move |_| page.set(McduPage::LatReq), "LAT REQ" }
                                            button { class: "a320-mcdu-lsk-text right", onclick: move |_| page.set(McduPage::VertReq), "VERT REQ" }
                                        }

                                        div { class: "a320-mcdu-label-row" }
                                        div { class: "a320-mcdu-data-row",
                                            button { class: "a320-mcdu-lsk-text left", onclick: move |_| page.set(McduPage::OtherReq), "OTHER REQ" }
                                            button { class: "a320-mcdu-lsk-text right", onclick: move |_| page.set(McduPage::Text), "TEXT" }
                                        }

                                        div { class: "a320-mcdu-label-row" }
                                        div { class: "a320-mcdu-data-row",
                                            button { class: "a320-mcdu-lsk-text left", onclick: move |_| page.set(McduPage::Notification), "NOTIF" }
                                            button {
                                                class: "a320-mcdu-lsk-text right action",
                                                disabled: !has_pending,
                                                onclick: move |_| {
                                                    draft_mrn.set(current_open_min);
                                                    draft_elements.set(pending_elements.read().clone());
                                                    pending_elements.set(vec![]);
                                                    field_values.set(HashMap::new());
                                                },
                                                "XFR TO DCDU"
                                            }
                                        }

                                        div { class: "a320-mcdu-label-row" }
                                        div { class: "a320-mcdu-data-row" }

                                        div { class: "a320-mcdu-label-row" }
                                        div { class: "a320-mcdu-data-row" }

                                        div { class: "a320-mcdu-label-row" }
                                        div { class: "a320-mcdu-data-row" }
                                    }
                                },
                                McduPage::Notification => rsx! {
                                    div { class: "a320-mcdu-grid14",
                                        div { class: "a320-mcdu-title", "NOTIFICATION" }

                                        div { class: "a320-mcdu-label-row",
                                            span { class: "a320-mcdu-label left", "ATC FLT NBR" }
                                        }
                                        div { class: "a320-mcdu-data-row",
                                            span { class: "a320-mcdu-value left green", "{callsign}" }
                                        }

                                        div { class: "a320-mcdu-label-row",
                                            span { class: "a320-mcdu-label left", "ATC CENTER" }
                                        }
                                        div { class: "a320-mcdu-data-row",
                                            {
                                                let center_display = if atc_center.read().is_empty() {
                                                    "----".to_string()
                                                } else {
                                                    atc_center.read().clone()
                                                };
                                                rsx! {
                                                    span { class: "a320-mcdu-value left cyan", "{center_display}" }
                                                }
                                            }
                                            button {
                                                class: "a320-mcdu-lsk-text right action",
                                                disabled: atc_center.read().len() < 3,
                                                onclick: {
                                                    let callsign = callsign.clone();
                                                    let acars_address = acars_address.clone();
                                                    move |_| {
                                                        let center = atc_center.read().clone();
                                                        let clients = nats_clients.read();
                                                        if let Some(client) = clients.get(&tab_id) {
                                                            let msg = client.cpdlc_logon_request(&callsign, &acars_address, &center);
                                                            let client = client.clone();
                                                            spawn(async move {
                                                                let _ = client.send_to_server(msg).await;
                                                            });
                                                        }
                                                        crate::push_outgoing_message(&mut app_state.clone(), tab_id, &format!("LOGON REQUEST → {center}"));
                                                    }
                                                },
                                                "NOTIFY"
                                            }
                                        }

                                        div { class: "a320-mcdu-label-row" }
                                        div { class: "a320-mcdu-data-row" }

                                        div { class: "a320-mcdu-label-row" }
                                        div { class: "a320-mcdu-data-row",
                                            button { class: "a320-mcdu-lsk-text left", onclick: move |_| page.set(McduPage::AtcMenu), "RETURN" }
                                        }

                                        div { class: "a320-mcdu-label-row" }
                                        div { class: "a320-mcdu-data-row" }

                                        div { class: "a320-mcdu-label-row" }
                                        div { class: "a320-mcdu-data-row" }
                                    }
                                },
                                McduPage::LatReq => rsx! {
                                    McduCommandPage {
                                        title: "ATC LAT REQ".to_string(),
                                        entries: vec![
                                            ("DM22 DIR TO [POS]".to_string(), "DM22".to_string(), Some(ArgType::Position)),
                                            ("DM27 WX DEV [DIST]".to_string(), "DM27".to_string(), Some(ArgType::Distance)),
                                            ("DM70 HEADING [DEG]".to_string(), "DM70".to_string(), Some(ArgType::Degrees)),
                                            ("DM65 DUE WX".to_string(), "DM65".to_string(), None),
                                            ("DM66 DUE A/C PERF".to_string(), "DM66".to_string(), None),
                                        ],
                                        page,
                                        has_pending,
                                        on_transfer: move |_| {
                                            draft_mrn.set(current_open_min);
                                            draft_elements.set(pending_elements.read().clone());
                                            pending_elements.set(vec![]);
                                            field_values.set(HashMap::new());
                                        },
                                        scratchpad,
                                        field_values,
                                        pending_elements,
                                    }
                                },
                                McduPage::VertReq => rsx! {
                                    McduCommandPage {
                                        title: "ATC VERT REQ".to_string(),
                                        entries: vec![
                                            ("DM9 CLIMB TO [LEVEL]".to_string(), "DM9".to_string(), Some(ArgType::Level)),
                                            ("DM10 DESCEND TO [LEVEL]".to_string(), "DM10".to_string(), Some(ArgType::Level)),
                                            ("DM6 REQUEST [LEVEL]".to_string(), "DM6".to_string(), Some(ArgType::Level)),
                                            ("DM18 REQUEST [SPEED]".to_string(), "DM18".to_string(), Some(ArgType::Speed)),
                                            ("DM65 DUE WX".to_string(), "DM65".to_string(), None),
                                            ("DM66 DUE A/C PERF".to_string(), "DM66".to_string(), None),
                                        ],
                                        page,
                                        has_pending,
                                        on_transfer: move |_| {
                                            draft_mrn.set(current_open_min);
                                            draft_elements.set(pending_elements.read().clone());
                                            pending_elements.set(vec![]);
                                            field_values.set(HashMap::new());
                                        },
                                        scratchpad,
                                        field_values,
                                        pending_elements,
                                    }
                                },
                                McduPage::OtherReq => rsx! {
                                    McduCommandPage {
                                        title: "ATC OTHER REQ".to_string(),
                                        entries: vec![
                                            ("DM18 REQUEST [SPEED]".to_string(), "DM18".to_string(), Some(ArgType::Speed)),
                                            ("DM70 REQUEST HEADING [DEG]".to_string(), "DM70".to_string(), Some(ArgType::Degrees)),
                                            ("DM20 REQUEST VOICE".to_string(), "DM20".to_string(), None),
                                            ("DM25 REQUEST CLEARANCE".to_string(), "DM25".to_string(), None),
                                        ],
                                        page,
                                        has_pending,
                                        on_transfer: move |_| {
                                            draft_mrn.set(current_open_min);
                                            draft_elements.set(pending_elements.read().clone());
                                            pending_elements.set(vec![]);
                                            field_values.set(HashMap::new());
                                        },
                                        scratchpad,
                                        field_values,
                                        pending_elements,
                                    }
                                },
                                McduPage::Text => rsx! {
                                    McduCommandPage {
                                        title: "ATC TEXT".to_string(),
                                        entries: vec![
                                            ("DM67 FREE TEXT".to_string(), "DM67".to_string(), Some(ArgType::FreeText)),
                                        ],
                                        page,
                                        has_pending,
                                        on_transfer: move |_| {
                                            draft_mrn.set(current_open_min);
                                            draft_elements.set(pending_elements.read().clone());
                                            pending_elements.set(vec![]);
                                            field_values.set(HashMap::new());
                                        },
                                        scratchpad,
                                        field_values,
                                        pending_elements,
                                    }
                                },
                            }

                            div { class: "a320-native-scratch",
                                div { class: "a320-native-field", "SCRATCHPAD" }
                                input {
                                    class: "a320-native-input",
                                    r#type: "text",
                                    value: "{scratchpad.read().clone()}",
                                    oninput: move |evt: Event<FormData>| scratchpad.set(evt.value().to_uppercase()),
                                }
                            }
                            {
                                let pending_text = pending_elements
                                    .read()
                                    .iter()
                                    .map(render_element)
                                    .collect::<Vec<_>>()
                                    .join(" / ");
                                rsx! {
                                    div { class: "a320-native-pending", "PENDING: {pending_text}" }
                                }
                            }
                        }
                        div { class: "a320-native-lsk-col right",
                            for _i in 0..6 {
                                button { class: "a320-native-lsk", "" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn McduCommandPage(
    title: String,
    entries: Vec<(String, String, Option<ArgType>)>,
    page: Signal<McduPage>,
    has_pending: bool,
    on_transfer: EventHandler<()>,
    scratchpad: Signal<String>,
    field_values: Signal<HashMap<String, String>>,
    pending_elements: Signal<Vec<MessageElement>>,
) -> Element {
    let mut paired_entries: Vec<(Option<(String, String, Option<ArgType>)>, Option<(String, String, Option<ArgType>)>)> = Vec::new();
    let mut idx = 0;
    while idx < entries.len() {
        let left = Some(entries[idx].clone());
        let right = if idx + 1 < entries.len() {
            Some(entries[idx + 1].clone())
        } else {
            None
        };
        paired_entries.push((left, right));
        idx += 2;
    }
    // Keep a stable MCDU line structure: 4 command pairs + 2 footer pairs = 6 pairs.
    while paired_entries.len() < 4 {
        paired_entries.push((None, None));
    }

    rsx! {
        div { class: "a320-mcdu-grid14",
            div { class: "a320-mcdu-title", "{title}" }

            for (left_entry, right_entry) in paired_entries.into_iter() {
                div { class: "a320-mcdu-label-row" }
                div { class: "a320-mcdu-data-row",
                    if let Some((label, id, arg)) = left_entry {
                        {
                            let value = field_values.read().get(&id).cloned();
                            rsx! {
                                button {
                                    class: "a320-mcdu-lsk-text left",
                                    onclick: move |_| {
                                        let args = if let Some(arg_type) = arg {
                                            let val = scratchpad.read().clone();
                                            let Some(parsed) = parse_arg(arg_type, &val) else { return; };
                                            vec![parsed]
                                        } else {
                                            vec![]
                                        };

                                        let allow_duplicates = id == "DM67" || args.is_empty();

                                        {
                                            let mut pending = pending_elements.write();
                                            if !allow_duplicates {
                                                pending.retain(|e| e.id != id);
                                            }
                                            pending.push(MessageElement::new(id.clone(), args));
                                        }

                                        if arg.is_some() {
                                            let mut fields = field_values.write();
                                            fields.insert(id.clone(), scratchpad.read().clone());
                                            scratchpad.set(String::new());
                                        }
                                    },
                                    "{label}"
                                    if let Some(v) = value {
                                        span { class: "a320-native-cmd-val", " {v}" }
                                    }
                                }
                            }
                        }
                    }

                    if let Some((label, id, arg)) = right_entry {
                        {
                            let value = field_values.read().get(&id).cloned();
                            rsx! {
                                button {
                                    class: "a320-mcdu-lsk-text right",
                                    onclick: move |_| {
                                        let args = if let Some(arg_type) = arg {
                                            let val = scratchpad.read().clone();
                                            let Some(parsed) = parse_arg(arg_type, &val) else { return; };
                                            vec![parsed]
                                        } else {
                                            vec![]
                                        };

                                        let allow_duplicates = id == "DM67" || args.is_empty();

                                        {
                                            let mut pending = pending_elements.write();
                                            if !allow_duplicates {
                                                pending.retain(|e| e.id != id);
                                            }
                                            pending.push(MessageElement::new(id.clone(), args));
                                        }

                                        if arg.is_some() {
                                            let mut fields = field_values.write();
                                            fields.insert(id.clone(), scratchpad.read().clone());
                                            scratchpad.set(String::new());
                                        }
                                    },
                                    "{label}"
                                    if let Some(v) = value {
                                        span { class: "a320-native-cmd-val", " {v}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            div { class: "a320-mcdu-label-row",
                span { class: "a320-mcdu-label left", "INPUTS" }
            }
            div { class: "a320-mcdu-data-row",
                button {
                    class: "a320-mcdu-lsk-text left",
                    onclick: move |_| {
                        pending_elements.set(vec![]);
                        field_values.set(HashMap::new());
                        scratchpad.set(String::new());
                    },
                    "ERASE"
                }
            }

            div { class: "a320-mcdu-label-row" }
            div { class: "a320-mcdu-data-row",
                button { class: "a320-mcdu-lsk-text left", onclick: move |_| page.set(McduPage::AtcMenu), "RETURN" }
                button {
                    class: "a320-mcdu-lsk-text right action",
                    disabled: !has_pending,
                    onclick: move |_| on_transfer.call(()),
                    "XFR TO DCDU"
                }
            }
        }
    }
}
