//! Canonical NATS subject definitions for the OpenLink protocol.
//!
//! All NATS subject strings used by the OpenLink network **must** be built
//! through [`NatsSubjects`].  This ensures that clients, servers, and tooling
//! agree on a single naming convention and makes future versioning explicit.
//!
//! # Subject layout
//!
//! ```text
//! openlink.v1.{network}.outbox.{address}   ← clients PUBLISH here
//! openlink.v1.{network}.inbox.{address}    ← clients SUBSCRIBE here
//! openlink.v1.{network}.outbox.>           ← server wildcard (receives all client messages)
//! openlink.v1.{network}.inbox.>            ← server wildcard (all inboxes)
//! ```
//!
//! # KV bucket names
//!
//! ```text
//! openlink-v1-{network}-cpdlc-sessions     ← CPDLC session store
//! openlink-v1-{network}-station-registry    ← station registry store
//! ```

use openlink_models::{NetworkAddress, NetworkId};

/// Current subject version prefix.
const VERSION: &str = "v1";

/// Central authority for all NATS subject and bucket names.
///
/// Every subject produced by OpenLink flows through this struct so that
/// the naming convention is defined in exactly one place.
///
/// # Examples
///
/// ```
/// use openlink_models::{NetworkId, NetworkAddress};
/// use openlink_sdk::NatsSubjects;
///
/// let network = NetworkId::new("vatsim");
/// let station = NetworkAddress::new("LFPG");
///
/// assert_eq!(
///     NatsSubjects::outbox(&network, &station),
///     "openlink.v1.vatsim.outbox.LFPG",
/// );
/// assert_eq!(
///     NatsSubjects::inbox(&network, &station),
///     "openlink.v1.vatsim.inbox.LFPG",
/// );
/// assert_eq!(
///     NatsSubjects::outbox_wildcard(&network),
///     "openlink.v1.vatsim.outbox.>",
/// );
/// ```
pub struct NatsSubjects;

impl NatsSubjects {
    // ------------------------------------------------------------------
    // Messaging subjects
    // ------------------------------------------------------------------

    /// Subject a client publishes to when sending a message.
    ///
    /// The server subscribes to [`Self::outbox_wildcard`] to receive all
    /// outbound messages from every client on the network.
    pub fn outbox(network: &NetworkId, address: &NetworkAddress) -> String {
        format!("openlink.{VERSION}.{network}.outbox.{address}")
    }

    /// Subject a client subscribes to in order to receive messages.
    ///
    /// The server publishes to `inbox.<address>` when routing a message
    /// to a specific station or aircraft.
    pub fn inbox(network: &NetworkId, address: &NetworkAddress) -> String {
        format!("openlink.{VERSION}.{network}.inbox.{address}")
    }

    /// Wildcard subject that matches **all** outbox messages on a network.
    ///
    /// Intended for the OpenLink server to receive every client message.
    pub fn outbox_wildcard(network: &NetworkId) -> String {
        format!("openlink.{VERSION}.{network}.outbox.>")
    }

    /// Wildcard subject that matches **all** inbox messages on a network.
    ///
    /// Useful for monitoring or debugging.
    pub fn inbox_wildcard(network: &NetworkId) -> String {
        format!("openlink.{VERSION}.{network}.inbox.>")
    }

    // ------------------------------------------------------------------
    // JetStream KV bucket names
    // ------------------------------------------------------------------

    /// KV bucket name for storing CPDLC session state.
    pub fn kv_cpdlc_sessions(network: &NetworkId) -> String {
        format!("openlink-{VERSION}-{network}-cpdlc-sessions")
    }

    /// KV bucket name for the station registry.
    pub fn kv_station_registry(network: &NetworkId) -> String {
        format!("openlink-{VERSION}-{network}-station-registry")
    }

    // ------------------------------------------------------------------
    // Parsing helpers
    // ------------------------------------------------------------------

    /// Extract the sender address from an outbox subject.
    ///
    /// Given `"openlink.v1.vatsim.outbox.LFPG"` returns `Some("LFPG")`.
    /// Returns `None` if the subject does not match the expected pattern.
    pub fn parse_outbox_sender(subject: &str) -> Option<&str> {
        let parts: Vec<&str> = subject.splitn(5, '.').collect();
        if parts.len() == 5 && parts[0] == "openlink" && parts[3] == "outbox" {
            Some(parts[4])
        } else {
            None
        }
    }

    /// Extract the recipient address from an inbox subject.
    ///
    /// Given `"openlink.v1.vatsim.inbox.AFR123"` returns `Some("AFR123")`.
    pub fn parse_inbox_recipient(subject: &str) -> Option<&str> {
        let parts: Vec<&str> = subject.splitn(5, '.').collect();
        if parts.len() == 5 && parts[0] == "openlink" && parts[3] == "inbox" {
            Some(parts[4])
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn net() -> NetworkId {
        NetworkId::new("vatsim")
    }

    fn addr(s: &str) -> NetworkAddress {
        NetworkAddress::new(s)
    }

    // -- messaging subjects -------------------------------------------------

    #[test]
    fn outbox_subject() {
        assert_eq!(
            NatsSubjects::outbox(&net(), &addr("AFR123")),
            "openlink.v1.vatsim.outbox.AFR123",
        );
    }

    #[test]
    fn inbox_subject() {
        assert_eq!(
            NatsSubjects::inbox(&net(), &addr("LFPG")),
            "openlink.v1.vatsim.inbox.LFPG",
        );
    }

    #[test]
    fn outbox_wildcard_subject() {
        assert_eq!(
            NatsSubjects::outbox_wildcard(&net()),
            "openlink.v1.vatsim.outbox.>",
        );
    }

    #[test]
    fn inbox_wildcard_subject() {
        assert_eq!(
            NatsSubjects::inbox_wildcard(&net()),
            "openlink.v1.vatsim.inbox.>",
        );
    }

    // -- KV bucket names ----------------------------------------------------

    #[test]
    fn kv_cpdlc_sessions_bucket() {
        assert_eq!(
            NatsSubjects::kv_cpdlc_sessions(&net()),
            "openlink-v1-vatsim-cpdlc-sessions",
        );
    }

    #[test]
    fn kv_station_registry_bucket() {
        assert_eq!(
            NatsSubjects::kv_station_registry(&net()),
            "openlink-v1-vatsim-station-registry",
        );
    }

    // -- parsing helpers ----------------------------------------------------

    #[test]
    fn parse_outbox_sender_valid() {
        assert_eq!(
            NatsSubjects::parse_outbox_sender("openlink.v1.vatsim.outbox.AFR123"),
            Some("AFR123"),
        );
    }

    #[test]
    fn parse_outbox_sender_invalid() {
        assert_eq!(NatsSubjects::parse_outbox_sender("bad.subject"), None);
        assert_eq!(
            NatsSubjects::parse_outbox_sender("openlink.v1.vatsim.inbox.AFR123"),
            None,
        );
    }

    #[test]
    fn parse_inbox_recipient_valid() {
        assert_eq!(
            NatsSubjects::parse_inbox_recipient("openlink.v1.icao.inbox.LFPG"),
            Some("LFPG"),
        );
    }

    #[test]
    fn parse_inbox_recipient_invalid() {
        assert_eq!(NatsSubjects::parse_inbox_recipient("totally.wrong"), None);
    }

    // -- different networks -------------------------------------------------

    #[test]
    fn subjects_vary_by_network() {
        let icao = NetworkId::new("icao");
        let a = addr("STATION1");
        assert_eq!(
            NatsSubjects::outbox(&icao, &a),
            "openlink.v1.icao.outbox.STATION1",
        );
    }
}
