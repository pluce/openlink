//! # OpenLink SDK
//!
//! Reference SDK for connecting to the **OpenLink** aviation messaging
//! network.
//!
//! The SDK provides:
//!
//! * [`OpenLinkClient`] — authenticated NATS connection for
//!   publishing and subscribing to OpenLink messages.
//! * [`NatsSubjects`] — canonical NATS subject definitions shared
//!   by clients and servers alike.
//! * [`SdkError`] — unified error type for all SDK operations.
//! * [`OpenLinkCredentials`] — portable credential struct (seed,
//!   JWT, CID).
//!
//! Builders from [`openlink_models`] are re-exported for convenience.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use openlink_models::NetworkId;
//! use openlink_sdk::OpenLinkClient;
//!
//! # async fn run() -> Result<(), openlink_sdk::SdkError> {
//! let network = NetworkId::new("vatsim");
//! let client = OpenLinkClient::connect_with_authorization_code(
//!     "nats://localhost:4222",
//!     "http://auth.example.com",
//!     "my-oidc-code",
//!     &network,
//! ).await?;
//!
//! // Subscribe to incoming messages
//! let _inbox = client.subscribe_inbox().await?;
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod credentials;
pub mod error;
pub mod subjects;

pub use client::OpenLinkClient;
pub use credentials::OpenLinkCredentials;
pub use error::SdkError;
pub use subjects::NatsSubjects;

// Re-export builders from openlink-models for ergonomic usage.
pub use openlink_models::{
    CpdlcMessageBuilder, EnvelopeBuilder, MessageBuilder, StationStatusBuilder,
};
