//! CPDLC (Controller–Pilot Data Link Communications) message types.
//!
//! This module defines the full CPDLC message hierarchy:
//!
//! - [`CpdlcEnvelope`] — wraps a CPDLC message with source/destination callsigns.
//! - [`CpdlcMessageType`] — distinguishes application-level messages from meta
//!   (logon, connection, contact, transfer) messages.
//! - [`CpdlcMessage`] — concrete uplink/downlink application messages.
//! - [`CpdlcMetaMessage`] — protocol-level handshake and session management messages.
//! - [`SerializedMessagePayload`] — human-readable text serialisation of any CPDLC message.
//! - [`ICAOAirportCode`] — a validated four-letter ICAO airport designator.
//! - [`FlightLevel`] — a typed flight level value.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::acars::{AcarsEndpointCallsign, AcarsRoutingEndpoint};
use crate::error::ModelError;

// ---------------------------------------------------------------------------
// ICAOAirportCode
// ---------------------------------------------------------------------------

/// A validated four-letter ICAO airport code (e.g. `"LFPG"`, `"KJFK"`).
///
/// Use [`TryFrom`] or [`FromStr`] for validated construction, or [`new`](Self::new)
/// for an unchecked path (e.g. when the value is already known to be valid).
///
/// # Examples
///
/// ```
/// use openlink_models::ICAOAirportCode;
///
/// let code = ICAOAirportCode::new("LFPG");
/// assert_eq!(code.to_string(), "LFPG");
///
/// // Validated construction
/// let parsed: ICAOAirportCode = "KJFK".parse().unwrap();
/// assert_eq!(parsed.as_str(), "KJFK");
///
/// // Invalid codes are rejected
/// assert!("123".parse::<ICAOAirportCode>().is_err());
/// assert!("lfpg".parse::<ICAOAirportCode>().is_err());
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ICAOAirportCode(String);

impl ICAOAirportCode {
    /// Create a new airport code **without validation**.
    ///
    /// Prefer [`TryFrom`] or [`FromStr`] when the input is untrusted.
    pub fn new(code: &str) -> Self {
        Self(code.to_string())
    }

    /// Return the inner string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validate that a string is a well-formed ICAO airport code
    /// (exactly 4 uppercase ASCII letters A-Z).
    fn validate(s: &str) -> Result<(), ModelError> {
        if s.len() != 4 || !s.bytes().all(|b| b.is_ascii_uppercase()) {
            Err(ModelError::InvalidICAOCode {
                value: s.to_string(),
                reason: "must be exactly 4 uppercase ASCII letters".to_string(),
            })
        } else {
            Ok(())
        }
    }
}

impl fmt::Display for ICAOAirportCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<&str> for ICAOAirportCode {
    type Error = ModelError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::validate(s)?;
        Ok(Self(s.to_string()))
    }
}

impl TryFrom<String> for ICAOAirportCode {
    type Error = ModelError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::validate(&s)?;
        Ok(Self(s))
    }
}

impl FromStr for ICAOAirportCode {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

// ---------------------------------------------------------------------------
// FlightLevel
// ---------------------------------------------------------------------------

/// A typed flight level (e.g. FL350 corresponds to `FlightLevel(350)`).
///
/// Serialises as a bare `u16` for compactness.
///
/// # Examples
///
/// ```
/// use openlink_models::FlightLevel;
///
/// let fl = FlightLevel::new(350);
/// assert_eq!(fl.to_string(), "FL350");
/// assert_eq!(fl.value(), 350);
///
/// let parsed: FlightLevel = "FL350".parse().unwrap();
/// assert_eq!(parsed, fl);
///
/// let parsed2: FlightLevel = "350".parse().unwrap();
/// assert_eq!(parsed2, fl);
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FlightLevel(u16);

impl FlightLevel {
    /// Create a new flight level from a numeric value.
    ///
    /// The value represents hundreds of feet (e.g. `350` → FL350 = 35 000 ft).
    pub fn new(level: u16) -> Self {
        Self(level)
    }

