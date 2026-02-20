//! High-level NATS client for the OpenLink network.
//!
//! [`OpenLinkClient`] handles authentication, connection, and message
//! routing on behalf of a single participant (aircraft or ground station).
//!
//! # Typical usage
//!
//! ```rust,no_run
//! use openlink_models::NetworkId;
//! use openlink_sdk::OpenLinkClient;
//!
//! # async fn run() -> Result<(), openlink_sdk::SdkError> {
//! let network = NetworkId::new("demonetwork");
//! let client = OpenLinkClient::connect_with_authorization_code(
//!     "nats://localhost:4222",
//!     "http://localhost:3001",
//!     "my-oidc-code",
//!     &network,
//! ).await?;
//!
//! println!("Connected as CID {}", client.cid());
//! # Ok(())
//! # }
//! ```

use async_nats::ConnectOptions;
use nkeys::KeyPair;
use openlink_models::{
    AcarsEndpointAddress, MessageBuilder, MessageElement, NetworkAddress, NetworkId,
    OpenLinkEnvelope, OpenLinkMessage,
};

use crate::credentials::OpenLinkCredentials;
use crate::error::SdkError;
use crate::subjects::NatsSubjects;

/// A connected OpenLink participant.
///
/// Wraps the underlying NATS connection and exposes typed methods to
/// publish and subscribe to the correct subjects.
///
/// A client can represent a **station / aircraft** (connected via
/// [`connect_with_authorization_code`](Self::connect_with_authorization_code))
/// or a **server** (connected via [`connect_as_server`](Self::connect_as_server))
/// with wildcard permissions.
#[derive(Clone)]
pub struct OpenLinkClient {
    nats_client: async_nats::Client,
    creds: OpenLinkCredentials,
    network: NetworkId,
    address: NetworkAddress,
}

impl OpenLinkClient {
    // ------------------------------------------------------------------
    // Connection
    // ------------------------------------------------------------------

    /// Authenticate via an OIDC authorization code, then connect to NATS.
    ///
    /// This is the recommended entry-point for most integrations.
    ///
    /// 1. Generates an ephemeral NKey pair.
    /// 2. Exchanges the code + public key for a signed NATS JWT.
    /// 3. Connects to the NATS server using JWT + NKey challenge.
    pub async fn connect_with_authorization_code(
        nats_url: &str,
        auth_url: &str,
        authorization_code: &str,
        network: &NetworkId,
    ) -> Result<Self, SdkError> {
        // 1. Generate ephemeral user key-pair
        let user_kp = KeyPair::new(nkeys::KeyPairType::User);
        let seed = user_kp
            .seed()
            .map_err(|e| SdkError::Config(e.to_string()))?;
        let public_key = user_kp.public_key();

        // 2. Exchange authorization code for NATS JWT
        let http = reqwest::Client::new();
        let res = http
            .post(format!("{auth_url}/exchange"))
            .json(&serde_json::json!({
                "oidc_code": authorization_code,
                "user_nkey_public": public_key,
                "network": network.as_str(),
            }))
            .send()
            .await?;

        if !res.status().is_success() {
            let text = res.text().await?;
            return Err(SdkError::Auth(text));
        }

        let body: serde_json::Value = res.json().await?;
        let jwt = body["jwt"]
            .as_str()
            .ok_or_else(|| SdkError::Auth("missing `jwt` in auth response".into()))?
            .to_string();
        let cid = body["cid"]
            .as_str()
            .ok_or_else(|| SdkError::Auth("missing `cid` in auth response".into()))?
            .to_string();

        let creds = OpenLinkCredentials { seed, jwt, cid };

        // 3. Connect
        Self::connect(nats_url, creds, network).await
    }

    /// Connect to NATS using pre-existing credentials.
    ///
    /// Supports both TCP (`nats://`) and WebSocket (`ws://`, `wss://`).
    pub async fn connect(
        nats_url: &str,
        creds: OpenLinkCredentials,
        network: &NetworkId,
    ) -> Result<Self, SdkError> {
        // Sanity-check the seed
        let _ = KeyPair::from_seed(&creds.seed)
            .map_err(|e| SdkError::Config(format!("invalid NKey seed: {e}")))?;

        let jwt = creds.jwt.clone();
        let seed_for_sign = creds.seed.clone();
        let address = NetworkAddress::new(&creds.cid);

        let options = ConnectOptions::with_jwt(jwt, move |nonce| {
            let seed = seed_for_sign.clone();
            async move {
                let kp = KeyPair::from_seed(&seed).map_err(async_nats::AuthError::new)?;
                kp.sign(&nonce).map_err(async_nats::AuthError::new)
            }
        });

        let nats_client = async_nats::connect_with_options(nats_url, options).await?;

        Ok(Self {
            nats_client,
            creds,
            network: network.clone(),
            address,
        })
    }

