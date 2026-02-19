use openlink_models::{
    AcarsEndpointAddress, CpdlcApplicationMessage, CpdlcEnvelope, CpdlcMessageType,
    CpdlcMetaMessage, MessageElement, NetworkId, OpenLinkEnvelope, OpenLinkMessage,
};
use openlink_sdk::{MessageBuilder, OpenLinkClient};

/// Connect to NATS via the OpenLink SDK and return the client.
pub async fn connect_nats(
    network_id: &str,
    network_address: &str,
) -> Result<OpenLinkClient, String> {
    let nats_url =
        std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    let auth_url =
        std::env::var("AUTH_URL").unwrap_or_else(|_| "http://localhost:3001".to_string());

    OpenLinkClient::connect_with_authorization_code(
        &nats_url,
        &auth_url,
        network_address,
        &NetworkId::new(network_id),
    )
        .await
        .map_err(|e| format!("{e}"))
}

/// Build and send a Meta::StationStatus(Online) message.
pub async fn send_online_status(
    client: &OpenLinkClient,
    _network_id: &str,
    network_address: &str,
    callsign: &str,
    acars_address: &str,
) -> Result<(), String> {
    let msg = MessageBuilder::station_status(network_address, callsign, acars_address)
        .online()
        .build();
    client
        .send_to_server(msg)
        .await
        .map_err(|e| format!("{e}"))
}

/// Build a CPDLC logon request message (aircraft → ground station).
pub fn build_logon_request(
    aircraft_callsign: &str,
    aircraft_address: &AcarsEndpointAddress,
    target_station: &str,
) -> OpenLinkMessage {
    MessageBuilder::cpdlc(aircraft_callsign, aircraft_address.to_string())
        .from(aircraft_callsign)
        .to(target_station)
        .logon_request(target_station, "ZZZZ", "ZZZZ")
        .build()
}

/// Build a CPDLC logon response message (ATC → aircraft).
pub fn build_logon_response(
    atc_callsign: &str,
    aircraft_callsign: &str,
    aircraft_address: &AcarsEndpointAddress,
    accepted: bool,
) -> OpenLinkMessage {
    MessageBuilder::cpdlc(aircraft_callsign, aircraft_address.to_string())
        .from(atc_callsign)
        .to(aircraft_callsign)
        .logon_response(accepted)
        .build()
}

/// Build a CPDLC connection request message (ATC → aircraft).
pub fn build_connection_request(
    atc_callsign: &str,
    aircraft_callsign: &str,
    aircraft_address: &AcarsEndpointAddress,
) -> OpenLinkMessage {
    MessageBuilder::cpdlc(aircraft_callsign, aircraft_address.to_string())
        .from(atc_callsign)
        .to(aircraft_callsign)
        .connection_request()
        .build()
}

/// Build a CPDLC connection response message (aircraft → ATC).
pub fn build_connection_response(
    aircraft_callsign: &str,
    aircraft_address: &AcarsEndpointAddress,
    atc_callsign: &str,
    accepted: bool,
) -> OpenLinkMessage {
    MessageBuilder::cpdlc(aircraft_callsign, aircraft_address.to_string())
        .from(aircraft_callsign)
        .to(atc_callsign)
        .connection_response(accepted)
        .build()
}

/// Build a CPDLC NextDataAuthority message (ATC → aircraft).
pub fn build_next_data_authority(
    atc_callsign: &str,
    aircraft_callsign: &str,
    aircraft_address: &AcarsEndpointAddress,
    nda_callsign: &str,
) -> OpenLinkMessage {
    MessageBuilder::cpdlc(aircraft_callsign, aircraft_address.to_string())
        .from(atc_callsign)
        .to(aircraft_callsign)
        .next_data_authority(nda_callsign, "")
        .build()
}

