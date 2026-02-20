//! Network-level addressing and routing types.
//!
//! These types represent the OpenLink network layer, which sits above the
//! transport (NATS) and below the ACARS application layer. Every message
//! exchanged on the network carries an [`OpenLinkRouting`] header that
//! identifies the source and destination endpoints.
//!
//! A [`NetworkId`] identifies a network (e.g. "demonetwork", "icao") on which
//! stations are registered. Each station within a network is identified by
//! its [`NetworkAddress`].

use std::convert::Infallible;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// NetworkId
// ---------------------------------------------------------------------------

/// Identifier for a network in the OpenLink ecosystem.
///
/// A `NetworkId` names the network itself (e.g. `"demonetwork"`, `"icao"`).
/// Stations registered on that network are then identified by their
/// [`NetworkAddress`].
///
/// # Examples
///
/// ```
/// use openlink_models::NetworkId;
///
/// let id = NetworkId::new("demonetwork");
/// assert_eq!(id.to_string(), "demonetwork");
///
/// let id2: NetworkId = "demonetwork".into();
/// assert_eq!(id, id2);
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetworkId(String);

impl NetworkId {
    /// Create a new `NetworkId` from a string slice.
    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }

    /// Return the inner string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NetworkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for NetworkId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for NetworkId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl FromStr for NetworkId {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

// ---------------------------------------------------------------------------
// NetworkAddress
// ---------------------------------------------------------------------------

/// Address of a station within a network.
///
/// While [`NetworkId`] identifies *which network* is being used,
/// `NetworkAddress` identifies a specific station registered on that
/// network (e.g. a ground station, an aircraft gateway).
///
/// # Examples
///
/// ```
/// use openlink_models::NetworkAddress;
///
/// let addr: NetworkAddress = "LFPG".into();
/// assert_eq!(addr.to_string(), "LFPG");
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetworkAddress(String);

impl NetworkAddress {
    /// Create a new `NetworkAddress` from a string slice.
    pub fn new(addr: &str) -> Self {
        Self(addr.to_string())
    }

    /// Return the inner string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NetworkAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for NetworkAddress {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for NetworkAddress {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl FromStr for NetworkAddress {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

// ---------------------------------------------------------------------------
// OpenLinkRoutingEndpoint
// ---------------------------------------------------------------------------

/// One end of an OpenLink routing path.
///
/// An endpoint can be:
/// - [`Server`](Self::Server) — targets the network itself (identified by
///   its [`NetworkId`]); the server decides how to route further.
/// - [`Address`](Self::Address) — targets a specific station on a network,
///   combining a [`NetworkId`] with a [`NetworkAddress`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum OpenLinkRoutingEndpoint {
    /// A network-level endpoint (routing delegated to the server).
    Server(NetworkId),
    /// A station on a specific network (network + station address).
    Address(NetworkId, NetworkAddress),
}

// ---------------------------------------------------------------------------
// OpenLinkRouting
// ---------------------------------------------------------------------------

/// Source → Destination routing header attached to every [`super::OpenLinkEnvelope`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OpenLinkRouting {
    /// The originator of the message.
    pub source: OpenLinkRoutingEndpoint,
    /// The intended recipient of the message.
    pub destination: OpenLinkRoutingEndpoint,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_id_display_and_equality() {
        let a = NetworkId::new("atc-paris");
        let b: NetworkId = "atc-paris".into();
        assert_eq!(a, b);
        assert_eq!(a.to_string(), "atc-paris");
        assert_eq!(a.as_str(), "atc-paris");
    }

    #[test]
    fn network_id_from_owned_string() {
        let id = NetworkId::from(String::from("owned"));
        assert_eq!(id.as_str(), "owned");
    }

    #[test]
    fn network_address_display_and_from() {
        let addr = NetworkAddress::new("acars.uplink.LFPG");
        let addr2: NetworkAddress = "acars.uplink.LFPG".into();
        assert_eq!(addr, addr2);
        assert_eq!(addr.to_string(), "acars.uplink.LFPG");
    }

    #[test]
    fn routing_endpoint_variants() {
        let server = OpenLinkRoutingEndpoint::Server(NetworkId::new("srv"));
        let addressed = OpenLinkRoutingEndpoint::Address(
            NetworkId::new("srv"),
            NetworkAddress::new("some.subject"),
        );
        // Ensure they are distinct variants
        assert_ne!(server, addressed);
    }

    #[test]
    fn routing_roundtrip_serde() {
        let routing = OpenLinkRouting {
            source: OpenLinkRoutingEndpoint::Server(NetworkId::new("server-1")),
            destination: OpenLinkRoutingEndpoint::Address(
                NetworkId::new("gw-2"),
                NetworkAddress::new("acars.down"),
            ),
        };
        let json = serde_json::to_string(&routing).unwrap();
        let back: OpenLinkRouting = serde_json::from_str(&json).unwrap();
        assert_eq!(routing, back);
    }

    #[test]
    fn network_id_hash_usable_in_collections() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(NetworkId::new("a"));
        set.insert(NetworkId::new("b"));
        set.insert(NetworkId::new("a"));
        assert_eq!(set.len(), 2);
    }
}
