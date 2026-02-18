//! NATS JWT generation.
//!
//! Signs a NATS user JWT that encodes the CID, station NKey public key,
//! and scoped publish/subscribe permissions derived from [`NatsSubjects`].

use std::time::SystemTime;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use nkeys::KeyPair;
use openlink_models::{NetworkAddress, NetworkId};
use openlink_sdk::NatsSubjects;
use serde::Serialize;

use crate::error::AuthError;

// ---------------------------------------------------------------------------
// NATS JWT claim types
// ---------------------------------------------------------------------------

/// Top-level NATS JWT claims (header `alg: ed25519-nkey`).
#[derive(Serialize)]
struct NatsUserClaims {
    jti: String,
    iat: u64,
    exp: u64,
    iss: String,
    name: String,
    sub: String,
    nats: NatsClaims,
}

/// The `nats` object embedded in the JWT body.
#[derive(Serialize)]
struct NatsClaims {
    permissions: NatsPermissions,
    #[serde(rename = "type")]
    claim_type: String,
    version: i32,
}

/// Publish / subscribe permission lists.
#[derive(Serialize)]
struct NatsPermissions {
    publish: NatsPermissionList,
    subscribe: NatsPermissionList,
}

/// A list of allowed subjects.
#[derive(Serialize)]
struct NatsPermissionList {
    allow: Vec<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Sign a NATS user JWT for the given CID on a specific network.
///
/// The JWT grants the user:
/// - **publish** on their outbox subject
/// - **subscribe** on their inbox subject
///
/// # Arguments
///
/// * `account_kp` — The NATS account key-pair used to sign the JWT.
/// * `user_nkey_public` — Client-provided NKey public key (JWT `sub`).
/// * `cid` — The authenticated CID (becomes the JWT `name` and is used
///   for subject scoping).
/// * `network` — The network this JWT authorises access to.
/// * `ttl_secs` — Lifetime of the token in seconds.
pub fn sign_user_jwt(
    account_kp: &KeyPair,
    user_nkey_public: &str,
    cid: &str,
    network: &NetworkId,
    ttl_secs: u64,
) -> Result<String, AuthError> {
    let address = NetworkAddress::new(cid);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs();

    let claims = NatsUserClaims {
        jti: uuid::Uuid::new_v4().to_string(),
        iat: now,
        exp: now + ttl_secs,
        iss: account_kp.public_key(),
        name: cid.to_string(),
        sub: user_nkey_public.to_string(),
        nats: NatsClaims {
            claim_type: "user".to_string(),
            version: 2,
            permissions: NatsPermissions {
                publish: NatsPermissionList {
                    allow: vec![NatsSubjects::outbox(network, &address)],
                },
                subscribe: NatsPermissionList {
                    allow: vec![NatsSubjects::inbox(network, &address)],
                },
            },
        },
    };

    encode_and_sign(account_kp, &claims)
}

/// Sign a NATS JWT granting **server-level** permissions on a network.
///
/// The server JWT can:
/// - **subscribe** to all outbox messages (`outbox.>`)
/// - **publish** to any station inbox (`inbox.>`)
/// - **access** JetStream KV buckets (`$JS.API.>`, `_INBOX.>`)
///
/// # Arguments
///
/// * `account_kp` — The NATS account key-pair used to sign the JWT.
/// * `user_nkey_public` — Server-generated NKey public key.
/// * `network` — The network this server JWT authorises.
/// * `ttl_secs` — Lifetime of the token in seconds.
pub fn sign_server_jwt(
    account_kp: &KeyPair,
    user_nkey_public: &str,
    network: &NetworkId,
    ttl_secs: u64,
) -> Result<String, AuthError> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs();

    let claims = NatsUserClaims {
        jti: uuid::Uuid::new_v4().to_string(),
        iat: now,
        exp: now + ttl_secs,
        iss: account_kp.public_key(),
        name: format!("openlink-server-{network}"),
        sub: user_nkey_public.to_string(),
        nats: NatsClaims {
            claim_type: "user".to_string(),
            version: 2,
            permissions: NatsPermissions {
                publish: NatsPermissionList {
                    allow: vec![
                        NatsSubjects::inbox_wildcard(network),
                        "$JS.API.>".to_string(),
                        "_INBOX.>".to_string(),
                    ],
                },
                subscribe: NatsPermissionList {
                    allow: vec![
                        NatsSubjects::outbox_wildcard(network),
                        "$JS.API.>".to_string(),
                        "_INBOX.>".to_string(),
                    ],
                },
            },
        },
    };

    encode_and_sign(account_kp, &claims)
}

// ---------------------------------------------------------------------------
// Encoding helpers
// ---------------------------------------------------------------------------