    /// Return the numeric value.
    pub fn value(self) -> u16 {
        self.0
    }
}

impl fmt::Display for FlightLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FL{}", self.0)
    }
}

impl From<u16> for FlightLevel {
    fn from(level: u16) -> Self {
        Self(level)
    }
}

impl FromStr for FlightLevel {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let numeric = s.strip_prefix("FL").unwrap_or(s);
        let level: u16 = numeric
            .parse()
            .map_err(|_| ModelError::InvalidFlightLevel {
                value: s.to_string(),
                reason: "must be a number between 0 and 999".to_string(),
            })?;
        if level > 999 {
            return Err(ModelError::InvalidFlightLevel {
                value: s.to_string(),
                reason: "must be a number between 0 and 999".to_string(),
            });
        }
        Ok(Self(level))
    }
}

// ---------------------------------------------------------------------------
// CpdlcEnvelope
// ---------------------------------------------------------------------------

/// A CPDLC message envelope carrying source, destination, and payload.
///
/// The `source` and `destination` are ACARS-level callsigns (aircraft or
/// ground station identifiers).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CpdlcEnvelope {
    /// The originator of this CPDLC message (callsign).
    pub source: AcarsEndpointCallsign,
    /// The intended recipient (callsign).
    pub destination: AcarsEndpointCallsign,
    /// The message content.
    pub message: CpdlcMessageType,
}

// ---------------------------------------------------------------------------
// CpdlcMessageType
// ---------------------------------------------------------------------------

/// Discriminator between application-level and meta (protocol) CPDLC messages.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum CpdlcMessageType {
    /// An operational CPDLC message (clearance, request, etc.).
    Application(CpdlcMessage),
    /// A session-management / protocol message (logon, connection, contact…).
    Meta(CpdlcMetaMessage),
}

