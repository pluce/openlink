//! Bidirectional CPDLC translator between Hoppie and OpenLink formats.
//!
//! ## Hoppie CPDLC packet format
//!
//! ```text
//! /data2/{min}/{mrn}/{response_attr}/{message_text}
//! ```
//!
//! - `min`: message identification number (running counter)
//! - `mrn`: message reference number (empty if initiating)
//! - `response_attr`: Y, N, WU, AN, R, NE
//! - `message_text`: rendered FANS-1/A text (e.g. "CLIMB TO FL350")
//!
//! ## OpenLink format
//!
//! Uses [`CpdlcEnvelope`] with structured [`MessageElement`]s that carry
//! typed arguments.

use anyhow::{Context, Result};
use chrono::Utc;
use tracing::warn;

use openlink_models::{
    AcarsEndpointCallsign, ArgType, CpdlcApplicationMessage, CpdlcArgument,
    CpdlcEnvelope, CpdlcMessageType, FlightLevel, MessageDefinition, MessageDirection,
    MessageElement, ResponseAttribute, MESSAGE_REGISTRY,
};

use crate::hoppie_client::parse_cpdlc_packet;

// ─── Hoppie → OpenLink ─────────────────────────────────────────────

/// Convert a Hoppie CPDLC packet into an OpenLink [`CpdlcEnvelope`].
///
/// `from_callsign` is the callsign of the Hoppie sender.
/// `to_callsign` is the bridge callsign that polled this message.
/// `min` is the bridge-assigned MIN for OpenLink.
pub fn hoppie_to_openlink(
    from_callsign: &str,
    to_callsign: &str,
    packet_str: &str,
    min: u8,
    direction: MessageDirection,
) -> Result<CpdlcEnvelope> {
    let pkt =
        parse_cpdlc_packet(packet_str).context("failed to parse Hoppie CPDLC packet")?;

    // Match the body text against the message registry to find the element ID
    let (msg_id, args) = match_body_to_element(&pkt.body, direction);

    let mrn: Option<u8> = pkt
        .mrn
        .as_ref()
        .and_then(|s| s.parse::<u8>().ok());

    let element = MessageElement::new(msg_id, args);

    let app_msg = CpdlcApplicationMessage {
        min,
        mrn,
        elements: vec![element],
        timestamp: Utc::now(),
    };

    Ok(CpdlcEnvelope {
        source: AcarsEndpointCallsign::new(from_callsign),
        destination: AcarsEndpointCallsign::new(to_callsign),
        message: CpdlcMessageType::Application(app_msg),
    })
}

/// Match a rendered body text against the message registry.
///
/// Returns the message ID (e.g. "DM6") and extracted typed arguments.
/// Falls back to a free-text DM/UM67 if no match is found.
fn match_body_to_element(
    body: &str,
    direction: MessageDirection,
) -> (String, Vec<CpdlcArgument>) {
    let body = body.trim();
    let dir_filter = direction;

    // Collect all matching definitions, then pick the most specific one
    // (most static text in template = higher specificity).
    let mut best: Option<(&MessageDefinition, Vec<CpdlcArgument>, usize)> = None;

    for def in MESSAGE_REGISTRY.iter().filter(|d| d.direction == dir_filter) {
        if let Some(args) = try_match_template(body, def) {
            let specificity = template_specificity(def.template);
            if best.as_ref().map_or(true, |(_, _, best_s)| specificity > *best_s) {
                best = Some((def, args, specificity));
            }
        }
    }

    if let Some((def, args, _)) = best {
        return (def.id.to_string(), args);
    }

    // Fallback: wrap as free text using DM67 (downlink) or UM169 (uplink)
    let fallback_id = match direction {
        MessageDirection::Downlink => "DM67",
        MessageDirection::Uplink => "UM169",
    };
    (
        fallback_id.to_string(),
        vec![CpdlcArgument::FreeText(body.to_string())],
    )
}

/// Compute a specificity score for a template: the total length of static
/// (non-placeholder) text. Higher = more specific match.
fn template_specificity(template: &str) -> usize {
    let segments = split_template(template);
    segments.iter().map(|s| s.trim().len()).sum()
}