    /// Connect to NATS as an **OpenLink server** with wildcard permissions.
    ///
    /// 1. Generates an ephemeral NKey pair.
    /// 2. Exchanges the server secret for a master NATS JWT via
    ///    `POST /exchange-server`.
    /// 3. Connects to NATS with JWT + NKey challenge.
    ///
    /// The returned client has publish access to all inboxes and subscribe
    /// access to all outboxes on the given `network`, as well as JetStream
    /// KV access.
    pub async fn connect_as_server(
        nats_url: &str,
        auth_url: &str,
        server_secret: &str,
        network: &NetworkId,
    ) -> Result<Self, SdkError> {
        // 1. Generate ephemeral user key-pair
        let user_kp = KeyPair::new(nkeys::KeyPairType::User);
        let seed = user_kp
            .seed()
            .map_err(|e| SdkError::Config(e.to_string()))?;
        let public_key = user_kp.public_key();

        // 2. Exchange server secret for a master NATS JWT
        let http = reqwest::Client::new();
        let res = http
            .post(format!("{auth_url}/exchange-server"))
            .json(&serde_json::json!({
                "server_secret": server_secret,
                "user_nkey_public": public_key,
                "network": network.as_str(),
            }))
            .send()
            .await?;

        if !res.status().is_success() {
            let text = res.text().await?;
            return Err(SdkError::Auth(text));
        }

        let body: serde_json::Value = res.json().await?;
        let jwt = body["jwt"]
            .as_str()
            .ok_or_else(|| SdkError::Auth("missing `jwt` in auth response".into()))?
            .to_string();

        let server_name = format!("openlink-server-{network}");
        let creds = OpenLinkCredentials {
            seed,
            jwt,
            cid: server_name.clone(),
        };
        let address = NetworkAddress::new(&server_name);

        // 3. Connect
        let jwt_for_connect = creds.jwt.clone();
        let seed_for_sign = creds.seed.clone();

        let options = ConnectOptions::with_jwt(jwt_for_connect, move |nonce| {
            let seed = seed_for_sign.clone();
            async move {
                let kp = KeyPair::from_seed(&seed).map_err(async_nats::AuthError::new)?;
                kp.sign(&nonce).map_err(async_nats::AuthError::new)
            }
        });

        let nats_client = async_nats::connect_with_options(nats_url, options).await?;

        Ok(Self {
            nats_client,
            creds,
            network: network.clone(),
            address,
        })
    }

    // ------------------------------------------------------------------
    // Publishing
    // ------------------------------------------------------------------

    /// Send a message to the OpenLink server.
    ///
    /// The message is wrapped in an [`OpenLinkEnvelope`] with routing
    /// set from this client's address to the network server, then
    /// published on the client's **outbox** subject.
    pub async fn send_to_server(&self, msg: OpenLinkMessage) -> Result<(), SdkError> {
        let envelope = MessageBuilder::envelope(msg)
            .source_address(self.network.as_str(), self.creds.cid.as_str())
            .destination_server(self.network.as_str())
            .build();

        let subject = NatsSubjects::outbox(&self.network, &self.address);
        self.publish_envelope(&subject, &envelope).await
    }

    // ------------------------------------------------------------------
    // High-level CPDLC helpers
    // ------------------------------------------------------------------