impl From<CpdlcMessageType> for SerializedMessagePayload {
    fn from(value: CpdlcMessageType) -> Self {
        match value {
            CpdlcMessageType::Application(msg) => msg.into(),
            CpdlcMessageType::Meta(meta) => meta.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// CpdlcMetaMessage
// ---------------------------------------------------------------------------

/// Protocol-level CPDLC messages used for session management.
///
/// These messages handle the logon / connection / contact / transfer lifecycle
/// between an aircraft and successive ATC ground stations.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum CpdlcMetaMessage {
    /// Aircraft requests logon with a ground station.
    LogonRequest {
        /// The target ground station callsign.
        station: AcarsEndpointCallsign,
        /// ICAO code of the flight-plan origin airport.
        flight_plan_origin: ICAOAirportCode,
        /// ICAO code of the flight-plan destination airport.
        flight_plan_destination: ICAOAirportCode,
    },

    /// Ground station responds to a logon request.
    LogonResponse {
        /// Whether the logon was accepted.
        accepted: bool,
    },

    ///  Ground station requests a CPDLC data connection.
    ConnectionRequest,

    /// Aircraft responds to a connection request.
    ConnectionResponse {
        /// Whether the connection was accepted.
        accepted: bool,
    },

    /// Ground station instructs the aircraft to contact another station.
    ContactRequest {
        /// The next station the aircraft should contact.
        station: AcarsEndpointCallsign,
    },

    /// Aircraft responds to a contact request.
    ContactResponse {
        /// Whether the aircraft accepts the contact instruction.
        accepted: bool,
    },

    /// Aircraft confirms that the contact handover is complete.
    ContactComplete,

    /// Server-side forwarding of logon credentials to a new station.
    LogonForward {
        /// The callsign of the flight being forwarded.
        flight: AcarsEndpointCallsign,
        /// ICAO code of the flight-plan origin airport.
        flight_plan_origin: ICAOAirportCode,
        /// ICAO code of the flight-plan destination airport.
        flight_plan_destination: ICAOAirportCode,
        /// The station that should receive the logon.
        new_station: AcarsEndpointCallsign,
    },

    /// Notification of the Next Data Authority for a flight.
    NextDataAuthority {
        /// The new data authority endpoint.
        nda: AcarsRoutingEndpoint,
    },
}

impl From<CpdlcMetaMessage> for SerializedMessagePayload {
    fn from(value: CpdlcMetaMessage) -> Self {
        let text = match value {
            CpdlcMetaMessage::LogonRequest {
                station,
                flight_plan_origin,
                flight_plan_destination,
            } => format!(
                "LOGON REQUEST TO {} - FP ORIGIN {} DEST {}",
                station, flight_plan_origin, flight_plan_destination
            ),
            CpdlcMetaMessage::LogonResponse { accepted } => {
                format!("LOGON {}", if accepted { "ACCEPTED" } else { "REJECTED" })
            }
            CpdlcMetaMessage::ConnectionRequest => "CONNECTION REQUEST".to_string(),
            CpdlcMetaMessage::ConnectionResponse { accepted } => {
                format!(
                    "CONNECTION {}",
                    if accepted { "ACCEPTED" } else { "REJECTED" }
                )
            }
            CpdlcMetaMessage::ContactRequest { station } => {
                format!("CONTACT {}", station)
            }
            CpdlcMetaMessage::ContactResponse { accepted } => {
                format!("CONTACT {}", if accepted { "ACCEPTED" } else { "REJECTED" })
            }
            CpdlcMetaMessage::ContactComplete => "CONTACT COMPLETE".to_string(),
            CpdlcMetaMessage::LogonForward {
                flight,
                flight_plan_origin,
                flight_plan_destination,
                new_station,
            } => format!(
                "LOGON FORWARD FLIGHT {} ORIGIN {} DEST {} NEW STATION {}",
                flight, flight_plan_origin, flight_plan_destination, new_station
            ),
            CpdlcMetaMessage::NextDataAuthority { nda } => {
                format!("NEXT DATA AUTHORITY {} {}", nda.callsign, nda.address)
            }
        };
        SerializedMessagePayload(text)
    }
}

// ---------------------------------------------------------------------------
// CpdlcMessage
// ---------------------------------------------------------------------------

/// Operational (application-level) CPDLC messages.
///
/// Each variant corresponds to a message element from the ICAO Doc 4444
/// CPDLC message reference set.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum CpdlcMessage {
    /// ATC instructs the aircraft to climb to a given flight level.
    UplinkClimbToFlightLevel {
        /// Target flight level (e.g. `FL350`).
        level: FlightLevel,
    },
    /// Aircraft requests a level change.
    DownlinkRequestLevelChange {
        /// Requested flight level (e.g. `FL390`).
        level: FlightLevel,
    },
}

impl From<CpdlcMessage> for SerializedMessagePayload {
    fn from(value: CpdlcMessage) -> Self {
        let text = match value {
            CpdlcMessage::UplinkClimbToFlightLevel { level } => {
                format!("CLIMB TO {level}")
            }
            CpdlcMessage::DownlinkRequestLevelChange { level } => {
                format!("REQUEST CLIMB TO {level}")
            }
        };
        SerializedMessagePayload(text)
    }
}

// ---------------------------------------------------------------------------
// SerializedMessagePayload
// ---------------------------------------------------------------------------

/// Human-readable text representation of a CPDLC message.
///
/// Produced by converting a [`CpdlcMessage`], [`CpdlcMetaMessage`], or
/// [`CpdlcMessageType`] via the `From` / `Into` trait.
///
/// # Examples
///
/// ```
/// use openlink_models::{CpdlcMessage, FlightLevel, SerializedMessagePayload};
///
/// let msg = CpdlcMessage::UplinkClimbToFlightLevel { level: FlightLevel::new(350) };
/// let payload: SerializedMessagePayload = msg.into();
/// assert_eq!(payload.to_string(), "CLIMB TO FL350");
/// ```
pub struct SerializedMessagePayload(pub(crate) String);

impl SerializedMessagePayload {
    /// Return the inner string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SerializedMessagePayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- ICAOAirportCode ---------------------------------------------------