/// Build a CPDLC ContactRequest message (ATC → aircraft).
/// Instructs the aircraft to contact a new station.
pub fn build_contact_request(
    atc_callsign: &str,
    aircraft_callsign: &str,
    aircraft_address: &AcarsEndpointAddress,
    next_station: &str,
) -> OpenLinkMessage {
    MessageBuilder::cpdlc(aircraft_callsign, aircraft_address.to_string())
        .from(atc_callsign)
        .to(aircraft_callsign)
        .contact_request(next_station)
        .build()
}

/// Build a CPDLC EndService message (ATC → aircraft).
/// Terminates the active connection and promotes the inactive one.
pub fn build_end_service(
    atc_callsign: &str,
    aircraft_callsign: &str,
    aircraft_address: &AcarsEndpointAddress,
) -> OpenLinkMessage {
    MessageBuilder::cpdlc(aircraft_callsign, aircraft_address.to_string())
        .from(atc_callsign)
        .to(aircraft_callsign)
        .end_service()
        .build()
}

/// Build a CPDLC LogonForward message (ATC → ATC, station-to-station).
/// The current station forwards the flight to a new station.
pub fn build_logon_forward(
    atc_callsign: &str,
    aircraft_callsign: &str,
    aircraft_address: &AcarsEndpointAddress,
    new_station: &str,
) -> OpenLinkMessage {
    MessageBuilder::cpdlc(aircraft_callsign, aircraft_address.to_string())
        .from(atc_callsign)
        .to(new_station)
        .logon_forward(aircraft_callsign, "ZZZZ", "ZZZZ", new_station)
        .build()
}

/// Try to extract the CPDLC meta message from an envelope.
/// Returns (cpdlc_envelope, meta_message, aircraft_acars_address).
pub fn extract_cpdlc_meta(envelope: &OpenLinkEnvelope) -> Option<(&CpdlcEnvelope, &CpdlcMetaMessage, &AcarsEndpointAddress)> {
    if let OpenLinkMessage::Acars(ref acars) = envelope.payload {
        let openlink_models::AcarsMessage::CPDLC(ref cpdlc) = acars.message;
        if let CpdlcMessageType::Meta(ref meta) = cpdlc.message {
            return Some((cpdlc, meta, &acars.routing.aircraft.address));
        }
    }
    None
}

/// Try to extract the CPDLC application message from an envelope.
/// Returns (cpdlc_envelope, application_message, aircraft_acars_address).
pub fn extract_cpdlc_application(envelope: &OpenLinkEnvelope) -> Option<(&CpdlcEnvelope, &CpdlcApplicationMessage, &AcarsEndpointAddress)> {
    if let OpenLinkMessage::Acars(ref acars) = envelope.payload {
        let openlink_models::AcarsMessage::CPDLC(ref cpdlc) = acars.message;
        if let CpdlcMessageType::Application(ref app) = cpdlc.message {
            return Some((cpdlc, app, &acars.routing.aircraft.address));
        }
    }
    None
}

/// Build a CPDLC application message (uplink or downlink).
pub fn build_uplink_message(
    atc_callsign: &str,
    aircraft_callsign: &str,
    aircraft_address: &AcarsEndpointAddress,
    elements: Vec<MessageElement>,
    mrn: Option<u8>,
) -> OpenLinkMessage {
    let builder = MessageBuilder::cpdlc(aircraft_callsign, aircraft_address.to_string())
        .from(atc_callsign)
        .to(aircraft_callsign)
        .application_message_with_mrn(elements, mrn);
    builder.build()
}

/// Build a CPDLC downlink message (aircraft → ground station).
pub fn build_downlink_message(
    aircraft_callsign: &str,
    aircraft_address: &AcarsEndpointAddress,
    station_callsign: &str,
    elements: Vec<MessageElement>,
    mrn: Option<u8>,
) -> OpenLinkMessage {
    let builder = MessageBuilder::cpdlc(aircraft_callsign, aircraft_address.to_string())
        .from(aircraft_callsign)
        .to(station_callsign)
        .application_message_with_mrn(elements, mrn);
    builder.build()
}
