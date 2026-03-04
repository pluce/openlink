//! # OpenLink Hoppie Bridge
//!
//! Bidirectional bridge between the Hoppie ACARS network and the OpenLink
//! datalink network.
//!
//! This bridge allows:
//! - OpenLink clients (ATC or aircraft) to exchange CPDLC messages with
//!   Hoppie-connected users.
//! - Hoppie users to reach OpenLink users transparently.
//!
//! ## Architecture
//!
//! The bridge acts as a standard OpenLink client connected via NATS, and
//! simultaneously polls the Hoppie HTTP API. Messages are translated between
//! the two formats and relayed in both directions.
//!
//! ## Modules
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`config`] | CLI arguments and bridge configuration |
//! | [`hoppie_client`] | HTTP client for the Hoppie ACARS API |
//! | [`translator`] | Bidirectional CPDLC message conversion |
//! | [`session`] | MIN mapping and deduplication tracker |
//! | [`bridge`] | Main orchestration loop |

pub mod bridge;
pub mod config;
pub mod hoppie_client;
pub mod session;
pub mod translator;
