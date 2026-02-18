//! # OpenLink Message Builder
//!
//! Fluent builder API for constructing [`OpenLinkMessage`](crate::OpenLinkMessage) and
//! [`OpenLinkEnvelope`](crate::OpenLinkEnvelope) objects without manually nesting
//! the deep model hierarchy.
//!
//! ## Quick examples
//!
//! ```rust
//! use openlink_models::MessageBuilder;
//!
//! // Build just an OpenLinkMessage (inner payload)
//! let msg = MessageBuilder::cpdlc("AFR123", "394A0B")
//!     .from("AFR123")
//!     .to("LFPG")
//!     .logon_request("LFPG", "LFPG", "KJFK")
//!     .build();
//!
//! // Build a complete OpenLinkEnvelope in one chain
//! let envelope = MessageBuilder::cpdlc("AFR123", "394A0B")
//!     .from("AFR123")
//!     .to("LFPG")
//!     .logon_request("LFPG", "LFPG", "KJFK")
//!     .envelope()
//!     .source_address("vatsim", "765283")
//!     .destination_server("vatsim")
//!     .build();
//!
//! // Station status with full envelope
//! let envelope = MessageBuilder::station_status("1234", "LFPG", "394A0B")
//!     .online()
//!     .envelope()
//!     .source_address("vatsim", "1234")
//!     .destination_server("vatsim")
//!     .build();
//! ```

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::acars::{
    AcarsEndpointCallsign, AcarsEnvelope, AcarsMessage, AcarsRouting, AcarsRoutingEndpoint,
};
use crate::cpdlc::{
    CpdlcEnvelope, CpdlcMessage, CpdlcMessageType, CpdlcMetaMessage, FlightLevel,
    ICAOAirportCode,
};
use crate::envelope::{OpenLinkEnvelope, OpenLinkMessage};
use crate::network::{NetworkAddress, NetworkId, OpenLinkRouting, OpenLinkRoutingEndpoint};
use crate::station::{MetaMessage, StationId, StationStatus};

// ─── CPDLC Message Builder ───────────────────────────────────────────

/// Builder for CPDLC messages (both meta and application).
///
/// Created via [`MessageBuilder::cpdlc`].
pub struct CpdlcMessageBuilder {
    aircraft_callsign: String,
    aircraft_address: String,
    source: Option<String>,
    destination: Option<String>,
    message_type: Option<CpdlcMessageType>,
}

impl CpdlcMessageBuilder {
    fn new(aircraft_callsign: impl Into<String>, aircraft_address: impl Into<String>) -> Self {
        Self {
            aircraft_callsign: aircraft_callsign.into(),
            aircraft_address: aircraft_address.into(),
            source: None,
            destination: None,
            message_type: None,
        }
    }

    /// Set the CPDLC source callsign (who is sending).
    pub fn from(mut self, callsign: impl Into<String>) -> Self {
        self.source = Some(callsign.into());
        self
    }

    /// Set the CPDLC destination callsign (who should receive).
    pub fn to(mut self, callsign: impl Into<String>) -> Self {
        self.destination = Some(callsign.into());
        self
    }

    // ── Meta messages ────────────────────────────────────────────────

    /// Logon request from an aircraft to a ground station.
    pub fn logon_request(
        mut self,
        station: impl Into<String>,
        origin: impl Into<String>,
        destination: impl Into<String>,
    ) -> Self {
        let station: String = station.into();
        self.message_type = Some(CpdlcMessageType::Meta(CpdlcMetaMessage::LogonRequest {
            station: AcarsEndpointCallsign::new(&station),
            flight_plan_origin: ICAOAirportCode::new(origin.into().as_str()),
            flight_plan_destination: ICAOAirportCode::new(destination.into().as_str()),
        }));
        self
    }

    /// Logon response (accept / reject).
    pub fn logon_response(mut self, accepted: bool) -> Self {
        self.message_type = Some(CpdlcMessageType::Meta(CpdlcMetaMessage::LogonResponse {
            accepted,
        }));
        self
    }

    /// Connection request (ATC → aircraft).
    pub fn connection_request(mut self) -> Self {
        self.message_type = Some(CpdlcMessageType::Meta(
            CpdlcMetaMessage::ConnectionRequest,
        ));
        self
    }

    /// Connection response (aircraft → ATC).
    pub fn connection_response(mut self, accepted: bool) -> Self {
        self.message_type = Some(CpdlcMessageType::Meta(
            CpdlcMetaMessage::ConnectionResponse { accepted },
        ));
        self
    }

