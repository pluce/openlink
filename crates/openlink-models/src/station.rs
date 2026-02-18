//! Station / ATC meta-message types.
//!
//! These types represent system-level messages that are not part of the ACARS
//! or CPDLC protocols but are used by the OpenLink infrastructure to track
//! ground-station availability.

use std::convert::Infallible;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::acars::AcarsRoutingEndpoint;

// ---------------------------------------------------------------------------
// StationId
// ---------------------------------------------------------------------------

/// Unique identifier for a ground station within the OpenLink network.
///
/// # Examples
///
/// ```
/// use openlink_models::StationId;
///
/// let id = StationId::new("LFPG-APP");
/// assert_eq!(id.to_string(), "LFPG-APP");
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct StationId(String);

impl StationId {
    /// Create a new station identifier.
    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }

    /// Return the inner string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for StationId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for StationId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl FromStr for StationId {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

// ---------------------------------------------------------------------------
// StationStatus
// ---------------------------------------------------------------------------

/// The availability status of a ground station.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString, strum::EnumIter)]
#[strum(serialize_all = "lowercase")]
pub enum StationStatus {
    /// The station is online and ready to accept connections.
    Online,
    /// The station is offline.
    Offline,
}

// ---------------------------------------------------------------------------
// MetaMessage
// ---------------------------------------------------------------------------

/// System-level messages exchanged on the OpenLink network.
///
/// Currently only station-status updates are defined.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum MetaMessage {
    /// A station announces or updates its status.
    StationStatus(StationId, StationStatus, AcarsRoutingEndpoint),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acars::AcarsRoutingEndpoint;

    #[test]
    fn station_id_display_and_equality() {
        let a = StationId::new("LFPG-APP");
        let b: StationId = "LFPG-APP".into();
        assert_eq!(a, b);
        assert_eq!(a.to_string(), "LFPG-APP");
        assert_eq!(a.as_str(), "LFPG-APP");
    }

    #[test]
    fn station_status_display() {
        assert_eq!(StationStatus::Online.to_string(), "online");
        assert_eq!(StationStatus::Offline.to_string(), "offline");
    }

    #[test]
    fn station_status_copy() {
        let s = StationStatus::Online;
        let s2 = s; // Copy
        assert_eq!(s, s2);
    }

    #[test]
    fn meta_message_serde_roundtrip() {
        let msg = MetaMessage::StationStatus(
            StationId::new("LFPG-APP"),
            StationStatus::Online,
            AcarsRoutingEndpoint::new("LFPG", "ADDR001"),
        );
        let json = serde_json::to_string(&msg).unwrap();
        let back: MetaMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn station_id_hash_usable() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(StationId::new("A"));
        set.insert(StationId::new("B"));
        set.insert(StationId::new("A"));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn station_status_from_str() {
        use std::str::FromStr;
        assert_eq!(StationStatus::from_str("online").unwrap(), StationStatus::Online);
        assert_eq!(StationStatus::from_str("offline").unwrap(), StationStatus::Offline);
        assert!(StationStatus::from_str("unknown").is_err());
    }

    #[test]
    fn station_status_enum_iter() {
        use strum::IntoEnumIterator;
        let variants: Vec<_> = StationStatus::iter().collect();
        assert_eq!(variants, vec![StationStatus::Online, StationStatus::Offline]);
    }

    #[test]
    fn station_id_from_str() {
        let id: StationId = "LFPG-APP".parse().unwrap();
        assert_eq!(id.as_str(), "LFPG-APP");
    }
}
