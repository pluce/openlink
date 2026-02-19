//! CPDLC (Controller–Pilot Data Link Communications) message types.
//!
//! This module defines the full CPDLC message hierarchy with a data-driven
//! approach — operational messages are described by a static registry of
//! [`MessageDefinition`]s rather than individual enum variants.
//!
//! ## Key types
//!
//! - [`CpdlcEnvelope`] — wraps a CPDLC message with source/destination callsigns.
//! - [`CpdlcMessageType`] — distinguishes application-level messages from meta messages.
//! - [`CpdlcApplicationMessage`] — a dynamic, data-driven operational message.
//! - [`CpdlcMetaMessage`] — protocol-level handshake and session management.
//! - [`MessageDefinition`] — static description of a message template from the ICAO reference.
//! - [`CpdlcArgument`] — typed arguments that fill template placeholders.
//! - [`ResponseAttribute`] — what kind of reply a message expects (W/U, A/N, R, Y, N).
//! - [`CpdlcDialogue`] — tracks the open/closed state of a MIN↔MRN exchange.
//! - [`ICAOAirportCode`] — a validated four-letter ICAO airport designator.
//! - [`FlightLevel`] — a typed flight level value.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
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
// MessageDirection
// ---------------------------------------------------------------------------

/// Whether a CPDLC message element is an uplink (ATC → aircraft) or a
/// downlink (aircraft → ATC).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageDirection {
    /// ATC → aircraft (UM)
    Uplink,
    /// Aircraft → ATC (DM)
    Downlink,
}

impl fmt::Display for MessageDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageDirection::Uplink => write!(f, "UM"),
            MessageDirection::Downlink => write!(f, "DM"),
        }
    }
}

// ---------------------------------------------------------------------------
// ResponseAttribute
// ---------------------------------------------------------------------------

/// ICAO response attribute — dictates which replies are valid for closing
/// a CPDLC dialogue.
///
/// When combining multi-element messages, precedence is:
/// `WU > AN > R > Y > N`.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ResponseAttribute {
    /// No response required — dialogue closed immediately.
    N = 0,
    /// Any CPDLC message carrying the requested data closes the dialogue.
    Y = 1,
    /// Roger / Unable / Standby.
    R = 2,
    /// Affirm / Negative / Standby.
    AN = 3,
    /// Wilco / Unable / Standby / Not Current Data Authority / Error.
    WU = 4,
    /// Not Enabled (FANS 1/A specific) — system closes immediately.
    NE = 5,
}

impl fmt::Display for ResponseAttribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResponseAttribute::WU => write!(f, "W/U"),
            ResponseAttribute::AN => write!(f, "A/N"),
            ResponseAttribute::R => write!(f, "R"),
            ResponseAttribute::Y => write!(f, "Y"),
            ResponseAttribute::N => write!(f, "N"),
            ResponseAttribute::NE => write!(f, "NE"),
        }
    }
}

impl ResponseAttribute {
    /// Compute the effective response attribute for a multi-element message
    /// by taking the highest-precedence attribute.
    ///
    /// Precedence: `WU > AN > R > Y > N`.  `NE` is treated as `N` for
    /// precedence purposes.
    pub fn effective(attrs: &[ResponseAttribute]) -> ResponseAttribute {
        attrs
            .iter()
            .copied()
            .map(|a| if a == ResponseAttribute::NE { ResponseAttribute::N } else { a })
            .max()
            .unwrap_or(ResponseAttribute::N)
    }
}

// ---------------------------------------------------------------------------
// ArgType / CpdlcArgument
// ---------------------------------------------------------------------------

/// The kind of argument expected in a CPDLC message template placeholder.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArgType {
    Level,
    Speed,
    Time,
    Position,
    Direction,
    Degrees,
    Distance,
    RouteClearance,
    ProcedureName,
    UnitName,
    FacilityDesignation,
    Frequency,
    Code,
    AtisCode,
    ErrorInfo,
    FreeText,
    VerticalRate,
    Altimeter,
    LegType,
    PositionReport,
    RemainingFuel,
    PersonsOnBoard,
    SpeedType,
    DepartureClearance,
}

/// A typed argument value that fills a template placeholder.
///
/// Most variants carry a simple `String` representation today; `Level` is
/// strongly typed via [`FlightLevel`].  Further refinement (speed as a
/// numeric type, etc.) can be added without breaking the wire format.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum CpdlcArgument {
    Level(FlightLevel),
    Speed(String),
    Time(String),
    Position(String),
    Direction(String),
    Degrees(u16),
    Distance(String),
    RouteClearance(String),
    ProcedureName(String),
    UnitName(String),
    FacilityDesignation(String),
    Frequency(String),
    Code(String),
    AtisCode(String),
    ErrorInfo(String),
    FreeText(String),
    VerticalRate(String),
    Altimeter(String),
    LegType(String),
    PositionReport(String),
    RemainingFuel(String),
    PersonsOnBoard(String),
    SpeedType(String),
    DepartureClearance(String),
}