    /// Contact request — ask aircraft to contact another station.
    pub fn contact_request(mut self, station: impl Into<String>) -> Self {
        let station: String = station.into();
        self.message_type = Some(CpdlcMessageType::Meta(CpdlcMetaMessage::ContactRequest {
            station: AcarsEndpointCallsign::new(&station),
        }));
        self
    }

    /// Contact response (accept / reject).
    pub fn contact_response(mut self, accepted: bool) -> Self {
        self.message_type = Some(CpdlcMessageType::Meta(
            CpdlcMetaMessage::ContactResponse { accepted },
        ));
        self
    }

    /// Contact complete — indicates the contact handoff is finished.
    pub fn contact_complete(mut self) -> Self {
        self.message_type = Some(CpdlcMessageType::Meta(CpdlcMetaMessage::ContactComplete));
        self
    }

    /// Forward a logon to another station.
    pub fn logon_forward(
        mut self,
        flight: impl Into<String>,
        origin: impl Into<String>,
        destination: impl Into<String>,
        new_station: impl Into<String>,
    ) -> Self {
        let flight: String = flight.into();
        let new_station: String = new_station.into();
        self.message_type = Some(CpdlcMessageType::Meta(CpdlcMetaMessage::LogonForward {
            flight: AcarsEndpointCallsign::new(&flight),
            flight_plan_origin: ICAOAirportCode::new(origin.into().as_str()),
            flight_plan_destination: ICAOAirportCode::new(destination.into().as_str()),
            new_station: AcarsEndpointCallsign::new(&new_station),
        }));
        self
    }

    /// Set the Next Data Authority.
    pub fn next_data_authority(
        mut self,
        nda_callsign: impl Into<String>,
        nda_address: impl Into<String>,
    ) -> Self {
        self.message_type = Some(CpdlcMessageType::Meta(
            CpdlcMetaMessage::NextDataAuthority {
                nda: AcarsRoutingEndpoint::new(
                    nda_callsign.into().as_str(),
                    nda_address.into().as_str(),
                ),
            },
        ));
        self
    }

    // ── Application messages ─────────────────────────────────────────

    /// (Uplink) Climb to a flight level.
    pub fn climb_to(mut self, level: FlightLevel) -> Self {
        self.message_type = Some(CpdlcMessageType::Application(
            CpdlcMessage::UplinkClimbToFlightLevel { level },
        ));
        self
    }

    /// (Downlink) Request a level change.
    pub fn request_level_change(mut self, level: FlightLevel) -> Self {
        self.message_type = Some(CpdlcMessageType::Application(
            CpdlcMessage::DownlinkRequestLevelChange { level },
        ));
        self
    }

    /// Set a raw [`CpdlcMessageType`] directly for advanced / future message types.
    pub fn raw_message(mut self, msg: CpdlcMessageType) -> Self {
        self.message_type = Some(msg);
        self
    }

    /// Consume the builder and produce an [`OpenLinkMessage`].
    ///
    /// # Panics
    ///
    /// Panics if `from`, `to`, or a message method has not been called.
    pub fn build(self) -> OpenLinkMessage {
        let source = self
            .source
            .expect("CpdlcMessageBuilder: `from()` must be called before `build()`");
        let destination = self
            .destination
            .expect("CpdlcMessageBuilder: `to()` must be called before `build()`");
        let message_type = self.message_type.expect(
            "CpdlcMessageBuilder: a message method (e.g. `logon_request()`) must be called before `build()`",
        );

        OpenLinkMessage::Acars(AcarsEnvelope {
            routing: AcarsRouting {
                aircraft: AcarsRoutingEndpoint::new(
                    self.aircraft_callsign.as_str(),
                    self.aircraft_address.as_str(),
                ),
            },
            message: AcarsMessage::CPDLC(CpdlcEnvelope {
                source: AcarsEndpointCallsign::new(&source),
                destination: AcarsEndpointCallsign::new(&destination),
                message: message_type,
            }),
        })
    }

    /// Transition into an [`EnvelopeBuilder`] to wrap this message in an
    /// [`OpenLinkEnvelope`].
    ///
    /// Calls `.build()` internally, so the same validation applies.
    pub fn envelope(self) -> EnvelopeBuilder {
        EnvelopeBuilder::new(self.build())
    }
}

// ─── Station Status Builder ──────────────────────────────────────────