/// Try to match a body against a definition's template.
///
/// Returns `Some(args)` if the template matches, `None` otherwise.
fn try_match_template(body: &str, def: &MessageDefinition) -> Option<Vec<CpdlcArgument>> {
    if def.args.is_empty() {
        // No-arg template: exact match
        if body.eq_ignore_ascii_case(def.template) {
            return Some(vec![]);
        }
        return None;
    }

    // Split template on placeholders to get static segments
    let segments = split_template(def.template);
    let values = extract_values_between_segments(body, &segments);

    if values.len() != def.args.len() {
        return None;
    }

    // Verify all values are non-empty
    if values.iter().any(|v| v.is_empty()) {
        return None;
    }

    let args = values
        .iter()
        .zip(def.args.iter())
        .map(|(val, arg_type)| text_to_argument(val, *arg_type))
        .collect();

    Some(args)
}

// ─── OpenLink → Hoppie ─────────────────────────────────────────────

/// Convert an OpenLink CPDLC application message into a Hoppie packet string.
///
/// Returns `None` for meta messages (logon, connection, etc.) which have no
/// direct Hoppie equivalent.
pub fn openlink_to_hoppie(envelope: &CpdlcEnvelope) -> Option<HoppiePacketOut> {
    let app = match &envelope.message {
        CpdlcMessageType::Application(app) => app,
        CpdlcMessageType::Meta(_) => return None,
    };

    // Render all elements to text
    let body = app.render();

    // Compute response attribute
    let ra = app.effective_response_attr();
    let ra_str = response_attr_to_hoppie(ra);

    // MRN
    let mrn_str = app.mrn.map(|m| m.to_string());

    let packet = crate::hoppie_client::format_cpdlc_packet(
        &app.min.to_string(),
        mrn_str.as_deref(),
        ra_str,
        &body,
    );

    Some(HoppiePacketOut {
        peer: envelope.destination.to_string(),
        from: envelope.source.to_string(),
        packet,
        min: app.min,
    })
}

/// Output data for sending a CPDLC message via Hoppie.
#[derive(Debug, Clone)]
pub struct HoppiePacketOut {
    /// Peer (destination) callsign.
    pub peer: String,
    /// Sender callsign.
    pub from: String,
    /// Formatted `/data2/...` packet.
    pub packet: String,
    /// The MIN of this message (for session tracking).
    pub min: u8,
}

// ─── Helpers ────────────────────────────────────────────────────────

/// Convert an OpenLink [`ResponseAttribute`] to the Hoppie shorthand string.
fn response_attr_to_hoppie(ra: ResponseAttribute) -> &'static str {
    match ra {
        ResponseAttribute::WU => "WU",
        ResponseAttribute::AN => "AN",
        ResponseAttribute::R => "R",
        ResponseAttribute::Y => "Y",
        ResponseAttribute::N => "N",
        ResponseAttribute::NE => "NE",
    }
}

/// Parse a Hoppie response attribute shorthand back to OpenLink enum.
pub fn hoppie_ra_to_openlink(ra: &str) -> ResponseAttribute {
    match ra.to_uppercase().as_str() {
        "WU" | "W/U" => ResponseAttribute::WU,
        "AN" | "A/N" => ResponseAttribute::AN,
        "R" => ResponseAttribute::R,
        "Y" => ResponseAttribute::Y,
        "N" => ResponseAttribute::N,
        "NE" => ResponseAttribute::NE,
        _ => {
            warn!("unknown Hoppie response attribute: {ra}, defaulting to N");
            ResponseAttribute::N
        }
    }
}

/// Split a template like "CLIMB TO [level] BY [time]" into static segments:
/// `["CLIMB TO ", " BY ", ""]`
fn split_template(template: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut remaining = template;

    loop {
        if let Some(start) = remaining.find('[') {
            segments.push(remaining[..start].to_string());
            if let Some(end) = remaining[start..].find(']') {
                remaining = &remaining[start + end + 1..];
            } else {
                break;
            }
        } else {
            segments.push(remaining.to_string());
            break;
        }
    }

    segments
}