impl CpdlcArgument {
    /// Return the [`ArgType`] discriminant for this argument.
    pub fn arg_type(&self) -> ArgType {
        match self {
            CpdlcArgument::Level(_) => ArgType::Level,
            CpdlcArgument::Speed(_) => ArgType::Speed,
            CpdlcArgument::Time(_) => ArgType::Time,
            CpdlcArgument::Position(_) => ArgType::Position,
            CpdlcArgument::Direction(_) => ArgType::Direction,
            CpdlcArgument::Degrees(_) => ArgType::Degrees,
            CpdlcArgument::Distance(_) => ArgType::Distance,
            CpdlcArgument::RouteClearance(_) => ArgType::RouteClearance,
            CpdlcArgument::ProcedureName(_) => ArgType::ProcedureName,
            CpdlcArgument::UnitName(_) => ArgType::UnitName,
            CpdlcArgument::FacilityDesignation(_) => ArgType::FacilityDesignation,
            CpdlcArgument::Frequency(_) => ArgType::Frequency,
            CpdlcArgument::Code(_) => ArgType::Code,
            CpdlcArgument::AtisCode(_) => ArgType::AtisCode,
            CpdlcArgument::ErrorInfo(_) => ArgType::ErrorInfo,
            CpdlcArgument::FreeText(_) => ArgType::FreeText,
            CpdlcArgument::VerticalRate(_) => ArgType::VerticalRate,
            CpdlcArgument::Altimeter(_) => ArgType::Altimeter,
            CpdlcArgument::LegType(_) => ArgType::LegType,
            CpdlcArgument::PositionReport(_) => ArgType::PositionReport,
            CpdlcArgument::RemainingFuel(_) => ArgType::RemainingFuel,
            CpdlcArgument::PersonsOnBoard(_) => ArgType::PersonsOnBoard,
            CpdlcArgument::SpeedType(_) => ArgType::SpeedType,
            CpdlcArgument::DepartureClearance(_) => ArgType::DepartureClearance,
        }
    }
}

impl fmt::Display for CpdlcArgument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CpdlcArgument::Level(fl) => write!(f, "{fl}"),
            CpdlcArgument::Speed(s)
            | CpdlcArgument::Time(s)
            | CpdlcArgument::Position(s)
            | CpdlcArgument::Direction(s)
            | CpdlcArgument::Distance(s)
            | CpdlcArgument::RouteClearance(s)
            | CpdlcArgument::ProcedureName(s)
            | CpdlcArgument::UnitName(s)
            | CpdlcArgument::FacilityDesignation(s)
            | CpdlcArgument::Frequency(s)
            | CpdlcArgument::Code(s)
            | CpdlcArgument::AtisCode(s)
            | CpdlcArgument::ErrorInfo(s)
            | CpdlcArgument::FreeText(s)
            | CpdlcArgument::VerticalRate(s)
            | CpdlcArgument::Altimeter(s)
            | CpdlcArgument::LegType(s)
            | CpdlcArgument::PositionReport(s)
            | CpdlcArgument::RemainingFuel(s)
            | CpdlcArgument::PersonsOnBoard(s)
            | CpdlcArgument::SpeedType(s)
            | CpdlcArgument::DepartureClearance(s) => write!(f, "{s}"),
            CpdlcArgument::Degrees(d) => write!(f, "{d}"),
        }
    }
}

// ---------------------------------------------------------------------------
// MessageDefinition & Registry
// ---------------------------------------------------------------------------

/// Static description of one CPDLC message element from the ICAO reference.
///
/// Stored in [`MESSAGE_REGISTRY`] — looked up by ID at runtime to validate
/// arguments, render display text, and determine response behaviour.
#[derive(Debug, Clone, PartialEq)]
pub struct MessageDefinition {
    /// Message identifier, e.g. `"UM20"` or `"DM6"`.
    pub id: &'static str,
    /// Uplink (ATC→aircraft) or Downlink (aircraft→ATC).
    pub direction: MessageDirection,
    /// Human-readable template with placeholders like `[level]`.
    pub template: &'static str,
    /// Ordered list of expected argument types matching the template placeholders.
    pub args: &'static [ArgType],
    /// What kind of reply the message expects.
    pub response_attr: ResponseAttribute,
    /// Supported on FANS 1/A systems.
    pub fans: bool,
    /// Supported on ATN B1 systems.
    pub atn_b1: bool,
}

impl MessageDefinition {
    /// Render the message template by substituting placeholders with the
    /// provided arguments, in order.
    ///
    /// Placeholders are any `[…]` sequences in the template.  If there
    /// are more placeholders than arguments the remaining brackets are
    /// left as-is.
    pub fn render(&self, args: &[CpdlcArgument]) -> String {
        let mut result = self.template.to_string();
        for arg in args {
            if let Some(start) = result.find('[') {
                if let Some(end) = result[start..].find(']') {
                    result.replace_range(start..start + end + 1, &arg.to_string());
                }
            }
        }
        result
    }
}

/// Look up a [`MessageDefinition`] by its ID (e.g. `"UM20"`, `"DM0"`).
pub fn find_definition(id: &str) -> Option<&'static MessageDefinition> {
    MESSAGE_REGISTRY.iter().find(|d| d.id == id)
}

