//! OpenLink auth service — exchanges OIDC authorization codes for NATS JWTs.
//!
//! The service is configured with a mapping of network keys (e.g. `demonetwork`)
//! to OIDC provider parameters.  On each request it:
//!
//! 1. Validates the OIDC code against the identity provider.
//! 2. Signs a scoped NATS user JWT (publish outbox / subscribe inbox).
//! 3. Returns the JWT and authenticated CID to the caller.

mod config;
mod error;
mod jwt;
mod oidc;

use std::sync::Arc;

use axum::extract::{Json, State};
use axum::routing::{get, post};
use axum::Router;
use nkeys::KeyPair;
use openlink_models::NetworkId;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::config::AppConfig;
use crate::error::AuthError;

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

/// State shared across all Axum handlers.
struct AppState {
    /// NATS account key-pair used to sign user JWTs.
    account_kp: KeyPair,
    /// Global configuration (network → OIDC mapping).
    config: AppConfig,
    /// Shared secret that server instances present to obtain a master JWT.
    server_secret: String,
}

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

/// Body of `POST /exchange`.
#[derive(Deserialize)]
struct ExchangeRequest {
    /// OIDC authorization code received from the identity provider.
    oidc_code: String,
    /// Client-generated NKey public key to embed in the JWT.
    user_nkey_public: String,
    /// Network the user wants to authenticate against (e.g. `"demonetwork"`).
    #[serde(default = "default_network")]
    network: String,
}

fn default_network() -> String {
    "demonetwork".to_string()
}

/// Response of `POST /exchange`.
#[derive(Serialize)]
struct ExchangeResponse {
    /// Signed NATS user JWT.
    jwt: String,
    /// Authenticated CID.
    cid: String,
    /// Network the JWT was issued for.
    network: String,
}

/// Body of `POST /exchange-server`.
#[derive(Deserialize)]
struct ExchangeServerRequest {
    /// Pre-shared secret proving the caller is a legitimate server.
    server_secret: String,
    /// Client-generated NKey public key to embed in the JWT.
    user_nkey_public: String,
    /// Network the server needs master access to.
    network: String,
}

/// Response of `POST /exchange-server`.
#[derive(Serialize)]
struct ExchangeServerResponse {
    /// Signed NATS server JWT with wildcard permissions.
    jwt: String,
    /// Network the JWT was issued for.
    network: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /public-key` — return the NATS account public key.
///
/// Clients or monitoring tools can use this to verify they are talking
/// to the expected auth service.
async fn get_public_key(State(state): State<Arc<AppState>>) -> String {
    state.account_kp.public_key()
}

/// `POST /exchange` — exchange an OIDC code for a NATS JWT.
async fn exchange_token(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExchangeRequest>,
) -> Result<Json<ExchangeResponse>, AuthError> {
    let network = NetworkId::new(&req.network);

    // 1. Resolve OIDC provider for the requested network
    let provider = state
        .config
        .provider_for(&network)
        .ok_or_else(|| AuthError::UnknownNetwork(req.network.clone()))?;

    info!(network = %network, "exchange request received");

    // 2. Exchange the OIDC code for a CID
    let cid = oidc::exchange_code(provider, &req.oidc_code).await?;
    info!(network = %network, cid = %cid, "OIDC authentication successful");

    // 3. Sign a scoped NATS JWT
    let jwt_ttl_secs = 3600;
    let jwt_token =
        jwt::sign_user_jwt(&state.account_kp, &req.user_nkey_public, &cid, &network, jwt_ttl_secs)?;

    info!(network = %network, cid = %cid, "JWT issued");

    Ok(Json(ExchangeResponse {
        jwt: jwt_token,
        cid,
        network: req.network,
    }))
}

/// `POST /exchange-server` — exchange a server secret for a master NATS JWT.
///
/// The server JWT grants wildcard publish/subscribe on all outbox and inbox
/// subjects of the requested network, plus JetStream API access for KV stores.
async fn exchange_server_token(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExchangeServerRequest>,
) -> Result<Json<ExchangeServerResponse>, AuthError> {
    // 1. Verify the shared secret
    if req.server_secret != state.server_secret {
        return Err(AuthError::OidcExchangeFailed(
            "invalid server secret".into(),
        ));
    }

    let network = NetworkId::new(&req.network);

    info!(network = %network, "server token request");

    // 2. Sign a server-scoped NATS JWT (long-lived: 24h)
    let jwt_ttl_secs = 86_400;
    let jwt_token = jwt::sign_server_jwt(
        &state.account_kp,
        &req.user_nkey_public,
        &network,
        jwt_ttl_secs,
    )?;

    info!(network = %network, "server JWT issued");

    Ok(Json(ExchangeServerResponse {
        jwt: jwt_token,
        network: req.network,
    }))
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    // Structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Configuration
    let config = AppConfig::from_env();

    // NATS account key-pair (in production this would be loaded from a vault)
    let account_kp = KeyPair::new_account();
    info!(
        public_key = %account_kp.public_key(),
        "NATS account key generated"
    );

    for (network, provider) in &config.networks {
        info!(
            network = %network,
            token_url = %provider.token_url,
            "OIDC provider registered"
        );
    }

    let server_secret = std::env::var("SERVER_SECRET")
        .unwrap_or_else(|_| "openlink-dev-secret".to_string());
    info!("server secret configured (use SERVER_SECRET env var in production)");

    let listen_port = config.listen_port;

    let state = Arc::new(AppState {
        account_kp,
        config,
        server_secret,
    });

    let app = Router::new()
        .route("/exchange", post(exchange_token))
        .route("/exchange-server", post(exchange_server_token))
        .route("/public-key", get(get_public_key))
        .with_state(state);

    let addr = format!("0.0.0.0:{listen_port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind listener");

    info!(address = %addr, "auth service listening");
    axum::serve(listener, app).await.expect("server error");
}
