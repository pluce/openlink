//! Shared CPDLC runtime rules for SDK clients.
//!
//! This module centralizes protocol decisions that should behave the same
//! across Rust and TypeScript SDKs.

use openlink_models::{
    closes_dialogue_response_elements as model_closes_dialogue_response_elements,
    find_definition, CpdlcResponseIntent, MessageElement, ResponseAttribute,
};

/// Logical acknowledgement downlink message ID (aircraft sender).
pub const LOGICAL_ACK_DOWNLINK_ID: &str = "DM100";
/// Logical acknowledgement uplink message ID (station sender).
pub const LOGICAL_ACK_UPLINK_ID: &str = "UM227";

/// Returns true if the message element id is a logical acknowledgement.
pub fn is_logical_ack_element_id(id: &str) -> bool {
    matches!(id, LOGICAL_ACK_DOWNLINK_ID | LOGICAL_ACK_UPLINK_ID)
}

/// Returns true if any element in the message is a logical acknowledgement.
pub fn message_contains_logical_ack(elements: &[MessageElement]) -> bool {
    elements.iter().any(|e| is_logical_ack_element_id(&e.id))
}

/// Returns true if clients should auto-send a logical acknowledgement.
///
/// Rules:
/// - incoming message must have a valid `MIN` (`min > 0`)
/// - message must not already be a logical acknowledgement (loop prevention)
pub fn should_auto_send_logical_ack(elements: &[MessageElement], min: u8) -> bool {
    min > 0 && !message_contains_logical_ack(elements)
}

/// Returns true if response elements close the referenced dialogue.
///
/// Delegates to the canonical model-level implementation.
pub fn closes_dialogue_response_elements(elements: &[MessageElement]) -> bool {
    model_closes_dialogue_response_elements(elements)
}

/// Map one response attribute to available short response intents.
pub fn response_attr_to_intents(attr: ResponseAttribute) -> Vec<CpdlcResponseIntent> {
    CpdlcResponseIntent::for_attribute(attr)
}

/// Choose short response intents using a custom response-attribute resolver.
///
/// This is useful for fixture-driven conformance tests and for SDKs that
/// may source catalog entries from a non-static registry.
pub fn choose_short_response_intents_with_resolver<F>(
    elements: &[MessageElement],
    mut resolve_attr: F,
) -> Vec<CpdlcResponseIntent>
where
    F: FnMut(&str) -> Option<ResponseAttribute>,
{
    let attrs: Vec<ResponseAttribute> = elements
        .iter()
        .filter_map(|e| resolve_attr(&e.id))
        .collect();

    let effective = if attrs.is_empty() {
        ResponseAttribute::WU
    } else {
        ResponseAttribute::effective(&attrs)
    };

    response_attr_to_intents(effective)
}

