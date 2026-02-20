use openlink_models::{
    AcarsEndpointAddress, CpdlcApplicationMessage, CpdlcEnvelope, CpdlcMessageType,
    CpdlcMetaMessage, NetworkId, OpenLinkEnvelope, OpenLinkMessage,
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
