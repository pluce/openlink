//! ACARS (Aircraft Communications Addressing and Reporting System) types.
//!
//! This module models the ACARS application layer that rides on top of the
//! OpenLink network. An [`AcarsEnvelope`] wraps an [`AcarsMessage`] together
//! with its [`AcarsRouting`] information (primarily identifying the aircraft).

use std::convert::Infallible;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::cpdlc::CpdlcEnvelope;

// ---------------------------------------------------------------------------
// AcarsEndpointCallsign
// ---------------------------------------------------------------------------

/// The callsign component of an ACARS endpoint (e.g. `"AFR1234"` or `"LFPG"`).
///
/// In the CPDLC context this is used to identify both aircraft and ground
/// stations.
///
/// # Examples
///
/// ```
/// use openlink_models::AcarsEndpointCallsign;
///
/// let cs: AcarsEndpointCallsign = "AFR1234".into();
/// assert_eq!(cs.to_string(), "AFR1234");
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AcarsEndpointCallsign(String);

impl AcarsEndpointCallsign {
    /// Create a new callsign.
    pub fn new(callsign: &str) -> Self {
        Self(callsign.to_string())
    }

    /// Return the inner string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AcarsEndpointCallsign {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for AcarsEndpointCallsign {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for AcarsEndpointCallsign {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl FromStr for AcarsEndpointCallsign {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

// ---------------------------------------------------------------------------
// AcarsEndpointAddress
// ---------------------------------------------------------------------------

/// The addressing component of an ACARS endpoint.
///
/// Represents the datalink address used to route ACARS messages to/from
/// an aircraft or ground station.
///
/// # Examples
///
/// ```
/// use openlink_models::AcarsEndpointAddress;
///
/// let addr: AcarsEndpointAddress = "ADDR001".into();
/// assert_eq!(addr.to_string(), "ADDR001");
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AcarsEndpointAddress(String);

impl AcarsEndpointAddress {
    /// Create a new ACARS endpoint address.
    pub fn new(address: &str) -> Self {
        Self(address.to_string())
    }

    /// Return the inner string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AcarsEndpointAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for AcarsEndpointAddress {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for AcarsEndpointAddress {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl FromStr for AcarsEndpointAddress {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

// ---------------------------------------------------------------------------
// AcarsRoutingEndpoint
// ---------------------------------------------------------------------------

/// Identifies one party in an ACARS exchange (callsign + datalink address).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AcarsRoutingEndpoint {
    /// The callsign of this endpoint (e.g. `"AFR1234"` or `"LFPG"`).
    pub callsign: AcarsEndpointCallsign,
    /// The datalink address of this endpoint.
    pub address: AcarsEndpointAddress,
}

impl AcarsRoutingEndpoint {
    /// Construct a new routing endpoint from types that can be converted into
    /// [`AcarsEndpointCallsign`] and [`AcarsEndpointAddress`].
    pub fn new(
        callsign: impl Into<AcarsEndpointCallsign>,
        address: impl Into<AcarsEndpointAddress>,
    ) -> Self {
        Self {
            callsign: callsign.into(),
            address: address.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// AcarsRouting
// ---------------------------------------------------------------------------

/// Routing information attached to every [`AcarsEnvelope`].
///
/// In a real Air↔Ground ACARS link the message implicitly flows from aircraft
/// to ground (or vice-versa). Here we carry the aircraft identity explicitly.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AcarsRouting {
    /// The aircraft endpoint associated with this message.
    pub aircraft: AcarsRoutingEndpoint,
}

// ---------------------------------------------------------------------------
// AcarsEnvelope
// ---------------------------------------------------------------------------

/// An ACARS-level message envelope.
///
/// Combines [`AcarsRouting`] (who) with an [`AcarsMessage`] (what).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AcarsEnvelope {
    /// Routing information (aircraft identity).
    pub routing: AcarsRouting,
    /// The application-level message.
    pub message: AcarsMessage,
}

// ---------------------------------------------------------------------------
// AcarsMessage
// ---------------------------------------------------------------------------

/// The payload of an [`AcarsEnvelope`].
///
/// Currently only CPDLC is implemented; future variants may include ADS-B,
/// AOC, etc.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum AcarsMessage {
    /// A CPDLC (Controller-Pilot Data Link Communications) message.
    CPDLC(CpdlcEnvelope),
    // Future: ADSB(AdsbMessage), AOC(AocMessage), …
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpdlc::{CpdlcMessage, CpdlcMessageType, FlightLevel};

    #[test]
    fn callsign_display_and_from() {
        let cs = AcarsEndpointCallsign::new("AFR1234");
        assert_eq!(cs.to_string(), "AFR1234");
        assert_eq!(cs.as_str(), "AFR1234");

        let cs2: AcarsEndpointCallsign = "AFR1234".into();
        assert_eq!(cs, cs2);
    }

    #[test]
    fn address_display_and_from() {
        let addr = AcarsEndpointAddress::new("ADDR001");
        assert_eq!(addr.to_string(), "ADDR001");

        let addr2: AcarsEndpointAddress = "ADDR001".into();
        assert_eq!(addr, addr2);
    }

    #[test]
    fn routing_endpoint_new() {
        let ep = AcarsRoutingEndpoint::new("AFR1234", "ADDR001");
        assert_eq!(ep.callsign.as_str(), "AFR1234");
        assert_eq!(ep.address.as_str(), "ADDR001");
    }

    #[test]
    fn acars_envelope_serde_roundtrip() {
        let envelope = AcarsEnvelope {
            routing: AcarsRouting {
                aircraft: AcarsRoutingEndpoint::new("AFR1234", "ADDR001"),
            },
            message: AcarsMessage::CPDLC(CpdlcEnvelope {
                source: "AFR1234".into(),
                destination: "LFPG".into(),
                message: CpdlcMessageType::Application(
                    CpdlcMessage::UplinkClimbToFlightLevel {
                        level: FlightLevel::new(350),
                    },
                ),
            }),
        };
        let json = serde_json::to_string(&envelope).unwrap();
        let back: AcarsEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(envelope, back);
    }

    #[test]
    fn callsign_hash_usable_as_map_key() {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(AcarsEndpointCallsign::new("AFR1234"), 42);
        assert_eq!(map.get(&AcarsEndpointCallsign::new("AFR1234")), Some(&42));
    }
}
