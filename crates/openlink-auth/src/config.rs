//! Auth service configuration.
//!
//! Maps each [`NetworkId`] to its OIDC provider parameters.  The mapping
//! is built from environment variables at startup and injected into Axum
//! handlers via [`axum::extract::State`].

use std::collections::HashMap;

use openlink_models::NetworkId;

/// OIDC provider parameters for a single network.
#[derive(Debug, Clone)]
pub struct OidcProviderConfig {
    /// Base URL of the OIDC token endpoint (e.g. `http://localhost:4000/token`).
    pub token_url: String,
}

/// Global configuration shared across all handlers.
///
/// Constructed once at startup and passed as Axum shared state.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Mapping of network key → OIDC provider.
    pub networks: HashMap<NetworkId, OidcProviderConfig>,
    /// Port to listen on (default `3001`).
    pub listen_port: u16,
}

impl AppConfig {
    /// Build the configuration from environment variables.
    ///
    /// | Variable              | Default                          | Description                     |
    /// |-----------------------|----------------------------------|---------------------------------|
    /// | `AUTH_PORT`            | `3001`                           | HTTP listen port                |
    /// | `OIDC_VATSIM_TOKEN_URL` | `http://localhost:4000/token`  | OIDC token endpoint for vatsim  |
    ///
    /// Additional networks can be added by setting
    /// `OIDC_{NETWORK}_TOKEN_URL` where `{NETWORK}` is upper-cased.
    pub fn from_env() -> Self {
        let listen_port: u16 = std::env::var("AUTH_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3001);

        let mut networks = HashMap::new();

        // vatsim — always present with a default
        let vatsim_token_url = std::env::var("OIDC_VATSIM_TOKEN_URL")
            .unwrap_or_else(|_| "http://localhost:4000/token".to_string());
        networks.insert(
            NetworkId::new("vatsim"),
            OidcProviderConfig {
                token_url: vatsim_token_url,
            },
        );

        Self {
            networks,
            listen_port,
        }
    }

    /// Look up the OIDC provider for the given network.
    pub fn provider_for(&self, network: &NetworkId) -> Option<&OidcProviderConfig> {
        self.networks.get(network)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_vatsim() {
        let cfg = AppConfig::from_env();
        let vatsim = NetworkId::new("vatsim");
        assert!(cfg.provider_for(&vatsim).is_some());
        assert!(cfg
            .provider_for(&vatsim)
            .unwrap()
            .token_url
            .contains("/token"));
    }

    #[test]
    fn default_listen_port() {
        let cfg = AppConfig::from_env();
        assert_eq!(cfg.listen_port, 3001);
    }

    #[test]
    fn unknown_network_returns_none() {
        let cfg = AppConfig::from_env();
        assert!(cfg.provider_for(&NetworkId::new("unknown")).is_none());
    }
}
