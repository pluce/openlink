//! Authentication credentials returned by the OpenLink auth service.

/// Credentials obtained after a successful OAuth / authorization-code exchange.
///
/// These are used to authenticate the NATS connection via NKey challenge.
///
/// * `seed`  – NKey seed (private key) used to sign the server challenge.
/// * `jwt`   – User JWT that authorises the connection with specific permissions.
/// * `cid`   – Unique connection identifier assigned by the auth service.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenLinkCredentials {
    /// NKey seed for NATS authentication.
    pub seed: String,
    /// User JWT that encodes NATS permissions.
    pub jwt: String,
    /// Connection ID.
    pub cid: String,
}
