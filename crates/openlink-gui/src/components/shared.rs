use dioxus::prelude::*;
use openlink_models::CpdlcResponseIntent;
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
    // Kept for API compatibility with DCDU/ATC views.
    let _ = (&on_respond, &on_respond_compose, &on_suggested_reply);
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
                        }
                    }
                }
            }
        }
    }
}
