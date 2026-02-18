//! SDK error types.
//!
//! [`SdkError`] is the single error type returned by every fallible
//! operation in the SDK.  It wraps underlying transport, serialization
//! and authentication errors into a unified enum.

/// Error type for all SDK operations.
#[derive(Debug, thiserror::Error)]
pub enum SdkError {
    /// Invalid or missing configuration (e.g. bad URL, missing field).
    #[error("configuration error: {0}")]
    Config(String),

    /// Authentication or authorization failure.
    #[error("authentication failed: {0}")]
    Auth(String),

    /// NATS transport error.
    #[error("NATS error: {0}")]
    Nats(String),

    /// HTTP request failure (e.g. during OAuth flow).
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization / deserialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Generic I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<async_nats::ConnectError> for SdkError {
    fn from(e: async_nats::ConnectError) -> Self {
        SdkError::Nats(e.to_string())
    }
}

impl From<async_nats::PublishError> for SdkError {
    fn from(e: async_nats::PublishError) -> Self {
        SdkError::Nats(e.to_string())
    }
}

impl From<async_nats::SubscribeError> for SdkError {
    fn from(e: async_nats::SubscribeError) -> Self {
        SdkError::Nats(e.to_string())
    }
}
