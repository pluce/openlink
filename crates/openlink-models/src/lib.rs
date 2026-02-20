#![deny(missing_docs)]

//! # OpenLink Models
//!
//! Core data types for the OpenLink datalink simulation network.
//!
//! ## Message hierarchy
//!
//! ```text
//! OpenLinkEnvelope
//! ├── OpenLinkMessage::Acars(AcarsEnvelope)
//! │   └── AcarsMessage::CPDLC(CpdlcEnvelope)
//! │       ├── CpdlcMessageType::Application(CpdlcMessage)
//! │       └── CpdlcMessageType::Meta(CpdlcMetaMessage)
//! │           ├── Logon / Connection / Contact / Transfer
//! │           └── NextDataAuthority
//! └── OpenLinkMessage::Meta(MetaMessage)
//!     └── StationStatus
//! ```
//!
//! ## Module layout
//!
//! | Module | Contents |
//! |--------|----------|
//! | [`network`] | Network-level addressing (`NetworkId`, `NetworkAddress`, routing) |
//! | [`acars`] | ACARS envelope, routing, callsigns, addresses |
//! | [`cpdlc`] | CPDLC messages, meta-messages, serialisation |
//! | [`envelope`] | Top-level `OpenLinkEnvelope` and `OpenLinkMessage` |
//! | [`station`] | Ground-station identity and status |

pub mod acars;
pub mod cpdlc;
pub mod envelope;
pub mod error;
pub mod message_builder;
pub mod network;
pub mod station;

// Re-export all public types at crate root for convenience.
// Downstream crates can use `openlink_models::NetworkId` directly.
pub use acars::*;
pub use cpdlc::*;
pub use envelope::*;
pub use error::*;
pub use message_builder::*;
pub use network::*;
pub use station::*;