    /// Build an aircraft → station CPDLC logon request.
    pub fn cpdlc_logon_request(
        &self,
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

    /// Build an ATC → aircraft CPDLC logon response.
    pub fn cpdlc_logon_response(
        &self,
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

    /// Build an ATC → aircraft CPDLC connection request.
    pub fn cpdlc_connection_request(
        &self,
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

    /// Build an aircraft → ATC CPDLC connection response.
    pub fn cpdlc_connection_response(
        &self,
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

    /// Build an ATC → aircraft Next Data Authority CPDLC message.
    pub fn cpdlc_next_data_authority(
        &self,
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

    /// Build an ATC → aircraft CPDLC contact request.
    pub fn cpdlc_contact_request(
        &self,
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

    /// Build an ATC → aircraft CPDLC end-service message.
    pub fn cpdlc_end_service(
        &self,
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

    /// Build an ATC → ATC CPDLC logon-forward message.
    pub fn cpdlc_logon_forward(
        &self,
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

    /// Build a station-originated CPDLC application message.
    pub fn cpdlc_station_application(
        &self,
        station_callsign: &str,
        aircraft_callsign: &str,
        aircraft_address: &AcarsEndpointAddress,
        elements: Vec<MessageElement>,
        mrn: Option<u8>,
    ) -> OpenLinkMessage {
        MessageBuilder::cpdlc(aircraft_callsign, aircraft_address.to_string())
            .from(station_callsign)
            .to(aircraft_callsign)
            .application_message_with_mrn(elements, mrn)
            .build()
    }

    /// Build an aircraft-originated CPDLC application message.
    pub fn cpdlc_aircraft_application(
        &self,
        aircraft_callsign: &str,
        aircraft_address: &AcarsEndpointAddress,
        station_callsign: &str,
        elements: Vec<MessageElement>,
        mrn: Option<u8>,
    ) -> OpenLinkMessage {
        MessageBuilder::cpdlc(aircraft_callsign, aircraft_address.to_string())
            .from(aircraft_callsign)
            .to(station_callsign)
            .application_message_with_mrn(elements, mrn)
            .build()
    }

    /// Publish an envelope directly to a station's **inbox**.
    ///
    /// This is used by the server (or by a station acting as relay)
    /// to deliver a message to a specific recipient.
    pub async fn send_to_station(
        &self,
        station: &NetworkAddress,
        envelope: &OpenLinkEnvelope,
    ) -> Result<(), SdkError> {
        let subject = NatsSubjects::inbox(&self.network, station);
        self.publish_envelope(&subject, envelope).await
    }

    /// Low-level: serialize an envelope and publish it on a raw subject.
    pub async fn publish_envelope(
        &self,
        subject: &str,
        envelope: &OpenLinkEnvelope,
    ) -> Result<(), SdkError> {
        let bytes = serde_json::to_vec(envelope)?;
        self.nats_client
            .publish(subject.to_string(), bytes.into())
            .await?;
        self.nats_client
            .flush()
            .await
            .map_err(|e| SdkError::Nats(e.to_string()))?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Subscribing
    // ------------------------------------------------------------------

    /// Subscribe to this client's **inbox** subject.
    ///
    /// Returns a stream of raw NATS messages that can be deserialized
    /// into [`OpenLinkEnvelope`].
    pub async fn subscribe_inbox(&self) -> Result<async_nats::Subscriber, SdkError> {
        let subject = NatsSubjects::inbox(&self.network, &self.address);
        let sub = self.nats_client.subscribe(subject).await?;
        Ok(sub)
    }

    /// Subscribe to the **outbox wildcard** subject for this network.
    ///
    /// This receives every message published by any client on the network.
    /// Intended for server-mode connections obtained via
    /// [`connect_as_server`](Self::connect_as_server).
    pub async fn subscribe_all_outbox(&self) -> Result<async_nats::Subscriber, SdkError> {
        let subject = NatsSubjects::outbox_wildcard(&self.network);
        let sub = self.nats_client.subscribe(subject).await?;
        Ok(sub)
    }

    // ------------------------------------------------------------------
    // Accessors
    // ------------------------------------------------------------------

    /// The network this client is connected to.
    pub fn network(&self) -> &NetworkId {
        &self.network
    }

    /// The network address assigned to this client (derived from CID).
    pub fn address(&self) -> &NetworkAddress {
        &self.address
    }

    /// The connection identifier (CID) from the auth service.
    pub fn cid(&self) -> &str {
        &self.creds.cid
    }

    /// The underlying credentials.
    pub fn credentials(&self) -> &OpenLinkCredentials {
        &self.creds
    }

    /// Access the raw NATS client for advanced operations.
    ///
    /// Use this when you need JetStream or other low-level NATS features
    /// that the SDK does not wrap directly.
    pub fn nats_client(&self) -> &async_nats::Client {
        &self.nats_client
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlink_models::NetworkId;

    #[test]
    fn address_derived_from_cid() {
        // Verify that the address would be derived from cid
        let cid = "12345";
        let address = NetworkAddress::new(cid);
        assert_eq!(address.as_str(), cid);
    }

    #[test]
    fn accessors_are_consistent() {
        // This tests the type contracts without a live NATS connection
        let network = NetworkId::new("demonetwork");
        let address = NetworkAddress::new("12345");
        assert_eq!(network.as_str(), "demonetwork");
        assert_eq!(address.as_str(), "12345");
    }
}