/// Builder for [`MetaMessage::StationStatus`] messages.
///
/// Created via [`MessageBuilder::station_status`].
pub struct StationStatusBuilder {
    network_address: String,
    callsign: String,
    acars_address: String,
    status: Option<StationStatus>,
}

impl StationStatusBuilder {
    fn new(
        network_address: impl Into<String>,
        callsign: impl Into<String>,
        acars_address: impl Into<String>,
    ) -> Self {
        Self {
            network_address: network_address.into(),
            callsign: callsign.into(),
            acars_address: acars_address.into(),
            status: None,
        }
    }

    /// Mark the station as online.
    pub fn online(mut self) -> Self {
        self.status = Some(StationStatus::Online);
        self
    }

    /// Mark the station as offline.
    pub fn offline(mut self) -> Self {
        self.status = Some(StationStatus::Offline);
        self
    }

    /// Consume the builder and produce an [`OpenLinkMessage`].
    ///
    /// # Panics
    ///
    /// Panics if neither `online()` nor `offline()` has been called.
    pub fn build(self) -> OpenLinkMessage {
        let status = self.status.expect(
            "StationStatusBuilder: `online()` or `offline()` must be called before `build()`",
        );

        OpenLinkMessage::Meta(MetaMessage::StationStatus(
            StationId::new(&self.network_address),
            status,
            AcarsRoutingEndpoint::new(self.callsign.as_str(), self.acars_address.as_str()),
        ))
    }

    /// Transition into an [`EnvelopeBuilder`] to wrap this message in an
    /// [`OpenLinkEnvelope`].
    ///
    /// Calls `.build()` internally, so the same validation applies.
    pub fn envelope(self) -> EnvelopeBuilder {
        EnvelopeBuilder::new(self.build())
    }
}

// ─── Envelope Builder ────────────────────────────────────────────────

/// Builder for constructing a complete [`OpenLinkEnvelope`].
///
/// Can be created:
/// - From an inner builder via `.envelope()` (recommended)
/// - Standalone via [`MessageBuilder::envelope`] with a pre-built [`OpenLinkMessage`]
///
/// Sensible defaults are applied:
/// - `id` → random UUID v4
/// - `timestamp` → `Utc::now()`
/// - `token` → `""` (empty)
/// - `correlation_id` → `None`
pub struct EnvelopeBuilder {
    id: Option<Uuid>,
    timestamp: Option<DateTime<Utc>>,
    correlation_id: Option<String>,
    token: Option<String>,
    source: Option<OpenLinkRoutingEndpoint>,
    destination: Option<OpenLinkRoutingEndpoint>,
    payload: OpenLinkMessage,
}

impl EnvelopeBuilder {
    fn new(payload: OpenLinkMessage) -> Self {
        Self {
            id: None,
            timestamp: None,
            correlation_id: None,
            token: None,
            source: None,
            destination: None,
            payload,
        }
    }

    /// Override the envelope ID (defaults to a random UUID v4).
    pub fn id(mut self, id: Uuid) -> Self {
        self.id = Some(id);
        self
    }

    /// Override the envelope timestamp (defaults to `Utc::now()`).
    pub fn timestamp(mut self, ts: DateTime<Utc>) -> Self {
        self.timestamp = Some(ts);
        self
    }

    /// Set a correlation ID to link request ↔ response pairs.
    pub fn correlation_id(mut self, cid: impl Into<String>) -> Self {
        self.correlation_id = Some(cid.into());
        self
    }

    /// Set the authentication / authorization token.
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    // ── Routing shortcuts ────────────────────────────────────────────

    /// Set the source as a network address (e.g. a user CID).
    ///
    /// Produces `OpenLinkRoutingEndpoint::Address(network_id, address)`.
    pub fn source_address(
        mut self,
        network_id: impl Into<String>,
        address: impl Into<String>,
    ) -> Self {
        self.source = Some(OpenLinkRoutingEndpoint::Address(
            NetworkId::new(&network_id.into()),
            NetworkAddress::new(&address.into()),
        ));
        self
    }

    /// Set the source as a server endpoint.
    ///
    /// Produces `OpenLinkRoutingEndpoint::Server(network_id)`.
    pub fn source_server(mut self, network_id: impl Into<String>) -> Self {
        self.source = Some(OpenLinkRoutingEndpoint::Server(
            NetworkId::new(&network_id.into()),
        ));
        self
    }