    #[test]
    fn icao_code_new_and_display() {
        let code = ICAOAirportCode::new("LFPG");
        assert_eq!(code.to_string(), "LFPG");
        assert_eq!(code.as_str(), "LFPG");
    }

    #[test]
    fn icao_code_try_from_valid() {
        let code = ICAOAirportCode::try_from("LFPG").unwrap();
        assert_eq!(code.as_str(), "LFPG");
    }

    #[test]
    fn icao_code_try_from_string_valid() {
        let code = ICAOAirportCode::try_from("KJFK".to_string()).unwrap();
        assert_eq!(code.as_str(), "KJFK");
    }

    #[test]
    fn icao_code_parse_valid() {
        let code: ICAOAirportCode = "EGLL".parse().unwrap();
        assert_eq!(code.as_str(), "EGLL");
    }

    #[test]
    fn icao_code_rejects_lowercase() {
        assert!(ICAOAirportCode::try_from("lfpg").is_err());
    }

    #[test]
    fn icao_code_rejects_wrong_length() {
        assert!(ICAOAirportCode::try_from("LFP").is_err());
        assert!(ICAOAirportCode::try_from("LFPGA").is_err());
    }

    #[test]
    fn icao_code_rejects_digits() {
        assert!(ICAOAirportCode::try_from("L1PG").is_err());
    }

    // -- FlightLevel -------------------------------------------------------

    #[test]
    fn flight_level_display() {
        assert_eq!(FlightLevel::new(350).to_string(), "FL350");
        assert_eq!(FlightLevel::new(0).to_string(), "FL0");
    }

    #[test]
    fn flight_level_parse_with_prefix() {
        let fl: FlightLevel = "FL350".parse().unwrap();
        assert_eq!(fl.value(), 350);
    }

    #[test]
    fn flight_level_parse_without_prefix() {
        let fl: FlightLevel = "350".parse().unwrap();
        assert_eq!(fl.value(), 350);
    }

    #[test]
    fn flight_level_from_u16() {
        let fl = FlightLevel::from(350u16);
        assert_eq!(fl, FlightLevel::new(350));
    }

    #[test]
    fn flight_level_rejects_over_999() {
        assert!("1000".parse::<FlightLevel>().is_err());
    }

    #[test]
    fn flight_level_rejects_non_numeric() {
        assert!("FLabc".parse::<FlightLevel>().is_err());
    }

    #[test]
    fn flight_level_ordering() {
        assert!(FlightLevel::new(350) > FlightLevel::new(290));
    }

    // -- SerializedMessagePayload ------------------------------------------

    #[test]
    fn serialized_payload_from_cpdlc_message() {
        let msg = CpdlcMessage::UplinkClimbToFlightLevel {
            level: FlightLevel::new(350),
        };
        let payload: SerializedMessagePayload = msg.into();
        assert_eq!(payload.to_string(), "CLIMB TO FL350");
    }

    #[test]
    fn serialized_payload_from_downlink() {
        let msg = CpdlcMessage::DownlinkRequestLevelChange {
            level: FlightLevel::new(390),
        };
        let payload: SerializedMessagePayload = msg.into();
        assert_eq!(payload.to_string(), "REQUEST CLIMB TO FL390");
    }

    // -- CpdlcMetaMessage serialisation ------------------------------------

    #[test]
    fn meta_logon_request_serialisation() {
        let meta = CpdlcMetaMessage::LogonRequest {
            station: "LFPG".into(),
            flight_plan_origin: ICAOAirportCode::new("LFPG"),
            flight_plan_destination: ICAOAirportCode::new("KJFK"),
        };
        let payload: SerializedMessagePayload = meta.into();
        assert_eq!(
            payload.to_string(),
            "LOGON REQUEST TO LFPG - FP ORIGIN LFPG DEST KJFK"
        );
    }

