use dioxus::prelude::*;
use crate::state::ReceivedMessage;
use crate::i18n::{use_locale, t};

/// Reusable status indicator badge
#[component]
pub fn StatusBadge(status: String) -> Element {
    let (class, icon) = match status.as_str() {
        "connected" => ("badge badge-connected", "●"),
        "pending" => ("badge badge-pending", "⏳"),
        "logon" => ("badge badge-logon", "●"),
        "offline" => ("badge badge-offline", "○"),
        _ => ("badge", "?"),
    };

    rsx! {
        span { class: "{class}", "{icon}" }
    }
}

/// Callback payload for responding to a message: (min_of_original, response_msg_id)
/// Example: (0, "DM0") means respond with WILCO to message with MIN=0
pub type RespondPayload = (u8, String);

/// Reusable message list component showing human-readable CPDLC messages.
/// When `on_respond` is provided, messages requiring a response display
/// inline action buttons (WILCO, UNABLE, etc.) directly on the message card.
#[component]
pub fn MessageList(
    messages: Vec<ReceivedMessage>,
    #[props(default)] on_respond: Option<EventHandler<RespondPayload>>,
) -> Element {
    let locale = use_locale();
    let tr = t(*locale.read());
    rsx! {
        div { class: "message-list",
            if messages.is_empty() {
                p { class: "placeholder", "{tr.no_messages}" }
            }
            for msg in messages.iter().rev() {
                {
                    let time_str = msg.timestamp.format("%H:%M:%S").to_string();
                    let from = msg.from_callsign.clone();
                    let display = msg.display_text.clone();
                    let raw = msg.raw_json.clone();
                    let is_outgoing = msg.is_outgoing;
                    let min = msg.min;
                    let mrn = msg.mrn;
                    let response_attr = msg.response_attr.clone();
                    let item_class = if is_outgoing { "message-item message-sent" } else { "message-item" };

                    // Build MIN/MRN annotation string
                    let annotation = {
                        let mut parts = Vec::new();
                        if let Some(m) = min {
                            parts.push(format!("MIN={m}"));
                        }
                        if let Some(m) = mrn {
                            parts.push(format!("MRN={m}"));
                        }
                        if let Some(ref attr) = response_attr {
                            parts.push(attr.clone());
                        }
                        if parts.is_empty() { None } else { Some(format!("[{}]", parts.join(" "))) }
                    };

                    // Determine which response buttons to show for this message
                    let response_buttons: Vec<(&str, &str)> = if !is_outgoing && on_respond.is_some() && !msg.responded {
                        match response_attr.as_deref() {
                            Some("WU") | Some("Y") => vec![("WILCO", "DM0"), ("UNABLE", "DM1"), ("STANDBY", "DM2")],
                            Some("AN") => vec![("AFFIRM", "DM4"), ("NEGATIVE", "DM5"), ("STANDBY", "DM2")],
                            Some("R") => vec![("ROGER", "DM3"), ("STANDBY", "DM2")],
                            _ => vec![],
                        }
                    } else {
                        vec![]
                    };

                    rsx! {
                        div { class: "{item_class}",
                            div { class: "message-header",
                                span { class: "message-time", "{time_str}" }
                                if is_outgoing {
                                    span { class: "message-dir-out", "↑ SENT" }
                                } else if let Some(ref from) = from {
                                    span { class: "message-from", "← {from}" }
                                }
                                if let Some(ref ann) = annotation {
                                    span { class: "message-annotation", " {ann}" }
                                }
                            }
                            if let Some(ref text) = display {
                                div { class: "message-payload", "{text}" }
                            } else if !raw.is_empty() {
                                pre { class: "message-body", "{raw}" }
                            }
                            // Inline response buttons
                            if !response_buttons.is_empty() {
                                if let Some(msg_min) = min {
                                    div { class: "message-response-buttons",
                                        for (label, msg_id) in response_buttons.iter() {
                                            {
                                                let msg_id = msg_id.to_string();
                                                let label = *label;
                                                let on_respond = on_respond.clone();
                                                rsx! {
                                                    button {
                                                        class: "message-response-btn",
                                                        onclick: move |_| {
                                                            if let Some(ref handler) = on_respond {
                                                                handler.call((msg_min, msg_id.clone()));
                                                            }
                                                        },
                                                        "{label}"
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
}