/// Choose short response intents for a (possibly multi-element) message.
///
/// Uses effective response attribute precedence from catalog definitions.
/// Fallback when definitions are unavailable is `W/U` intents.
pub fn choose_short_response_intents(elements: &[MessageElement]) -> Vec<CpdlcResponseIntent> {
    choose_short_response_intents_with_resolver(elements, |id| {
        find_definition(id).map(|d| d.response_attr)
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::Value;

    use openlink_models::MessageElement;

    use super::*;

    fn vectors_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../spec/sdk-conformance/runtime-vectors.v1.json")
    }

    fn load_vectors() -> Value {
        let path = vectors_path();
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read vectors at {}: {e}", path.display()));
        serde_json::from_str(&raw).expect("runtime vectors JSON must be valid")
    }

    fn parse_elements(input: &Value) -> Vec<MessageElement> {
        serde_json::from_value(
            input
                .get("elements")
                .cloned()
                .unwrap_or_else(|| Value::Array(vec![])),
        )
        .expect("vector input.elements must deserialize as MessageElement[]")
    }

    fn parse_attr(attr: &str) -> Option<ResponseAttribute> {
        Some(match attr {
            "WU" => ResponseAttribute::WU,
            "AN" => ResponseAttribute::AN,
            "R" => ResponseAttribute::R,
            "Y" => ResponseAttribute::Y,
            "N" => ResponseAttribute::N,
            "NE" => ResponseAttribute::NE,
            _ => return None,
        })
    }

    fn downlink_ids(intents: &[CpdlcResponseIntent]) -> Vec<&'static str> {
        intents.iter().map(|i| i.downlink_id()).collect()
    }

    #[test]
    fn logical_ack_helpers() {
        assert!(is_logical_ack_element_id("DM100"));
        assert!(is_logical_ack_element_id("UM227"));
        assert!(!is_logical_ack_element_id("DM0"));
    }

    #[test]
    fn auto_ack_rule() {
        let normal = vec![MessageElement::new("UM20", vec![])];
        assert!(should_auto_send_logical_ack(&normal, 12));
        assert!(!should_auto_send_logical_ack(&normal, 0));

        let ack = vec![MessageElement::new("DM100", vec![])];
        assert!(!should_auto_send_logical_ack(&ack, 12));
    }

    #[test]
    fn choose_intents_from_attr() {
        let elems = vec![MessageElement::new("UM20", vec![])]; // WU
        let intents = choose_short_response_intents(&elems);
        assert!(intents.iter().any(|i| matches!(i, CpdlcResponseIntent::Wilco)));
        assert!(intents.iter().any(|i| matches!(i, CpdlcResponseIntent::Unable)));
    }

    #[test]
    fn runtime_vectors_logical_ack() {
        let vectors = load_vectors();
        let cases = vectors["runtime"]["logical_ack"]
            .as_array()
            .expect("logical_ack vectors must be an array");

        for case in cases {
            let id = case["id"].as_str().unwrap_or("<unknown>");
            let op = case["operation"].as_str().unwrap_or("<unknown>");
            let input = &case["input"];
            let expected = case["expected"].as_bool().expect("expected must be bool");

            let got = match op {
                "is_logical_ack_element_id" => {
                    let msg_id = input["id"].as_str().expect("input.id must be a string");
                    is_logical_ack_element_id(msg_id)
                }
                "message_contains_logical_ack" => {
                    let elements = parse_elements(input);
                    message_contains_logical_ack(&elements)
                }
                "should_auto_send_logical_ack" => {
                    let elements = parse_elements(input);
                    let min = input["min"].as_u64().expect("input.min must be a number") as u8;
                    should_auto_send_logical_ack(&elements, min)
                }
                _ => panic!("unsupported logical_ack operation in vector {id}: {op}"),
            };

            assert_eq!(got, expected, "vector failed: {id}");
        }
    }

    #[test]
    fn runtime_vectors_response_attr() {
        let vectors = load_vectors();
        let cases = vectors["runtime"]["response_attr"]
            .as_array()
            .expect("response_attr vectors must be an array");

        for case in cases {
            let id = case["id"].as_str().unwrap_or("<unknown>");
            let attr = case["input"]["attr"]
                .as_str()
                .unwrap_or_else(|| panic!("vector {id}: input.attr must be a string"));
            let expected: Vec<String> = serde_json::from_value(
                case["expected_downlink_ids"]
                    .clone(),
            )
            .unwrap_or_else(|_| panic!("vector {id}: expected_downlink_ids must be string[]"));

            let attr = parse_attr(attr)
                .unwrap_or_else(|| panic!("vector {id}: unsupported response attr {attr}"));
            let got = downlink_ids(&response_attr_to_intents(attr))
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>();

            assert_eq!(got, expected, "vector failed: {id}");
        }
    }

    #[test]
    fn runtime_vectors_short_response_selection() {
        let vectors = load_vectors();
        let cases = vectors["runtime"]["short_response_selection"]
            .as_array()
            .expect("short_response_selection vectors must be an array");

        for case in cases {
            let id = case["id"].as_str().unwrap_or("<unknown>");
            let input = &case["input"];
            let elements = parse_elements(input);
            let entries = input["catalog_entries"].as_object();
            let expected: Vec<String> = serde_json::from_value(
                case["expected_downlink_ids"].clone(),
            )
            .unwrap_or_else(|_| panic!("vector {id}: expected_downlink_ids must be string[]"));

            let got = downlink_ids(&choose_short_response_intents_with_resolver(
                &elements,
                |msg_id| {
                    entries
                        .and_then(|m| m.get(msg_id))
                        .and_then(|entry| entry.get("response_attr"))
                        .and_then(|v| v.as_str())
                        .and_then(parse_attr)
                },
            ))
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();

            assert_eq!(got, expected, "vector failed: {id}");
        }
    }

    #[test]
    fn runtime_vectors_dialogue_close() {
        let vectors = load_vectors();
        let cases = vectors["runtime"]["dialogue_close"]
            .as_array()
            .expect("dialogue_close vectors must be an array");

        for case in cases {
            let id = case["id"].as_str().unwrap_or("<unknown>");
            let input = &case["input"];
            let elements = parse_elements(input);
            let expected = case["expected"].as_bool().expect("expected must be bool");

            let got = closes_dialogue_response_elements(&elements);
            assert_eq!(got, expected, "vector failed: {id}");
        }
    }
}