    /// Set the destination as a network address (e.g. a specific user).
    ///
    /// Produces `OpenLinkRoutingEndpoint::Address(network_id, address)`.
    pub fn destination_address(
        mut self,
        network_id: impl Into<String>,
        address: impl Into<String>,
    ) -> Self {
        self.destination = Some(OpenLinkRoutingEndpoint::Address(
            NetworkId::new(&network_id.into()),
            NetworkAddress::new(&address.into()),
        ));
        self
    }

    /// Set the destination as a server endpoint.
    ///
    /// Produces `OpenLinkRoutingEndpoint::Server(network_id)`.
    pub fn destination_server(mut self, network_id: impl Into<String>) -> Self {
        self.destination = Some(OpenLinkRoutingEndpoint::Server(
            NetworkId::new(&network_id.into()),
        ));
        self
    }

    /// Set a raw [`OpenLinkRoutingEndpoint`] as the source.
    pub fn source_raw(mut self, endpoint: OpenLinkRoutingEndpoint) -> Self {
        self.source = Some(endpoint);
        self
    }

    /// Set a raw [`OpenLinkRoutingEndpoint`] as the destination.
    pub fn destination_raw(mut self, endpoint: OpenLinkRoutingEndpoint) -> Self {
        self.destination = Some(endpoint);
        self
    }

    /// Consume the builder and produce an [`OpenLinkEnvelope`].
    ///
    /// # Panics
    ///
    /// Panics if `source` or `destination` routing has not been set.
    pub fn build(self) -> OpenLinkEnvelope {
        let source = self
            .source
            .expect("EnvelopeBuilder: a source must be set (e.g. `source_address()`)");
        let destination = self
            .destination
            .expect("EnvelopeBuilder: a destination must be set (e.g. `destination_server()`)");

        OpenLinkEnvelope {
            id: self.id.unwrap_or_else(Uuid::new_v4),
            timestamp: self.timestamp.unwrap_or_else(Utc::now),
            correlation_id: self.correlation_id,
            token: self.token.unwrap_or_default(),
            routing: OpenLinkRouting {
                source,
                destination,
            },
            payload: self.payload,
        }
    }
}

// ─── Top-level entry point ───────────────────────────────────────────

/// Unified entry point for building OpenLink messages.
///
/// Provides factory methods that return specialised builders.
pub struct MessageBuilder;

impl MessageBuilder {
    /// Start building a CPDLC message for a given aircraft.
    ///
    /// # Arguments
    ///
    /// * `aircraft_callsign` — ACARS callsign of the aircraft (e.g. `"AFR123"`)
    /// * `aircraft_address`  — ACARS address / ICAO 24-bit code (e.g. `"394A0B"`)
    pub fn cpdlc(
        aircraft_callsign: impl Into<String>,
        aircraft_address: impl Into<String>,
    ) -> CpdlcMessageBuilder {
        CpdlcMessageBuilder::new(aircraft_callsign, aircraft_address)
    }

    /// Wrap an already-built [`OpenLinkMessage`] in an envelope.
    ///
    /// Use this when you have a raw [`OpenLinkMessage`] (e.g. from `.build()`)
    /// and want to construct the full [`OpenLinkEnvelope`] separately.
    pub fn envelope(payload: OpenLinkMessage) -> EnvelopeBuilder {
        EnvelopeBuilder::new(payload)
    }