/// Encode claims as a NATS JWT: `base64url(header).base64url(body).base64url(sig)`.
fn encode_and_sign(kp: &KeyPair, claims: &NatsUserClaims) -> Result<String, AuthError> {
    let header = serde_json::json!({
        "typ": "JWT",
        "alg": "ed25519-nkey"
    });

    let encoded_header = URL_SAFE_NO_PAD.encode(serde_json::to_string(&header)?);
    let encoded_body = URL_SAFE_NO_PAD.encode(serde_json::to_string(claims)?);
    let signing_input = format!("{encoded_header}.{encoded_body}");

    let sig = kp
        .sign(signing_input.as_bytes())
        .map_err(|e| AuthError::NKeyError(e.to_string()))?;
    let encoded_sig = URL_SAFE_NO_PAD.encode(sig);

    Ok(format!("{signing_input}.{encoded_sig}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_account_kp() -> KeyPair {
        KeyPair::new_account()
    }

    #[test]
    fn jwt_has_three_parts() {
        let kp = test_account_kp();
        let jwt = sign_user_jwt(&kp, "UABC123", "42", &NetworkId::new("vatsim"), 3600).unwrap();
        assert_eq!(jwt.split('.').count(), 3);
    }

    #[test]
    fn jwt_body_contains_correct_permissions() {
        let kp = test_account_kp();
        let net = NetworkId::new("vatsim");
        let jwt = sign_user_jwt(&kp, "UABC123", "42", &net, 3600).unwrap();

        // Decode the body (second part)
        let body_b64 = jwt.split('.').nth(1).unwrap();
        let body_bytes = URL_SAFE_NO_PAD.decode(body_b64).unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        let publish_allow = &body["nats"]["permissions"]["publish"]["allow"];
        let subscribe_allow = &body["nats"]["permissions"]["subscribe"]["allow"];

        assert_eq!(
            publish_allow[0].as_str().unwrap(),
            "openlink.v1.vatsim.outbox.42"
        );
        assert_eq!(
            subscribe_allow[0].as_str().unwrap(),
            "openlink.v1.vatsim.inbox.42"
        );
    }

    #[test]
    fn jwt_sub_matches_user_nkey() {
        let kp = test_account_kp();
        let user_pub = "UTEST_PUBLIC_KEY";
        let jwt = sign_user_jwt(&kp, user_pub, "99", &NetworkId::new("icao"), 3600).unwrap();

        let body_b64 = jwt.split('.').nth(1).unwrap();
        let body_bytes = URL_SAFE_NO_PAD.decode(body_b64).unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body["sub"].as_str().unwrap(), user_pub);
        assert_eq!(body["name"].as_str().unwrap(), "99");
    }

    #[test]
    fn jwt_issuer_is_account_public_key() {
        let kp = test_account_kp();
        let expected_issuer = kp.public_key();
        let jwt =
            sign_user_jwt(&kp, "UKEY", "1", &NetworkId::new("vatsim"), 3600).unwrap();

        let body_b64 = jwt.split('.').nth(1).unwrap();
        let body_bytes = URL_SAFE_NO_PAD.decode(body_b64).unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body["iss"].as_str().unwrap(), expected_issuer);
    }

    #[test]
    fn jwt_expiry_matches_ttl() {
        let kp = test_account_kp();
        let ttl = 7200_u64;
        let jwt = sign_user_jwt(&kp, "UKEY", "1", &NetworkId::new("vatsim"), ttl).unwrap();

        let body_b64 = jwt.split('.').nth(1).unwrap();
        let body_bytes = URL_SAFE_NO_PAD.decode(body_b64).unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        let iat = body["iat"].as_u64().unwrap();
        let exp = body["exp"].as_u64().unwrap();
        assert_eq!(exp - iat, ttl);
    }

    // -- server JWT --------------------------------------------------------

    #[test]
    fn server_jwt_has_wildcard_permissions() {
        let kp = test_account_kp();
        let net = NetworkId::new("vatsim");
        let jwt = sign_server_jwt(&kp, "USERVER", &net, 3600).unwrap();

        let body_b64 = jwt.split('.').nth(1).unwrap();
        let body_bytes = URL_SAFE_NO_PAD.decode(body_b64).unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        let pub_allow: Vec<&str> = body["nats"]["permissions"]["publish"]["allow"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let sub_allow: Vec<&str> = body["nats"]["permissions"]["subscribe"]["allow"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();

        assert!(pub_allow.contains(&"openlink.v1.vatsim.inbox.>"));
        assert!(pub_allow.contains(&"$JS.API.>"));
        assert!(sub_allow.contains(&"openlink.v1.vatsim.outbox.>"));
        assert!(sub_allow.contains(&"$JS.API.>"));
    }

    #[test]
    fn server_jwt_name_contains_network() {
        let kp = test_account_kp();
        let jwt = sign_server_jwt(&kp, "USERVER", &NetworkId::new("icao"), 3600).unwrap();

        let body_b64 = jwt.split('.').nth(1).unwrap();
        let body_bytes = URL_SAFE_NO_PAD.decode(body_b64).unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body["name"].as_str().unwrap(), "openlink-server-icao");
    }
}