/// Given static segments extracted from a template, extract the dynamic
/// values from the body text.
fn extract_values_between_segments(body: &str, segments: &[String]) -> Vec<String> {
    let mut values = Vec::new();
    let mut remaining = body;

    for (i, seg) in segments.iter().enumerate() {
        if i == 0 {
            if !seg.is_empty() {
                if let Some(rest) = remaining.strip_prefix(seg.trim_end()) {
                    remaining = rest.trim_start();
                } else {
                    return values;
                }
            }
        } else {
            if seg.is_empty() {
                values.push(remaining.trim().to_string());
                remaining = "";
            } else {
                let needle = seg.trim();
                if let Some(pos) = remaining.find(needle) {
                    values.push(remaining[..pos].trim().to_string());
                    remaining = &remaining[pos + needle.len()..];
                    remaining = remaining.trim_start();
                } else {
                    values.push(remaining.trim().to_string());
                    return values;
                }
            }
        }
    }

    values
}

/// Convert a raw text value to a typed [`CpdlcArgument`] based on the
/// expected [`ArgType`].
fn text_to_argument(value: &str, arg_type: ArgType) -> CpdlcArgument {
    match arg_type {
        ArgType::Level => {
            if let Ok(fl) = value.parse::<FlightLevel>() {
                CpdlcArgument::Level(fl)
            } else {
                CpdlcArgument::FreeText(value.to_string())
            }
        }
        ArgType::Speed => CpdlcArgument::Speed(value.to_string()),
        ArgType::Time => CpdlcArgument::Time(value.to_string()),
        ArgType::Position => CpdlcArgument::Position(value.to_string()),
        ArgType::Direction => CpdlcArgument::Direction(value.to_string()),
        ArgType::Degrees => {
            if let Ok(d) = value.parse::<u16>() {
                CpdlcArgument::Degrees(d)
            } else {
                CpdlcArgument::Degrees(0)
            }
        }
        ArgType::Distance => CpdlcArgument::Distance(value.to_string()),
        ArgType::RouteClearance => CpdlcArgument::RouteClearance(value.to_string()),
        ArgType::ProcedureName => CpdlcArgument::ProcedureName(value.to_string()),
        ArgType::UnitName => CpdlcArgument::UnitName(value.to_string()),
        ArgType::FacilityDesignation => CpdlcArgument::FacilityDesignation(value.to_string()),
        ArgType::Frequency => CpdlcArgument::Frequency(value.to_string()),
        ArgType::Code => CpdlcArgument::Code(value.to_string()),
        ArgType::AtisCode => CpdlcArgument::AtisCode(value.to_string()),
        ArgType::ErrorInfo => CpdlcArgument::ErrorInfo(value.to_string()),
        ArgType::FreeText => CpdlcArgument::FreeText(value.to_string()),
        ArgType::VerticalRate => CpdlcArgument::VerticalRate(value.to_string()),
        ArgType::Altimeter => CpdlcArgument::Altimeter(value.to_string()),
        ArgType::LegType => CpdlcArgument::LegType(value.to_string()),
        ArgType::PositionReport => CpdlcArgument::PositionReport(value.to_string()),
        ArgType::RemainingFuel => CpdlcArgument::RemainingFuel(value.to_string()),
        ArgType::PersonsOnBoard => CpdlcArgument::PersonsOnBoard(value.to_string()),
        ArgType::SpeedType => CpdlcArgument::SpeedType(value.to_string()),
        ArgType::DepartureClearance => CpdlcArgument::DepartureClearance(value.to_string()),
    }
}

/// Check if a Hoppie CPDLC body text is a logon-related message.
pub fn is_logon_body(body: &str) -> bool {
    let body = body.trim().to_uppercase();
    body == "REQUEST LOGON" || body == "LOGON"
}

