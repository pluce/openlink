//! Top-level OpenLink message envelope.
//!
//! Every message transiting the OpenLink network is wrapped in an
//! [`OpenLinkEnvelope`] which carries:
//!
//! - A unique message id.
//! - A UTC timestamp.
//! - An optional correlation id for request/response pairing.
//! - Network-level routing information ([`OpenLinkRouting`]).
//! - An authentication token.
//! - The actual payload ([`OpenLinkMessage`]).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::acars::AcarsEnvelope;
use crate::network::OpenLinkRouting;
use crate::station::MetaMessage;

// ---------------------------------------------------------------------------
// OpenLinkEnvelope
// ---------------------------------------------------------------------------

/// The outermost message envelope on the OpenLink network.
///
/// All communication between servers, ground stations, and aircraft gateways
/// is contained in this structure.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OpenLinkEnvelope {
    /// Unique message identifier (UUID v4).
    pub id: Uuid,
    /// Timestamp (UTC) at which the message was created.
    pub timestamp: DateTime<Utc>,
    /// Optional id linking this message to an earlier request.
    pub correlation_id: Option<String>,
    /// Network-level source → destination routing.
    pub routing: OpenLinkRouting,
    /// The actual message content.
    pub payload: OpenLinkMessage,
    /// Bearer / JWT token for authentication.
    pub token: String,
}

// ---------------------------------------------------------------------------
// OpenLinkMessage
// ---------------------------------------------------------------------------

/// Discriminator for the content of an [`OpenLinkEnvelope`].
///
/// New protocol families (ADS-B, AOC, …) will be added as additional variants.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum OpenLinkMessage {
    /// An ACARS-level message (contains CPDLC, and in the future ADS-B/AOC).
    Acars(AcarsEnvelope),
    /// A system-level meta message (station status, etc.).
    Meta(MetaMessage),
    // Future: Adsb(…), Aoc(…)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acars::{AcarsMessage, AcarsRouting, AcarsRoutingEndpoint};
    use crate::cpdlc::{
        CpdlcApplicationMessage, CpdlcArgument, CpdlcEnvelope, CpdlcMessageType, FlightLevel,
        MessageElement,
    };
    use crate::network::{NetworkId, OpenLinkRoutingEndpoint};

    /// Helper to build a minimal envelope for tests.
    fn sample_envelope() -> OpenLinkEnvelope {
        OpenLinkEnvelope {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            correlation_id: None,
            routing: OpenLinkRouting {
                source: OpenLinkRoutingEndpoint::Server(NetworkId::new("server-1")),
                destination: OpenLinkRoutingEndpoint::Server(NetworkId::new("gw-2")),
            },
            payload: OpenLinkMessage::Acars(AcarsEnvelope {
                routing: AcarsRouting {
                    aircraft: AcarsRoutingEndpoint::new("AFR1234", "ADDR001"),
                },
                message: AcarsMessage::CPDLC(CpdlcEnvelope {
                    source: "AFR1234".into(),
                    destination: "LFPG".into(),
                    message: CpdlcMessageType::Application(CpdlcApplicationMessage {
                        min: 1,
                        mrn: None,
                        elements: vec![MessageElement::new(
                            "UM20",
                            vec![CpdlcArgument::Level(FlightLevel::new(350))],
                        )],
                        timestamp: Utc::now(),
                    }),
                }),
            }),
            token: "tok".to_string(),
        }
    }

    #[test]
    fn envelope_serde_roundtrip() {
        let envelope = sample_envelope();
        let json = serde_json::to_string(&envelope).unwrap();
        let back: OpenLinkEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(envelope, back);
    }

    #[test]
    fn envelope_with_correlation_id() {
        let mut env = sample_envelope();
        env.correlation_id = Some("corr-42".to_string());
        let json = serde_json::to_string(&env).unwrap();
        let back: OpenLinkEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(back.correlation_id, Some("corr-42".to_string()));
    }

    #[test]
    fn meta_message_variant() {
        use crate::station::{StationId, StationStatus};
        let msg = OpenLinkMessage::Meta(MetaMessage::StationStatus(
            StationId::new("stn-1"),
            StationStatus::Online,
            AcarsRoutingEndpoint::new("LFPG", "ADDR001"),
        ));
        let json = serde_json::to_string(&msg).unwrap();
        let back: OpenLinkMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }
}
