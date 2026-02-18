use openlink_models::{
    CpdlcEnvelope, CpdlcMessageType, CpdlcMetaMessage, NetworkId,
    OpenLinkEnvelope, OpenLinkMessage,
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
    aircraft_address: &str,
    target_station: &str,
) -> OpenLinkMessage {
    MessageBuilder::cpdlc(aircraft_callsign, aircraft_address)
        .from(aircraft_callsign)
        .to(target_station)
        .logon_request(target_station, "ZZZZ", "ZZZZ")
        .build()
}

/// Build a CPDLC logon response message (ATC → aircraft).
pub fn build_logon_response(
    atc_callsign: &str,
    aircraft_callsign: &str,
    aircraft_address: &str,
    accepted: bool,
) -> OpenLinkMessage {
    MessageBuilder::cpdlc(aircraft_callsign, aircraft_address)
        .from(atc_callsign)
        .to(aircraft_callsign)
        .logon_response(accepted)
        .build()
}

/// Build a CPDLC connection request message (ATC → aircraft).
pub fn build_connection_request(
    atc_callsign: &str,
    aircraft_callsign: &str,
    aircraft_address: &str,
) -> OpenLinkMessage {
    MessageBuilder::cpdlc(aircraft_callsign, aircraft_address)
        .from(atc_callsign)
        .to(aircraft_callsign)
        .connection_request()
        .build()
}

/// Build a CPDLC connection response message (aircraft → ATC).
pub fn build_connection_response(
    aircraft_callsign: &str,
    aircraft_address: &str,
    atc_callsign: &str,
    accepted: bool,
) -> OpenLinkMessage {
    MessageBuilder::cpdlc(aircraft_callsign, aircraft_address)
        .from(aircraft_callsign)
        .to(atc_callsign)
        .connection_response(accepted)
        .build()
}

/// Try to extract the CPDLC meta message from an envelope.
pub fn extract_cpdlc_meta(envelope: &OpenLinkEnvelope) -> Option<(&CpdlcEnvelope, &CpdlcMetaMessage)> {
    if let OpenLinkMessage::Acars(ref acars) = envelope.payload {
        let openlink_models::AcarsMessage::CPDLC(ref cpdlc) = acars.message;
        if let CpdlcMessageType::Meta(ref meta) = cpdlc.message {
            return Some((cpdlc, meta));
        }
    }
    None
}