/// The complete CPDLC message registry.
///
/// Every entry maps an ICAO CPDLC message identifier to its template,
/// argument types, response attribute, and system support flags.
pub static MESSAGE_REGISTRY: &[MessageDefinition] = &[
    // ── Uplink: Responses, Ack, Connection ─────────────────────────
    MessageDefinition { id: "UM0",  direction: MessageDirection::Uplink, template: "UNABLE",                                          args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM1",  direction: MessageDirection::Uplink, template: "STANDBY",                                         args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM2",  direction: MessageDirection::Uplink, template: "REQUEST DEFERRED",                                args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: false },
    MessageDefinition { id: "UM3",  direction: MessageDirection::Uplink, template: "ROGER",                                           args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM4",  direction: MessageDirection::Uplink, template: "AFFIRM",                                          args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM5",  direction: MessageDirection::Uplink, template: "NEGATIVE",                                        args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM159", direction: MessageDirection::Uplink, template: "ERROR [error information]",                       args: &[ArgType::ErrorInfo],                       response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM160", direction: MessageDirection::Uplink, template: "NEXT DATA AUTHORITY [facility designation]",      args: &[ArgType::FacilityDesignation],             response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM161", direction: MessageDirection::Uplink, template: "END SERVICE",                                    args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: false },
    MessageDefinition { id: "UM162", direction: MessageDirection::Uplink, template: "MESSAGE NOT SUPPORTED BY THIS ATS UNIT",          args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM163", direction: MessageDirection::Uplink, template: "[facility designation]",                          args: &[ArgType::FacilityDesignation],             response_attr: ResponseAttribute::N,  fans: true,  atn_b1: false },
    MessageDefinition { id: "UM211", direction: MessageDirection::Uplink, template: "REQUEST FORWARDED",                               args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM227", direction: MessageDirection::Uplink, template: "LOGICAL ACKNOWLEDGEMENT",                         args: &[],                                         response_attr: ResponseAttribute::N,  fans: false, atn_b1: true  },

    // ── Uplink: Vertical Clearances ────────────────────────────────
    MessageDefinition { id: "UM19",  direction: MessageDirection::Uplink, template: "MAINTAIN [level]",                                args: &[ArgType::Level],                           response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM20",  direction: MessageDirection::Uplink, template: "CLIMB TO [level]",                                args: &[ArgType::Level],                           response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM21",  direction: MessageDirection::Uplink, template: "AT [time] CLIMB TO [level]",                      args: &[ArgType::Time, ArgType::Level],            response_attr: ResponseAttribute::WU, fans: true,  atn_b1: false },
    MessageDefinition { id: "UM22",  direction: MessageDirection::Uplink, template: "AT [position] CLIMB TO [level]",                  args: &[ArgType::Position, ArgType::Level],        response_attr: ResponseAttribute::WU, fans: true,  atn_b1: false },
    MessageDefinition { id: "UM23",  direction: MessageDirection::Uplink, template: "DESCEND TO [level]",                              args: &[ArgType::Level],                           response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM24",  direction: MessageDirection::Uplink, template: "AT [time] DESCEND TO [level]",                    args: &[ArgType::Time, ArgType::Level],            response_attr: ResponseAttribute::WU, fans: true,  atn_b1: false },
    MessageDefinition { id: "UM25",  direction: MessageDirection::Uplink, template: "AT [position] DESCEND TO [level]",                args: &[ArgType::Position, ArgType::Level],        response_attr: ResponseAttribute::WU, fans: true,  atn_b1: false },
    MessageDefinition { id: "UM26",  direction: MessageDirection::Uplink, template: "CLIMB TO REACH [level] BY [time]",                args: &[ArgType::Level, ArgType::Time],            response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM27",  direction: MessageDirection::Uplink, template: "CLIMB TO REACH [level] BY [position]",            args: &[ArgType::Level, ArgType::Position],        response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM28",  direction: MessageDirection::Uplink, template: "DESCEND TO REACH [level] BY [time]",              args: &[ArgType::Level, ArgType::Time],            response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM29",  direction: MessageDirection::Uplink, template: "DESCEND TO REACH [level] BY [position]",          args: &[ArgType::Level, ArgType::Position],        response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM30",  direction: MessageDirection::Uplink, template: "MAINTAIN BLOCK [level] TO [level]",               args: &[ArgType::Level, ArgType::Level],           response_attr: ResponseAttribute::WU, fans: true,  atn_b1: false },
    MessageDefinition { id: "UM31",  direction: MessageDirection::Uplink, template: "CLIMB TO AND MAINTAIN BLOCK [level] TO [level]",  args: &[ArgType::Level, ArgType::Level],           response_attr: ResponseAttribute::WU, fans: true,  atn_b1: false },
    MessageDefinition { id: "UM32",  direction: MessageDirection::Uplink, template: "DESCEND TO AND MAINTAIN BLOCK [level] TO [level]",args: &[ArgType::Level, ArgType::Level],           response_attr: ResponseAttribute::WU, fans: true,  atn_b1: false },
    MessageDefinition { id: "UM34",  direction: MessageDirection::Uplink, template: "CRUISE CLIMB TO [level]",                         args: &[ArgType::Level],                           response_attr: ResponseAttribute::WU, fans: true,  atn_b1: false },
    MessageDefinition { id: "UM36",  direction: MessageDirection::Uplink, template: "EXPEDITE CLIMB TO [level]",                       args: &[ArgType::Level],                           response_attr: ResponseAttribute::WU, fans: true,  atn_b1: false },
    MessageDefinition { id: "UM37",  direction: MessageDirection::Uplink, template: "EXPEDITE DESCENT TO [level]",                     args: &[ArgType::Level],                           response_attr: ResponseAttribute::WU, fans: true,  atn_b1: false },
    MessageDefinition { id: "UM38",  direction: MessageDirection::Uplink, template: "IMMEDIATELY CLIMB TO [level]",                    args: &[ArgType::Level],                           response_attr: ResponseAttribute::WU, fans: true,  atn_b1: false },
    MessageDefinition { id: "UM39",  direction: MessageDirection::Uplink, template: "IMMEDIATELY DESCEND TO [level]",                  args: &[ArgType::Level],                           response_attr: ResponseAttribute::WU, fans: true,  atn_b1: false },

    // ── Uplink: Crossing constraints & Route ───────────────────────
    MessageDefinition { id: "UM46",  direction: MessageDirection::Uplink, template: "CROSS [position] AT [level]",                     args: &[ArgType::Position, ArgType::Level],        response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM47",  direction: MessageDirection::Uplink, template: "CROSS [position] AT OR ABOVE [level]",             args: &[ArgType::Position, ArgType::Level],        response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM48",  direction: MessageDirection::Uplink, template: "CROSS [position] AT OR BELOW [level]",             args: &[ArgType::Position, ArgType::Level],        response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM49",  direction: MessageDirection::Uplink, template: "CROSS [position] AT AND MAINTAIN [level]",         args: &[ArgType::Position, ArgType::Level],        response_attr: ResponseAttribute::WU, fans: true,  atn_b1: false },
    MessageDefinition { id: "UM50",  direction: MessageDirection::Uplink, template: "CROSS [position] BETWEEN [level] AND [level]",     args: &[ArgType::Position, ArgType::Level, ArgType::Level], response_attr: ResponseAttribute::WU, fans: true, atn_b1: false },
    MessageDefinition { id: "UM51",  direction: MessageDirection::Uplink, template: "CROSS [position] AT [time]",                       args: &[ArgType::Position, ArgType::Time],         response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM52",  direction: MessageDirection::Uplink, template: "CROSS [position] AT OR BEFORE [time]",             args: &[ArgType::Position, ArgType::Time],         response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM53",  direction: MessageDirection::Uplink, template: "CROSS [position] AT OR AFTER [time]",              args: &[ArgType::Position, ArgType::Time],         response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM54",  direction: MessageDirection::Uplink, template: "CROSS [position] BETWEEN [time] AND [time]",       args: &[ArgType::Position, ArgType::Time, ArgType::Time], response_attr: ResponseAttribute::WU, fans: true, atn_b1: true },
    MessageDefinition { id: "UM55",  direction: MessageDirection::Uplink, template: "CROSS [position] AT [speed]",                      args: &[ArgType::Position, ArgType::Speed],        response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM61",  direction: MessageDirection::Uplink, template: "CROSS [position] AT AND MAINTAIN [level] AT [speed]", args: &[ArgType::Position, ArgType::Level, ArgType::Speed], response_attr: ResponseAttribute::WU, fans: true, atn_b1: true },
    MessageDefinition { id: "UM74",  direction: MessageDirection::Uplink, template: "PROCEED DIRECT TO [position]",                     args: &[ArgType::Position],                        response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM79",  direction: MessageDirection::Uplink, template: "CLEARED TO [position] VIA [route clearance]",      args: &[ArgType::Position, ArgType::RouteClearance], response_attr: ResponseAttribute::WU, fans: true, atn_b1: true },
    MessageDefinition { id: "UM80",  direction: MessageDirection::Uplink, template: "CLEARED [route clearance]",                        args: &[ArgType::RouteClearance],                  response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM81",  direction: MessageDirection::Uplink, template: "CLEARED [procedure name]",                         args: &[ArgType::ProcedureName],                   response_attr: ResponseAttribute::WU, fans: true,  atn_b1: false },
    MessageDefinition { id: "UM82",  direction: MessageDirection::Uplink, template: "CLEARED TO DEVIATE UP TO [distance] [direction] OF ROUTE", args: &[ArgType::Distance, ArgType::Direction], response_attr: ResponseAttribute::WU, fans: true, atn_b1: true },
    MessageDefinition { id: "UM92",  direction: MessageDirection::Uplink, template: "HOLD AT [position] AS PUBLISHED MAINTAIN [level]", args: &[ArgType::Position, ArgType::Level],        response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },

    // ── Uplink: Heading, Speed, Offset ────────────────────────────
    MessageDefinition { id: "UM64",  direction: MessageDirection::Uplink, template: "OFFSET [distance] [direction] OF ROUTE",           args: &[ArgType::Distance, ArgType::Direction],    response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM67",  direction: MessageDirection::Uplink, template: "PROCEED BACK ON ROUTE",                            args: &[],                                         response_attr: ResponseAttribute::WU, fans: true,  atn_b1: false },
    MessageDefinition { id: "UM94",  direction: MessageDirection::Uplink, template: "TURN [direction] HEADING [degrees]",               args: &[ArgType::Direction, ArgType::Degrees],     response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM96",  direction: MessageDirection::Uplink, template: "CONTINUE PRESENT HEADING",                         args: &[],                                         response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM190", direction: MessageDirection::Uplink, template: "FLY HEADING [degrees]",                             args: &[ArgType::Degrees],                         response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM215", direction: MessageDirection::Uplink, template: "TURN [direction] [degrees] DEGREES",                args: &[ArgType::Direction, ArgType::Degrees],     response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM106", direction: MessageDirection::Uplink, template: "MAINTAIN [speed]",                                  args: &[ArgType::Speed],                           response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM107", direction: MessageDirection::Uplink, template: "MAINTAIN PRESENT SPEED",                            args: &[],                                         response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM108", direction: MessageDirection::Uplink, template: "MAINTAIN [speed] OR GREATER",                       args: &[ArgType::Speed],                           response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM109", direction: MessageDirection::Uplink, template: "MAINTAIN [speed] OR LESS",                          args: &[ArgType::Speed],                           response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM116", direction: MessageDirection::Uplink, template: "RESUME NORMAL SPEED",                               args: &[],                                         response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },

    // ── Uplink: Contact, Surveillance ─────────────────────────────
    MessageDefinition { id: "UM117", direction: MessageDirection::Uplink, template: "CONTACT [unit name] [frequency]",                   args: &[ArgType::UnitName, ArgType::Frequency],    response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM120", direction: MessageDirection::Uplink, template: "MONITOR [unit name] [frequency]",                   args: &[ArgType::UnitName, ArgType::Frequency],    response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM123", direction: MessageDirection::Uplink, template: "SQUAWK [code]",                                     args: &[ArgType::Code],                            response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM179", direction: MessageDirection::Uplink, template: "SQUAWK IDENT",                                      args: &[],                                         response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM129", direction: MessageDirection::Uplink, template: "REPORT MAINTAINING [level]",                         args: &[ArgType::Level],                           response_attr: ResponseAttribute::R,  fans: true,  atn_b1: false },
    MessageDefinition { id: "UM128", direction: MessageDirection::Uplink, template: "REPORT LEAVING [level]",                             args: &[ArgType::Level],                           response_attr: ResponseAttribute::R,  fans: true,  atn_b1: false },
    MessageDefinition { id: "UM130", direction: MessageDirection::Uplink, template: "REPORT PASSING [position]",                          args: &[ArgType::Position],                        response_attr: ResponseAttribute::R,  fans: true,  atn_b1: false },
    MessageDefinition { id: "UM132", direction: MessageDirection::Uplink, template: "REPORT POSITION",                                    args: &[],                                         response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: false },
    MessageDefinition { id: "UM133", direction: MessageDirection::Uplink, template: "REPORT PRESENT LEVEL",                               args: &[],                                         response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM135", direction: MessageDirection::Uplink, template: "CONFIRM ASSIGNED LEVEL",                             args: &[],                                         response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM148", direction: MessageDirection::Uplink, template: "WHEN CAN YOU ACCEPT [level]",                        args: &[ArgType::Level],                           response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM149", direction: MessageDirection::Uplink, template: "CAN YOU ACCEPT [level] AT [position]",               args: &[ArgType::Level, ArgType::Position],        response_attr: ResponseAttribute::AN, fans: true,  atn_b1: false },

    // ── Uplink: Information ────────────────────────────────────────
    MessageDefinition { id: "UM153", direction: MessageDirection::Uplink, template: "ALTIMETER [altimeter]",                              args: &[ArgType::Altimeter],                       response_attr: ResponseAttribute::R,  fans: true,  atn_b1: false },
    MessageDefinition { id: "UM158", direction: MessageDirection::Uplink, template: "ATIS [atis code]",                                   args: &[ArgType::AtisCode],                        response_attr: ResponseAttribute::R,  fans: true,  atn_b1: false },
    MessageDefinition { id: "UM168", direction: MessageDirection::Uplink, template: "DISREGARD",                                          args: &[],                                         response_attr: ResponseAttribute::R,  fans: true,  atn_b1: false },
    MessageDefinition { id: "UM169", direction: MessageDirection::Uplink, template: "[free text]",                                        args: &[ArgType::FreeText],                        response_attr: ResponseAttribute::R,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM183", direction: MessageDirection::Uplink, template: "[free text]",                                        args: &[ArgType::FreeText],                        response_attr: ResponseAttribute::WU, fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM222", direction: MessageDirection::Uplink, template: "NO SPEED RESTRICTION",                               args: &[],                                         response_attr: ResponseAttribute::R,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "UM176", direction: MessageDirection::Uplink, template: "MAINTAIN OWN SEPARATION AND VMC",                    args: &[],                                         response_attr: ResponseAttribute::WU, fans: true,  atn_b1: false },

    // ── Downlink: Responses ────────────────────────────────────────
    MessageDefinition { id: "DM0",   direction: MessageDirection::Downlink, template: "WILCO",                                           args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM1",   direction: MessageDirection::Downlink, template: "UNABLE",                                          args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM2",   direction: MessageDirection::Downlink, template: "STANDBY",                                         args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM3",   direction: MessageDirection::Downlink, template: "ROGER",                                           args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM4",   direction: MessageDirection::Downlink, template: "AFFIRM",                                          args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM5",   direction: MessageDirection::Downlink, template: "NEGATIVE",                                        args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM62",  direction: MessageDirection::Downlink, template: "ERROR [error information]",                        args: &[ArgType::ErrorInfo],                       response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM63",  direction: MessageDirection::Downlink, template: "NOT CURRENT DATA AUTHORITY",                       args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM100", direction: MessageDirection::Downlink, template: "LOGICAL ACKNOWLEDGEMENT",                          args: &[],                                         response_attr: ResponseAttribute::N,  fans: false, atn_b1: true  },

    // ── Downlink: Pilot Requests ──────────────────────────────────
    MessageDefinition { id: "DM6",   direction: MessageDirection::Downlink, template: "REQUEST [level]",                                  args: &[ArgType::Level],                           response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM7",   direction: MessageDirection::Downlink, template: "REQUEST BLOCK [level] TO [level]",                 args: &[ArgType::Level, ArgType::Level],           response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: false },
    MessageDefinition { id: "DM9",   direction: MessageDirection::Downlink, template: "REQUEST CLIMB TO [level]",                         args: &[ArgType::Level],                           response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM10",  direction: MessageDirection::Downlink, template: "REQUEST DESCENT TO [level]",                       args: &[ArgType::Level],                           response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM18",  direction: MessageDirection::Downlink, template: "REQUEST [speed]",                                  args: &[ArgType::Speed],                           response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM22",  direction: MessageDirection::Downlink, template: "REQUEST DIRECT TO [position]",                     args: &[ArgType::Position],                        response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM25",  direction: MessageDirection::Downlink, template: "REQUEST CLEARANCE",                                args: &[],                                         response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: false },
    MessageDefinition { id: "DM27",  direction: MessageDirection::Downlink, template: "REQUEST WEATHER DEVIATION UP TO [distance] [direction] OF ROUTE", args: &[ArgType::Distance, ArgType::Direction], response_attr: ResponseAttribute::Y, fans: true, atn_b1: true },
    MessageDefinition { id: "DM70",  direction: MessageDirection::Downlink, template: "REQUEST HEADING [degrees]",                        args: &[ArgType::Degrees],                         response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: false },
    MessageDefinition { id: "DM15",  direction: MessageDirection::Downlink, template: "REQUEST OFFSET [distance] [direction] OF ROUTE",   args: &[ArgType::Distance, ArgType::Direction],    response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: false },
    MessageDefinition { id: "DM20",  direction: MessageDirection::Downlink, template: "REQUEST VOICE CONTACT",                            args: &[],                                         response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: false },

    // ── Downlink: Reports ─────────────────────────────────────────
    MessageDefinition { id: "DM28",  direction: MessageDirection::Downlink, template: "LEAVING [level]",                                  args: &[ArgType::Level],                           response_attr: ResponseAttribute::N,  fans: true,  atn_b1: false },
    MessageDefinition { id: "DM29",  direction: MessageDirection::Downlink, template: "CLIMBING TO [level]",                              args: &[ArgType::Level],                           response_attr: ResponseAttribute::N,  fans: true,  atn_b1: false },
    MessageDefinition { id: "DM30",  direction: MessageDirection::Downlink, template: "DESCENDING TO [level]",                            args: &[ArgType::Level],                           response_attr: ResponseAttribute::N,  fans: true,  atn_b1: false },
    MessageDefinition { id: "DM31",  direction: MessageDirection::Downlink, template: "PASSING [position]",                               args: &[ArgType::Position],                        response_attr: ResponseAttribute::N,  fans: true,  atn_b1: false },
    MessageDefinition { id: "DM32",  direction: MessageDirection::Downlink, template: "PRESENT LEVEL [level]",                            args: &[ArgType::Level],                           response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM33",  direction: MessageDirection::Downlink, template: "PRESENT POSITION [position]",                      args: &[ArgType::Position],                        response_attr: ResponseAttribute::N,  fans: true,  atn_b1: false },
    MessageDefinition { id: "DM34",  direction: MessageDirection::Downlink, template: "PRESENT SPEED [speed]",                            args: &[ArgType::Speed],                           response_attr: ResponseAttribute::N,  fans: true,  atn_b1: false },
    MessageDefinition { id: "DM37",  direction: MessageDirection::Downlink, template: "MAINTAINING [level]",                              args: &[ArgType::Level],                           response_attr: ResponseAttribute::N,  fans: true,  atn_b1: false },
    MessageDefinition { id: "DM38",  direction: MessageDirection::Downlink, template: "ASSIGNED LEVEL [level]",                           args: &[ArgType::Level],                           response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM41",  direction: MessageDirection::Downlink, template: "BACK ON ROUTE",                                    args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: false },
    MessageDefinition { id: "DM48",  direction: MessageDirection::Downlink, template: "POSITION REPORT [position report]",                args: &[ArgType::PositionReport],                  response_attr: ResponseAttribute::N,  fans: true,  atn_b1: false },
    MessageDefinition { id: "DM65",  direction: MessageDirection::Downlink, template: "DUE TO WEATHER",                                   args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM66",  direction: MessageDirection::Downlink, template: "DUE TO AIRCRAFT PERFORMANCE",                      args: &[],                                         response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM89",  direction: MessageDirection::Downlink, template: "MONITORING [unit name] [frequency]",               args: &[ArgType::UnitName, ArgType::Frequency],    response_attr: ResponseAttribute::N,  fans: true,  atn_b1: true  },

    // ── Downlink: Emergencies ─────────────────────────────────────
    MessageDefinition { id: "DM55",  direction: MessageDirection::Downlink, template: "PAN PAN PAN",                                      args: &[],                                         response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM56",  direction: MessageDirection::Downlink, template: "MAYDAY MAYDAY MAYDAY",                             args: &[],                                         response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM58",  direction: MessageDirection::Downlink, template: "CANCEL EMERGENCY",                                 args: &[],                                         response_attr: ResponseAttribute::Y,  fans: true,  atn_b1: true  },
    MessageDefinition { id: "DM67",  direction: MessageDirection::Downlink, template: "[free text]",                                      args: &[ArgType::FreeText],                        response_attr: ResponseAttribute::R,  fans: true,  atn_b1: true  },
];

// ---------------------------------------------------------------------------
// MessageElement / CpdlcApplicationMessage
// ---------------------------------------------------------------------------

/// One element of a CPDLC application message.
///
/// A single CPDLC message can consist of up to 5 elements (multi-element).
/// Each element references a [`MessageDefinition`] by ID and carries the
/// concrete argument values.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MessageElement {
    /// Definition ID — e.g. `"UM20"`, `"DM0"`.  Must match a registry entry.
    pub id: String,
    /// Concrete argument values matching the definition's `args` spec.
    pub args: Vec<CpdlcArgument>,
}

impl MessageElement {
    /// Create a new element with the given definition ID and arguments.
    pub fn new(id: impl Into<String>, args: Vec<CpdlcArgument>) -> Self {
        Self {
            id: id.into(),
            args,
        }
    }

    /// Look up the [`MessageDefinition`] for this element's ID.
    pub fn definition(&self) -> Option<&'static MessageDefinition> {
        find_definition(&self.id)
    }

    /// Render this element to human-readable text by substituting the
    /// template placeholders with concrete argument values.
    pub fn render(&self) -> String {
        match self.definition() {
            Some(def) => def.render(&self.args),
            None => format!("[UNKNOWN {}]", self.id),
        }
    }
}

/// An operational (application-level) CPDLC message.
///
/// Replaces the former hard-coded `CpdlcMessage` enum with a data-driven
/// structure that can represent any ICAO CPDLC message via the
/// [`MESSAGE_REGISTRY`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CpdlcApplicationMessage {
    /// Message Identification Number (0–63), assigned by the sender.
    pub min: u8,
    /// Message Reference Number — the MIN of the message being replied to.
    /// `None` for an initiating (new-dialogue) message.
    pub mrn: Option<u8>,
    /// One to five message elements.
    pub elements: Vec<MessageElement>,
    /// When this message was created.
    pub timestamp: DateTime<Utc>,
}

impl CpdlcApplicationMessage {
    /// Compute the effective [`ResponseAttribute`] for a multi-element
    /// message using ICAO precedence rules.
    pub fn effective_response_attr(&self) -> ResponseAttribute {
        let attrs: Vec<ResponseAttribute> = self
            .elements
            .iter()
            .filter_map(|e| e.definition())
            .map(|d| d.response_attr)
            .collect();
        ResponseAttribute::effective(&attrs)
    }

    /// Render all elements to a single human-readable string,
    /// separated by ` / ` for multi-element messages.
    pub fn render(&self) -> String {
        self.elements
            .iter()
            .map(|e| e.render())
            .collect::<Vec<_>>()
            .join(" / ")
    }

    /// Return `true` if this message is a closing response (WILCO, UNABLE,
    /// ROGER, AFFIRM, NEGATIVE) that would close an open dialogue.
    pub fn is_closing_response(&self) -> bool {
        if self.elements.len() != 1 {
            return false;
        }
        matches!(
            self.elements[0].id.as_str(),
            "DM0" | "DM1" | "DM3" | "DM4" | "DM5" | "UM0" | "UM3" | "UM4" | "UM5"
        )
    }

    /// Return `true` if this message is a STANDBY (DM2, UM1, UM2) that
    /// does **not** close an open dialogue.
    pub fn is_standby(&self) -> bool {
        if self.elements.len() != 1 {
            return false;
        }
        matches!(
            self.elements[0].id.as_str(),
            "DM2" | "UM1" | "UM2"
        )
    }
}

impl From<CpdlcApplicationMessage> for SerializedMessagePayload {
    fn from(value: CpdlcApplicationMessage) -> Self {
        SerializedMessagePayload(value.render())
    }
}

// ---------------------------------------------------------------------------
// DialogueState / CpdlcDialogue
// ---------------------------------------------------------------------------

/// Whether a CPDLC dialogue is still awaiting a closing response.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialogueState {
    /// A response is still expected.
    Open,
    /// The dialogue has been closed by a final response.
    Closed,
}

/// Tracks a single MIN↔MRN dialogue within a CPDLC connection.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CpdlcDialogue {
    /// The MIN of the message that opened this dialogue.
    pub initiator_min: u8,
    /// Who sent the opening message.
    pub initiator: AcarsEndpointCallsign,
    /// Current state.
    pub state: DialogueState,
    /// Effective response attribute for this dialogue.
    pub response_attr: ResponseAttribute,
}

// ---------------------------------------------------------------------------
// CpdlcSessionView (server → clients session state broadcast)
// ---------------------------------------------------------------------------

/// Phase of an individual CPDLC connection.
///
/// Represents the lifecycle of a connection between an aircraft and a ground
/// station, from initial logon through to termination.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CpdlcConnectionPhase {
    /// Logon requested, awaiting response.
    LogonPending,
    /// Logon accepted, CPDLC connection not yet established.
    LoggedOn,
    /// CPDLC connection active — operational message exchange is possible.
    Connected,
    /// Connection terminated.
    Terminated,
}

impl fmt::Display for CpdlcConnectionPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CpdlcConnectionPhase::LogonPending => write!(f, "LOGON PENDING"),
            CpdlcConnectionPhase::LoggedOn => write!(f, "LOGGED ON"),
            CpdlcConnectionPhase::Connected => write!(f, "CONNECTED"),
            CpdlcConnectionPhase::Terminated => write!(f, "TERMINATED"),
        }
    }
}

/// View of one CPDLC connection as seen by a participant.
///
/// The `peer` field identifies the other party: for an aircraft it is the
/// ground station callsign; for a ground station it is the aircraft callsign.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CpdlcConnectionView {
    /// The callsign of the peer endpoint.
    pub peer: AcarsEndpointCallsign,
    /// Current phase of this connection.
    pub phase: CpdlcConnectionPhase,
}

/// Server-authoritative view of a CPDLC session for a given participant.
///
/// Broadcast by the server after every session-mutating meta-message so that
/// both sides (aircraft and ground station) can project the session state
/// without duplicating the state machine logic.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CpdlcSessionView {
    /// The active connection (CDA side for the aircraft).
    pub active_connection: Option<CpdlcConnectionView>,
    /// The inactive connection (NDA side for the aircraft).
    pub inactive_connection: Option<CpdlcConnectionView>,
    /// Next Data Authority, if designated.
    pub next_data_authority: Option<AcarsEndpointCallsign>,
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
    Application(CpdlcApplicationMessage),
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

    /// Ground station ends service with the aircraft, terminating the
    /// active connection and promoting the inactive one (if any).
    EndService,

    /// Server → client notification: session state after processing a meta-message.
    ///
    /// Sent by the server to both parties (aircraft + ground station) after
    /// every session-mutating event. Clients should replace their local
    /// session state with this snapshot.
    SessionUpdate {
        /// The authoritative session state for the recipient.
        session: CpdlcSessionView,
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
            CpdlcMetaMessage::EndService => "END SERVICE".to_string(),
            CpdlcMetaMessage::SessionUpdate { ref session } => {
                let active = session
                    .active_connection
                    .as_ref()
                    .map(|c| format!("{} ({})", c.peer, c.phase))
                    .unwrap_or_else(|| "NONE".to_string());
                let inactive = session
                    .inactive_connection
                    .as_ref()
                    .map(|c| format!("{} ({})", c.peer, c.phase))
                    .unwrap_or_else(|| "NONE".to_string());
                format!("SESSION UPDATE ACTIVE {} INACTIVE {}", active, inactive)
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
/// Produced by converting a [`CpdlcApplicationMessage`], [`CpdlcMetaMessage`],
/// or [`CpdlcMessageType`] via the `From` / `Into` trait.
///
/// # Examples
///
/// ```
/// use openlink_models::{CpdlcApplicationMessage, MessageElement, CpdlcArgument, FlightLevel, SerializedMessagePayload};
///
/// let msg = CpdlcApplicationMessage {
///     min: 1,
///     mrn: None,
///     elements: vec![MessageElement::new("UM20", vec![CpdlcArgument::Level(FlightLevel::new(350))])],
///     timestamp: chrono::Utc::now(),
/// };
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

    // -- MessageDefinition / Registry ---------------------------------------

    #[test]
    fn find_definition_um20() {
        let def = find_definition("UM20").expect("UM20 should exist");
        assert_eq!(def.direction, MessageDirection::Uplink);
        assert_eq!(def.response_attr, ResponseAttribute::WU);
        assert_eq!(def.args, &[ArgType::Level]);
    }

    #[test]
    fn find_definition_dm0() {
        let def = find_definition("DM0").expect("DM0 should exist");
        assert_eq!(def.direction, MessageDirection::Downlink);
        assert_eq!(def.template, "WILCO");
        assert_eq!(def.response_attr, ResponseAttribute::N);
    }

    #[test]
    fn find_definition_unknown() {
        assert!(find_definition("XY999").is_none());
    }

    #[test]
    fn definition_render_no_args() {
        let def = find_definition("DM0").unwrap();
        assert_eq!(def.render(&[]), "WILCO");
    }

    #[test]
    fn definition_render_with_level() {
        let def = find_definition("UM20").unwrap();
        let text = def.render(&[CpdlcArgument::Level(FlightLevel::new(350))]);
        assert_eq!(text, "CLIMB TO FL350");
    }

    #[test]
    fn definition_render_multi_args() {
        let def = find_definition("UM46").unwrap();
        let text = def.render(&[
            CpdlcArgument::Position("REKLA".to_string()),
            CpdlcArgument::Level(FlightLevel::new(350)),
        ]);
        assert_eq!(text, "CROSS REKLA AT FL350");
    }

    // -- ResponseAttribute precedence --------------------------------------

    #[test]
    fn response_attr_effective_wu_wins() {
        let effective = ResponseAttribute::effective(&[
            ResponseAttribute::Y,
            ResponseAttribute::WU,
            ResponseAttribute::R,
        ]);
        assert_eq!(effective, ResponseAttribute::WU);
    }

    #[test]
    fn response_attr_effective_ne_treated_as_n() {
        let effective = ResponseAttribute::effective(&[
            ResponseAttribute::NE,
            ResponseAttribute::R,
        ]);
        assert_eq!(effective, ResponseAttribute::R);
    }

    #[test]
    fn response_attr_effective_empty_is_n() {
        assert_eq!(ResponseAttribute::effective(&[]), ResponseAttribute::N);
    }

    // -- CpdlcApplicationMessage -------------------------------------------

    #[test]
    fn application_message_render_single() {
        let msg = CpdlcApplicationMessage {
            min: 1,
            mrn: None,
            elements: vec![MessageElement::new(
                "UM20",
                vec![CpdlcArgument::Level(FlightLevel::new(350))],
            )],
            timestamp: Utc::now(),
        };
        assert_eq!(msg.render(), "CLIMB TO FL350");
    }

    #[test]
    fn application_message_render_multi() {
        let msg = CpdlcApplicationMessage {
            min: 2,
            mrn: None,
            elements: vec![
                MessageElement::new(
                    "UM20",
                    vec![CpdlcArgument::Level(FlightLevel::new(350))],
                ),
                MessageElement::new(
                    "UM129",
                    vec![CpdlcArgument::Level(FlightLevel::new(350))],
                ),
            ],
            timestamp: Utc::now(),
        };
        assert_eq!(msg.render(), "CLIMB TO FL350 / REPORT MAINTAINING FL350");
    }

    #[test]
    fn application_message_effective_attr_multi() {
        let msg = CpdlcApplicationMessage {
            min: 3,
            mrn: None,
            elements: vec![
                MessageElement::new("UM20", vec![CpdlcArgument::Level(FlightLevel::new(350))]),
                MessageElement::new("UM129", vec![CpdlcArgument::Level(FlightLevel::new(350))]),
            ],
            timestamp: Utc::now(),
        };
        // UM20 = W/U, UM129 = R  →  effective = W/U
        assert_eq!(msg.effective_response_attr(), ResponseAttribute::WU);
    }

    #[test]
    fn application_message_is_closing() {
        let wilco = CpdlcApplicationMessage {
            min: 4, mrn: Some(1),
            elements: vec![MessageElement::new("DM0", vec![])],
            timestamp: Utc::now(),
        };
        assert!(wilco.is_closing_response());

        let standby = CpdlcApplicationMessage {
            min: 5, mrn: Some(1),
            elements: vec![MessageElement::new("DM2", vec![])],
            timestamp: Utc::now(),
        };
        assert!(!standby.is_closing_response());
        assert!(standby.is_standby());
    }

    // -- SerializedMessagePayload ------------------------------------------

    #[test]
    fn serialized_payload_from_application_message() {
        let msg = CpdlcApplicationMessage {
            min: 1,
            mrn: None,
            elements: vec![MessageElement::new(
                "UM20",
                vec![CpdlcArgument::Level(FlightLevel::new(350))],
            )],
            timestamp: Utc::now(),
        };
        let payload: SerializedMessagePayload = msg.into();
        assert_eq!(payload.to_string(), "CLIMB TO FL350");
    }

    #[test]
    fn serialized_payload_from_downlink_request() {
        let msg = CpdlcApplicationMessage {
            min: 8,
            mrn: None,
            elements: vec![MessageElement::new(
                "DM9",
                vec![CpdlcArgument::Level(FlightLevel::new(390))],
            )],
            timestamp: Utc::now(),
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
        let mt = CpdlcMessageType::Application(CpdlcApplicationMessage {
            min: 1,
            mrn: None,
            elements: vec![MessageElement::new(
                "UM20",
                vec![CpdlcArgument::Level(FlightLevel::new(350))],
            )],
            timestamp: Utc::now(),
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
            message: CpdlcMessageType::Application(CpdlcApplicationMessage {
                min: 1,
                mrn: None,
                elements: vec![MessageElement::new(
                    "UM20",
                    vec![CpdlcArgument::Level(FlightLevel::new(350))],
                )],
                timestamp: Utc::now(),
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