/// Check if a Hoppie CPDLC body text is a logon response.
pub fn is_logon_response_body(body: &str) -> Option<bool> {
    let body = body.trim().to_uppercase();
    if body == "LOGON ACCEPTED" {
        Some(true)
    } else if body == "LOGON REJECTED" {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_template() {
        let segments = split_template("CLIMB TO [level]");
        assert_eq!(segments, vec!["CLIMB TO ", ""]);
    }

    #[test]
    fn test_split_template_multi_arg() {
        let segments = split_template("AT [time] CLIMB TO [level]");
        assert_eq!(segments, vec!["AT ", " CLIMB TO ", ""]);
    }

    #[test]
    fn test_extract_values_single() {
        let segments = vec!["CLIMB TO ".to_string(), "".to_string()];
        let values = extract_values_between_segments("CLIMB TO FL350", &segments);
        assert_eq!(values, vec!["FL350"]);
    }

    #[test]
    fn test_extract_values_multi() {
        let segments = vec!["AT ".to_string(), " CLIMB TO ".to_string(), "".to_string()];
        let values = extract_values_between_segments("AT 1430 CLIMB TO FL350", &segments);
        assert_eq!(values, vec!["1430", "FL350"]);
    }

    #[test]
    fn test_hoppie_to_openlink_climb() {
        // Hoppie format: /data2/{min}/{mrn}/{RA}/{body}
        let env = hoppie_to_openlink(
            "AFR123",
            "LFPG",
            "/data2/5//WU/CLIMB TO FL350",
            1,
            MessageDirection::Uplink,
        )
        .unwrap();

        assert_eq!(env.source.to_string(), "AFR123");
        assert_eq!(env.destination.to_string(), "LFPG");
        match &env.message {
            CpdlcMessageType::Application(app) => {
                assert_eq!(app.min, 1);
                assert!(app.mrn.is_none());
                assert_eq!(app.elements.len(), 1);
                assert_eq!(app.elements[0].id, "UM20");
                assert_eq!(app.elements[0].args.len(), 1);
                assert_eq!(
                    app.elements[0].args[0],
                    CpdlcArgument::Level(FlightLevel::new(350))
                );
            }
            _ => panic!("expected Application message"),
        }
    }

    #[test]
    fn test_hoppie_to_openlink_wilco() {
        let env = hoppie_to_openlink(
            "AFR123",
            "LFPG",
            "/data2/6/5/N/WILCO",
            2,
            MessageDirection::Downlink,
        )
        .unwrap();

        match &env.message {
            CpdlcMessageType::Application(app) => {
                assert_eq!(app.min, 2);
                assert_eq!(app.mrn, Some(5));
                assert_eq!(app.elements[0].id, "DM0");
            }
            _ => panic!("expected Application message"),
        }
    }

    #[test]
    fn test_openlink_to_hoppie_wilco() {
        let env = CpdlcEnvelope {
            source: AcarsEndpointCallsign::new("AFR123"),
            destination: AcarsEndpointCallsign::new("LFPG"),
            message: CpdlcMessageType::Application(CpdlcApplicationMessage {
                min: 2,
                mrn: Some(1),
                elements: vec![MessageElement::new("DM0", vec![])],
                timestamp: Utc::now(),
            }),
        };

        let out = openlink_to_hoppie(&env).unwrap();
        assert_eq!(out.from, "AFR123");
        assert_eq!(out.peer, "LFPG");
        // New format: /data2/{min}/{mrn}/{RA}/{body}
        assert_eq!(out.packet, "/data2/2/1/N/WILCO");
    }

    #[test]
    fn test_response_attr_roundtrip() {
        for ra in [
            ResponseAttribute::WU,
            ResponseAttribute::AN,
            ResponseAttribute::R,
            ResponseAttribute::Y,
            ResponseAttribute::N,
            ResponseAttribute::NE,
        ] {
            let hoppie = response_attr_to_hoppie(ra);
            let back = hoppie_ra_to_openlink(hoppie);
            assert_eq!(ra, back, "roundtrip failed for {ra:?}");
        }
    }

    #[test]
    fn test_is_logon_body() {
        assert!(is_logon_body("REQUEST LOGON"));
        assert!(is_logon_body("request logon"));
        assert!(is_logon_body("LOGON"));
        assert!(!is_logon_body("CLIMB TO FL350"));
    }
}