    /// Start building a station status announcement.
    ///
    /// # Arguments
    ///
    /// * `network_address` — The station's network address / CID
    /// * `callsign`        — The station's ACARS callsign
    /// * `acars_address`   — The station's ACARS address
    pub fn station_status(
        network_address: impl Into<String>,
        callsign: impl Into<String>,
        acars_address: impl Into<String>,
    ) -> StationStatusBuilder {
        StationStatusBuilder::new(network_address, callsign, acars_address)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_logon_request() {
        let msg = MessageBuilder::cpdlc("AFR123", "394A0B")
            .from("AFR123")
            .to("LFPG")
            .logon_request("LFPG", "LFPG", "KJFK")
            .build();

        match msg {
            OpenLinkMessage::Acars(env) => {
                assert_eq!(env.routing.aircraft.callsign.to_string(), "AFR123");
                assert_eq!(env.routing.aircraft.address.to_string(), "394A0B");
                match env.message {
                    AcarsMessage::CPDLC(cpdlc) => {
                        assert_eq!(cpdlc.source.to_string(), "AFR123");
                        assert_eq!(cpdlc.destination.to_string(), "LFPG");
                        match cpdlc.message {
                            CpdlcMessageType::Meta(CpdlcMetaMessage::LogonRequest { .. }) => {}
                            other => panic!("Expected LogonRequest, got {:?}", other),
                        }
                    }
                }
            }
            other => panic!("Expected Acars, got {:?}", other),
        }
    }

    #[test]
    fn build_logon_response() {
        let msg = MessageBuilder::cpdlc("AFR123", "394A0B")
            .from("LFPG")
            .to("AFR123")
            .logon_response(true)
            .build();

        match msg {
            OpenLinkMessage::Acars(env) => match env.message {
                AcarsMessage::CPDLC(cpdlc) => {
                    assert_eq!(cpdlc.source.to_string(), "LFPG");
                    assert_eq!(cpdlc.destination.to_string(), "AFR123");
                    match cpdlc.message {
                        CpdlcMessageType::Meta(CpdlcMetaMessage::LogonResponse {
                            accepted,
                        }) => assert!(accepted),
                        other => panic!("Expected LogonResponse, got {:?}", other),
                    }
                }
            },
            other => panic!("Expected Acars, got {:?}", other),
        }
    }

    #[test]
    fn build_connection_request() {
        let msg = MessageBuilder::cpdlc("AFR123", "394A0B")
            .from("LFPG")
            .to("AFR123")
            .connection_request()
            .build();

        match msg {
            OpenLinkMessage::Acars(env) => match env.message {
                AcarsMessage::CPDLC(cpdlc) => match cpdlc.message {
                    CpdlcMessageType::Meta(CpdlcMetaMessage::ConnectionRequest) => {}
                    other => panic!("Expected ConnectionRequest, got {:?}", other),
                },
            },
            other => panic!("Expected Acars, got {:?}", other),
        }
    }

    #[test]
    fn build_connection_response() {
        let msg = MessageBuilder::cpdlc("AFR123", "394A0B")
            .from("AFR123")
            .to("LFPG")
            .connection_response(true)
            .build();

        match msg {
            OpenLinkMessage::Acars(env) => match env.message {
                AcarsMessage::CPDLC(cpdlc) => match cpdlc.message {
                    CpdlcMessageType::Meta(CpdlcMetaMessage::ConnectionResponse {
                        accepted,
                    }) => assert!(accepted),
                    other => panic!("Expected ConnectionResponse, got {:?}", other),
                },
            },
            other => panic!("Expected Acars, got {:?}", other),
        }
    }

    #[test]
    fn build_station_online() {
        let msg = MessageBuilder::station_status("1234", "LFPG", "39401A")
            .online()
            .build();

        match msg {
            OpenLinkMessage::Meta(MetaMessage::StationStatus(id, status, endpoint)) => {
                assert_eq!(id.to_string(), "1234");
                assert_eq!(status, StationStatus::Online);
                assert_eq!(endpoint.callsign.to_string(), "LFPG");
                assert_eq!(endpoint.address.to_string(), "39401A");
            }
            other => panic!("Expected StationStatus, got {:?}", other),
        }
    }

    #[test]
    fn build_climb_to() {
        let msg = MessageBuilder::cpdlc("AFR123", "394A0B")
            .from("LFPG")
            .to("AFR123")
            .climb_to(FlightLevel::new(350))
            .build();

        match msg {
            OpenLinkMessage::Acars(env) => match env.message {
                AcarsMessage::CPDLC(cpdlc) => match cpdlc.message {
                    CpdlcMessageType::Application(CpdlcMessage::UplinkClimbToFlightLevel {
                        level,
                    }) => assert_eq!(level, FlightLevel::new(350)),
                    other => panic!("Expected ClimbTo, got {:?}", other),
                },
            },
            other => panic!("Expected Acars, got {:?}", other),
        }
    }

    // ── Envelope builder tests ─────────────────────────────────────

    #[test]
    fn build_cpdlc_envelope_from_chain() {
        let envelope = MessageBuilder::cpdlc("AFR123", "394A0B")
            .from("AFR123")
            .to("LFPG")
            .logon_request("LFPG", "LFPG", "KJFK")
            .envelope()
            .source_address("vatsim", "765283")
            .destination_server("vatsim")
            .token("my-token")
            .correlation_id("corr-001")
            .build();

        // Envelope metadata
        assert_ne!(envelope.id, Uuid::nil());
        assert_eq!(envelope.token, "my-token");
        assert_eq!(envelope.correlation_id, Some("corr-001".to_string()));

        // Routing
        match &envelope.routing.source {
            OpenLinkRoutingEndpoint::Address(net, addr) => {
                assert_eq!(net.to_string(), "vatsim");
                assert_eq!(addr.to_string(), "765283");
            }
            other => panic!("Expected Address source, got {:?}", other),
        }
        match &envelope.routing.destination {
            OpenLinkRoutingEndpoint::Server(net) => {
                assert_eq!(net.to_string(), "vatsim");
            }
            other => panic!("Expected Server destination, got {:?}", other),
        }

        // Payload
        match &envelope.payload {
            OpenLinkMessage::Acars(env) => match &env.message {
                AcarsMessage::CPDLC(cpdlc) => match &cpdlc.message {
                    CpdlcMessageType::Meta(CpdlcMetaMessage::LogonRequest { .. }) => {}
                    other => panic!("Expected LogonRequest, got {:?}", other),
                },
            },
            other => panic!("Expected Acars, got {:?}", other),
        }
    }

    #[test]
    fn build_station_status_envelope() {
        let envelope = MessageBuilder::station_status("1234", "LFPG", "39401A")
            .online()
            .envelope()
            .source_address("vatsim", "1234")
            .destination_server("vatsim")
            .build();

        match &envelope.routing.source {
            OpenLinkRoutingEndpoint::Address(net, addr) => {
                assert_eq!(net.to_string(), "vatsim");
                assert_eq!(addr.to_string(), "1234");
            }
            other => panic!("Expected Address source, got {:?}", other),
        }
        match &envelope.payload {
            OpenLinkMessage::Meta(MetaMessage::StationStatus(id, status, _)) => {
                assert_eq!(id.to_string(), "1234");
                assert_eq!(*status, StationStatus::Online);
            }
            other => panic!("Expected StationStatus, got {:?}", other),
        }
    }

    #[test]
    fn build_envelope_from_prebuilt_message() {
        let msg = MessageBuilder::cpdlc("AFR123", "394A0B")
            .from("LFPG")
            .to("AFR123")
            .connection_request()
            .build();

        let test_id = Uuid::nil();
        let envelope = MessageBuilder::envelope(msg)
            .source_server("vatsim")
            .destination_address("vatsim", "765283")
            .id(test_id)
            .build();

        assert_eq!(envelope.id, test_id);
        assert_eq!(envelope.token, ""); // default
        assert_eq!(envelope.correlation_id, None);

        match &envelope.routing.source {
            OpenLinkRoutingEndpoint::Server(net) => assert_eq!(net.to_string(), "vatsim"),
            other => panic!("Expected Server source, got {:?}", other),
        }
        match &envelope.routing.destination {
            OpenLinkRoutingEndpoint::Address(net, addr) => {
                assert_eq!(net.to_string(), "vatsim");
                assert_eq!(addr.to_string(), "765283");
            }
            other => panic!("Expected Address destination, got {:?}", other),
        }
    }

    #[test]
    #[should_panic(expected = "source must be set")]
    fn envelope_panic_without_source() {
        MessageBuilder::cpdlc("AFR123", "394A0B")
            .from("AFR123")
            .to("LFPG")
            .logon_response(true)
            .envelope()
            .destination_server("vatsim")
            .build();
    }

    #[test]
    #[should_panic(expected = "destination must be set")]
    fn envelope_panic_without_destination() {
        MessageBuilder::cpdlc("AFR123", "394A0B")
            .from("AFR123")
            .to("LFPG")
            .logon_response(true)
            .envelope()
            .source_address("vatsim", "1234")
            .build();
    }

    // ── Original panic tests ─────────────────────────────────────────

    #[test]
    #[should_panic(expected = "from()")]
    fn panic_without_from() {
        MessageBuilder::cpdlc("AFR123", "394A0B")
            .to("LFPG")
            .logon_response(true)
            .build();
    }

    #[test]
    #[should_panic(expected = "to()")]
    fn panic_without_to() {
        MessageBuilder::cpdlc("AFR123", "394A0B")
            .from("AFR123")
            .logon_response(true)
            .build();
    }

    #[test]
    #[should_panic(expected = "message method")]
    fn panic_without_message() {
        MessageBuilder::cpdlc("AFR123", "394A0B")
            .from("AFR123")
            .to("LFPG")
            .build();
    }
}
