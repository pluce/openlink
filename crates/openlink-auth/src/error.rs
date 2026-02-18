//! Error types for the OpenLink auth service.
//!
//! [`AuthError`] unifies all failure modes and implements [`axum::response::IntoResponse`]
//! so handlers can return `Result<…, AuthError>` directly.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// Errors that can occur during the authentication flow.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// The requested network has no configured OIDC provider.
    #[error("unknown network: {0}")]
    UnknownNetwork(String),

    /// The OIDC code exchange failed or the provider returned an error.
    #[error("OIDC authentication failed: {0}")]
    OidcExchangeFailed(String),

    /// The HTTP call to the OIDC provider failed at the transport level.
    #[error("failed to reach identity provider: {0}")]
    HttpError(#[from] reqwest::Error),

    /// An NKey operation failed (key generation or signing).
    #[error("NKey error: {0}")]
    NKeyError(String),

    /// JSON (de)serialisation error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::UnknownNetwork(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            Self::OidcExchangeFailed(_) => (StatusCode::UNAUTHORIZED, self.to_string()),
            Self::HttpError(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            Self::NKeyError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Self::Serialization(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        tracing::error!(%status, error = %message, "request failed");
        (status, Json(json!({ "error": message }))).into_response()
    }
}
