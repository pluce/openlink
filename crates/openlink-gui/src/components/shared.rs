use dioxus::prelude::*;
use openlink_models::{constrained_closing_reply_ids, AcarsMessage, CpdlcMessageType, CpdlcResponseIntent, OpenLinkMessage};
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

/// Callback payload for responding to a message: (min_of_original, semantic_intent)
pub type RespondPayload = (u8, CpdlcResponseIntent);

/// Reusable message list component showing human-readable CPDLC messages.
/// When `on_respond` is provided, messages requiring a response display
/// inline action buttons (WILCO, UNABLE, etc.) directly on the message card.
#[component]
pub fn MessageList(
    messages: Vec<ReceivedMessage>,
    #[props(default)] on_respond: Option<EventHandler<RespondPayload>>,
    #[props(default)] on_respond_compose: Option<EventHandler<RespondPayload>>,
    #[props(default)] on_suggested_reply: Option<EventHandler<u8>>,
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
                    let envelope = msg.envelope.clone();
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
                        if let Some(attr) = response_attr {
                            parts.push(attr.to_string());
                        }
                        if parts.is_empty() { None } else { Some(format!("[{}]", parts.join(" "))) }
                    };

                    // Determine which response buttons to show for this message
                    let response_buttons: Vec<CpdlcResponseIntent> = if !is_outgoing && on_respond.is_some() && !msg.responded {
                        response_attr
                            .map(CpdlcResponseIntent::for_attribute)
                            .unwrap_or_default()
                    } else {
                        vec![]
                    };

                    let has_suggested_replies = if !is_outgoing && on_suggested_reply.is_some() && !msg.responded {
                        envelope
                            .as_ref()
                            .and_then(|env| match &env.payload {
                                OpenLinkMessage::Acars(acars) => match &acars.message {
                                    AcarsMessage::CPDLC(cpdlc) => match &cpdlc.message {
                                        CpdlcMessageType::Application(app) => app.elements.first().map(|e| e.id.as_str()),
                                        _ => None,
                                    },
                                },
                                _ => None,
                            })
                            .map(|id| !constrained_closing_reply_ids(id).is_empty())
                            .unwrap_or(false)
                    } else {
                        false
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
                                        for intent in response_buttons.iter() {
                                            {
                                                let intent = *intent;
                                                let on_respond = on_respond.clone();
                                                let on_respond_compose = on_respond_compose.clone();
                                                rsx! {
                                                    div { class: "message-response-action",
                                                        button {
                                                            class: "message-response-btn",
                                                            onclick: move |_| {
                                                                if let Some(ref handler) = on_respond {
                                                                    handler.call((msg_min, intent));
                                                                }
                                                            },
                                                            "{intent.label()}"
                                                        }
                                                        if on_respond_compose.is_some() {
                                                            button {
                                                                class: "message-response-plus-btn",
                                                                title: "Respond + add",
                                                                onclick: move |_| {
                                                                    if let Some(ref handler) = on_respond_compose {
                                                                        handler.call((msg_min, intent));
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
                            if response_buttons.is_empty() && has_suggested_replies {
                                if let Some(msg_min) = min {
                                    if let Some(suggest_handler) = on_suggested_reply.clone() {
                                        div { class: "message-response-buttons",
                                            button {
                                                class: "message-response-btn",
                                                onclick: move |_| {
                                                    suggest_handler.call(msg_min);
                                                },
                                                "SUGGESTED REPLIES"
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