    #[test]
    fn meta_logon_response_accepted() {
        let meta = CpdlcMetaMessage::LogonResponse { accepted: true };
        let payload: SerializedMessagePayload = meta.into();
        assert_eq!(payload.to_string(), "LOGON ACCEPTED");
    }

    #[test]
    fn meta_logon_response_rejected() {
        let meta = CpdlcMetaMessage::LogonResponse { accepted: false };
        let payload: SerializedMessagePayload = meta.into();
        assert_eq!(payload.to_string(), "LOGON REJECTED");
    }

    #[test]
    fn meta_connection_request_serialisation() {
        let meta = CpdlcMetaMessage::ConnectionRequest;
        let payload: SerializedMessagePayload = meta.into();
        assert_eq!(payload.to_string(), "CONNECTION REQUEST");
    }

    #[test]
    fn meta_connection_response_accepted() {
        let meta = CpdlcMetaMessage::ConnectionResponse { accepted: true };
        let payload: SerializedMessagePayload = meta.into();
        assert_eq!(payload.to_string(), "CONNECTION ACCEPTED");
    }

    #[test]
    fn meta_contact_request_serialisation() {
        let meta = CpdlcMetaMessage::ContactRequest {
            station: "LFPG".into(),
        };
        let payload: SerializedMessagePayload = meta.into();
        assert_eq!(payload.to_string(), "CONTACT LFPG");
    }

    #[test]
    fn meta_contact_complete_serialisation() {
        let meta = CpdlcMetaMessage::ContactComplete;
        let payload: SerializedMessagePayload = meta.into();
        assert_eq!(payload.to_string(), "CONTACT COMPLETE");
    }

    #[test]
    fn meta_next_data_authority_serialisation() {
        use crate::acars::AcarsRoutingEndpoint;
        let meta = CpdlcMetaMessage::NextDataAuthority {
            nda: AcarsRoutingEndpoint::new("LFPG", "ADDR001"),
        };
        let payload: SerializedMessagePayload = meta.into();
        assert_eq!(payload.to_string(), "NEXT DATA AUTHORITY LFPG ADDR001");
    }

    // -- CpdlcMessageType delegation ---------------------------------------

    #[test]
    fn message_type_application_delegates() {
        let mt = CpdlcMessageType::Application(CpdlcMessage::UplinkClimbToFlightLevel {
            level: FlightLevel::new(350),
        });
        let payload: SerializedMessagePayload = mt.into();
        assert_eq!(payload.to_string(), "CLIMB TO FL350");
    }

    #[test]
    fn message_type_meta_delegates() {
        let mt = CpdlcMessageType::Meta(CpdlcMetaMessage::ConnectionRequest);
        let payload: SerializedMessagePayload = mt.into();
        assert_eq!(payload.to_string(), "CONNECTION REQUEST");
    }

    // -- Serde roundtrip ---------------------------------------------------

    #[test]
    fn cpdlc_envelope_serde_roundtrip() {
        let envelope = CpdlcEnvelope {
            source: "AFR1234".into(),
            destination: "LFPG".into(),
            message: CpdlcMessageType::Application(CpdlcMessage::UplinkClimbToFlightLevel {
                level: FlightLevel::new(350),
            }),
        };
        let json = serde_json::to_string(&envelope).unwrap();
        let back: CpdlcEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(envelope, back);
    }

    #[test]
    fn cpdlc_meta_serde_roundtrip() {
        let meta = CpdlcMetaMessage::LogonForward {
            flight: "AFR1234".into(),
            flight_plan_origin: ICAOAirportCode::new("LFPG"),
            flight_plan_destination: ICAOAirportCode::new("KJFK"),
            new_station: "EGLL".into(),
        };
        let json = serde_json::to_string(&meta).unwrap();
        let back: CpdlcMetaMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, back);
    }

    #[test]
    fn flight_level_serde_roundtrip() {
        let fl = FlightLevel::new(350);
        let json = serde_json::to_string(&fl).unwrap();
        assert_eq!(json, "350"); // serialises as bare u16
        let back: FlightLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(fl, back);
    }
}
