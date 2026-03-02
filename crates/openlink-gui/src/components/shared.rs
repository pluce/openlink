use dioxus::prelude::*;
use openlink_models::{
    AcarsMessage, CpdlcMessageType, CpdlcResponseIntent, OpenLinkMessage, ResponseAttribute,
};
use openlink_sdk::{choose_short_response_intents, response_attr_to_intents};
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

/// Reusable message list component showing human-readable CPDLC messages
#[component]
pub fn MessageList(
    messages: Vec<ReceivedMessage>,
    on_respond: Option<EventHandler<(u8, CpdlcResponseIntent)>>,
    on_respond_compose: Option<EventHandler<(u8, CpdlcResponseIntent)>>,
    on_suggested_reply: Option<EventHandler<u8>>,
) -> Element {
    let locale = use_locale();
    let tr = t(*locale.read());

    let response_intents_for_message = |msg: &ReceivedMessage| -> Vec<CpdlcResponseIntent> {
        if matches!(msg.response_attr, Some(ResponseAttribute::Y)) {
            return vec![CpdlcResponseIntent::Unable, CpdlcResponseIntent::Standby];
        }

        if let Some(attr) = msg.response_attr {
            return response_attr_to_intents(attr);
        }

        msg.envelope
            .as_ref()
            .and_then(|env| {
                if let OpenLinkMessage::Acars(acars) = &env.payload {
                    let AcarsMessage::CPDLC(cpdlc) = &acars.message;
                    if let CpdlcMessageType::Application(app) = &cpdlc.message {
                        return Some(choose_short_response_intents(&app.elements));
                    }
                }
                None
            })
            .unwrap_or_default()
    };

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
                    let on_respond_for_msg = on_respond.clone();
                    let on_respond_compose_for_msg = on_respond_compose.clone();
                    let on_suggested_reply_for_msg = on_suggested_reply.clone();
                    let intents = response_intents_for_message(msg);
                    let msg_min = msg.min;
                    let can_respond = !msg.is_outgoing
                        && !msg.responded
                        && msg_min.is_some()
                        && !intents.is_empty()
                        && on_respond_for_msg.is_some();
                    let can_compose = can_respond && on_respond_compose_for_msg.is_some();
                    rsx! {
                        div { class: "message-item",
                            div { class: "message-header",
                                span { class: "message-time", "{time_str}" }
                                if let Some(ref from) = from {
                                    span { class: "message-from", "← {from}" }
                                }
                            }
                            if let Some(ref text) = display {
                                div { class: "message-payload", "{text}" }
                            } else {
                                pre { class: "message-body", "{raw}" }
                            }

                            if can_respond {
                                div { class: "message-responses",
                                    for intent in intents.iter() {
                                        {
                                            let intent_val = *intent;
                                            let label = intent_val.label().to_string();
                                            rsx! {
                                                button {
                                                    class: "response-btn",
                                                    onclick: move |_| {
                                                        if let (Some(min), Some(handler)) =
                                                            (msg_min, on_respond_for_msg.clone())
                                                        {
                                                            handler.call((min, intent_val));
                                                        }
                                                    },
                                                    "{label}"
                                                }
                                            }
                                        }
                                    }
                                    if can_compose {
                                        button {
                                            class: "response-btn response-btn-secondary",
                                            onclick: move |_| {
                                                if let (Some(min), Some(intent), Some(handler)) = (
                                                    msg_min,
                                                    intents.first().copied(),
                                                    on_respond_compose_for_msg.clone(),
                                                ) {
                                                    handler.call((min, intent));
                                                }
                                            },
                                            "COMPOSE"
                                        }
                                    }
                                    if let (Some(min), Some(handler)) =
                                        (msg_min, on_suggested_reply_for_msg.clone())
                                    {
                                        button {
                                            class: "response-btn response-btn-secondary",
                                            onclick: move |_| handler.call(min),
                                            "SUGGEST"
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
