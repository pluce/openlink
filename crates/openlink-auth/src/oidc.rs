//! OIDC authorization-code exchange.
//!
//! Validates an authorization code against the configured identity provider
//! and returns the user's CID (connection identifier) on success.

use crate::config::OidcProviderConfig;
use crate::error::AuthError;

/// Exchange an OIDC authorization code for a user CID.
///
/// Sends the code to the provider's token endpoint and extracts the user
/// identity from the response.
///
/// The current implementation relies on the access-token format returned
/// by mock-oidc (`vatsim_{cid}`).  A production implementation would
/// instead validate the `id_token` JWT and read the `sub` claim.
pub async fn exchange_code(
    provider: &OidcProviderConfig,
    code: &str,
) -> Result<String, AuthError> {
    let client = reqwest::Client::new();

    let res = client
        .post(&provider.token_url)
        .form(&[("code", code), ("grant_type", "authorization_code")])
        .send()
        .await?;

    if !res.status().is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(AuthError::OidcExchangeFailed(format!(
            "provider returned error: {text}"
        )));
    }

    let body: serde_json::Value = res.json().await?;

    // Extract CID from the access_token (mock-oidc format: "vatsim_{cid}")
    let access_token = body["access_token"]
        .as_str()
        .ok_or_else(|| AuthError::OidcExchangeFailed("missing access_token".into()))?;

    extract_cid_from_token(access_token)
}

/// Parse the CID from a mock-oidc access token.
///
/// The mock provider returns tokens in the form `"vatsim_{cid}"`.
/// We take everything after the last `_` as the CID.
fn extract_cid_from_token(token: &str) -> Result<String, AuthError> {
    token
        .rsplit('_')
        .next()
        .filter(|s| !s.is_empty())
        .map(String::from)
        .ok_or_else(|| {
            AuthError::OidcExchangeFailed(format!("unexpected access_token format: {token}"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_cid_standard_format() {
        let cid = extract_cid_from_token("vatsim_123456").unwrap();
        assert_eq!(cid, "123456");
    }

    #[test]
    fn extract_cid_no_underscore() {
        let result = extract_cid_from_token("nounderscore");
        // "nounderscore" has no _, rsplit returns "nounderscore" as the
        // first (and only) element, which is non-empty → Ok.
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "nounderscore");
    }

    #[test]
    fn extract_cid_trailing_underscore() {
        let result = extract_cid_from_token("vatsim_");
        assert!(result.is_err());
    }

    #[test]
    fn extract_cid_multiple_underscores() {
        let cid = extract_cid_from_token("some_prefix_12345").unwrap();
        assert_eq!(cid, "12345");
    }
}
