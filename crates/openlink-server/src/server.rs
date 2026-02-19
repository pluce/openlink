//! Core server that subscribes to outbox messages and routes them to destination inboxes.

use anyhow::Result;
use futures::StreamExt;
use openlink_models::{
    AcarsEnvelope, MetaMessage, NetworkId, OpenLinkEnvelope, OpenLinkMessage,
    OpenLinkRouting,
};
use openlink_sdk::{MessageBuilder, NatsSubjects, OpenLinkClient};
use tracing::{debug, error, info, warn};

use crate::acars::{CPDLCServer, CPDLCSession};
use crate::station_registry;

/// The main server that listens for outbound messages on a single network
/// and routes them to the correct destination inbox.
///
/// Connects to NATS via the SDK with a **server-level** JWT granting
/// wildcard access to all outbox/inbox subjects and JetStream KV stores.
pub struct OpenLinkServer {
    network_id: NetworkId,
    client: OpenLinkClient,
    cpdlc_server: CPDLCServer,
    station_registry: station_registry::StationRegistry,
}

impl OpenLinkServer {
    /// Create a new server for the given network.
    ///
    /// Connects to NATS via the SDK using a server secret, obtaining
    /// wildcard permissions for all outbox/inbox subjects and JetStream.
    pub async fn new(
        network_id: NetworkId,
        nats_url: &str,
        auth_url: &str,
        server_secret: &str,
        clean: bool,
    ) -> Result<Self> {
        let client =
            OpenLinkClient::connect_as_server(nats_url, auth_url, server_secret, &network_id)
                .await
                .map_err(|e| anyhow::anyhow!("SDK connection failed: {e}"))?;

        let js = async_nats::jetstream::new(client.nats_client().clone());

        let station_registry =
            station_registry::StationRegistry::new(network_id.clone(), js.clone(), clean).await?;
        let cpdlc_server = CPDLCServer::new(network_id.clone(), js.clone(), clean).await?;

        Ok(Self {
            network_id,
            client,
            cpdlc_server,
            station_registry,
        })
    }

    /// Subscribe to the network-wide outbox wildcard and route every envelope
    /// to the appropriate handler, then forward the result to the destination
    /// station's inbox.
    pub async fn run(&self) {
        let subject = NatsSubjects::outbox_wildcard(&self.network_id);
        info!(network = %self.network_id, %subject, "server listening");

        let mut subscription = match self.client.subscribe_all_outbox().await {
            Ok(sub) => sub,
            Err(e) => {
                error!(network = %self.network_id, error = %e, "failed to subscribe");
                return;
            }
        };

        while let Some(message) = subscription.next().await {
            let envelope = match serde_json::from_slice::<OpenLinkEnvelope>(&message.payload) {
                Ok(env) => env,
                Err(e) => {
                    warn!(error = %e, "ignoring malformed envelope");
                    continue;
                }
            };

            let (destination_station, maybe_session, forward_envelope) = match envelope.payload {
                OpenLinkMessage::Meta(ref meta) => {
                    debug!(?meta, "received meta message");
                    let result = self.handle_meta_message(meta, &envelope).await;
                    match result {
                        Ok(dest) => (dest, None, envelope.clone()),
                        Err(e) => {
                            warn!(error = %e, "handler returned error");
                            continue;
                        }
                    }
                }
                OpenLinkMessage::Acars(ref acars) => {
                    debug!(?acars, "received ACARS message");
                    match self.handle_acars_message(acars, &envelope).await {
                        Ok((dest, session, modified_env)) => (dest, session, modified_env),
                        Err(e) => {
                            warn!(error = %e, "handler returned error");
                            continue;
                        }
                    }
                }
            };

            // Forward the (possibly modified) message to the destination station
            if let Some(ref dest) = destination_station {
                debug!(?dest, "forwarding to destination station");
                let mut transferred = forward_envelope;
                transferred.routing = OpenLinkRouting {
                    source: envelope.routing.destination.clone(),
                    destination: openlink_models::OpenLinkRoutingEndpoint::Address(
                        self.network_id.clone(),
                        dest.network_address.clone(),
                    ),
                };
                if let Err(e) = self
                    .client
                    .send_to_station(&dest.network_address, &transferred)
                    .await
                {
                    error!(error = %e, "failed to forward message");
                }
            }

            // Broadcast SessionUpdate to both parties if session was mutated
            if let Some(ref session) = maybe_session {
                self.broadcast_session_update(session, &envelope).await;
            }
        }
    }

    /// Handle station meta messages (status updates, etc.).
    async fn handle_meta_message(
        &self,
        meta: &MetaMessage,
        root: &OpenLinkEnvelope,
    ) -> Result<Option<station_registry::StationEntry>> {
        match meta {
            MetaMessage::StationStatus(station_id, status, acars_endpoint) => {
                info!(station = %station_id, ?status, "station status update");
                if let openlink_models::OpenLinkRoutingEndpoint::Address(_network, address) =
                    &root.routing.source
                {
                    if let Err(e) = self
                        .station_registry
                        .update_status(station_id, status, acars_endpoint, address)
                        .await
                    {
                        error!(error = %e, "failed to update station status");
                    }
                }
            }
        }
        Ok(None)
    }

    /// Route ACARS envelopes (currently only CPDLC) to the appropriate sub-handler.
    async fn handle_acars_message(
        &self,
        acars: &AcarsEnvelope,
        envelope: &OpenLinkEnvelope,
    ) -> Result<(Option<station_registry::StationEntry>, Option<CPDLCSession>, OpenLinkEnvelope)> {
        match acars.message {
            openlink_models::AcarsMessage::CPDLC(ref cpdlc) => {
                debug!(?cpdlc, "routing CPDLC message");
                let (dest_callsign, session, modified_envelope) = self
                    .cpdlc_server
                    .handle_cpdlc_message(cpdlc.clone(), acars.clone(), envelope)
                    .await?;
                // Resolve the destination callsign to a station entry for routing.
                let dest = self
                    .station_registry
                    .lookup_callsign(&dest_callsign)
                    .await
                    .ok()
                    .flatten();
                Ok((dest, session, modified_envelope))
            } // future: handle other ACARS types here
        }
    }

    /// Broadcast a `SessionUpdate` to both the aircraft and each connected
    /// ground station after a session-mutating meta-message.
    async fn broadcast_session_update(
        &self,
        session: &CPDLCSession,
        original_envelope: &OpenLinkEnvelope,
    ) {
        let aircraft = &session.aircraft;
        let aircraft_view = session.to_aircraft_view();

        // Build the SessionUpdate message for the aircraft
        let aircraft_msg = MessageBuilder::cpdlc(
            aircraft.callsign.to_string(),
            aircraft.address.to_string(),
        )
            .from("SERVER")
            .to(aircraft.callsign.to_string())
            .session_update(aircraft_view)
            .build();

        let aircraft_envelope = MessageBuilder::envelope(aircraft_msg)
            .source_server(self.network_id.as_str())
            .destination_address(self.network_id.as_str(), "aircraft")
            .correlation_id(original_envelope.id.to_string())
            .build();

        // Look up the aircraft's network address via the station registry
        if let Ok(Some(aircraft_entry)) = self
            .station_registry
            .lookup_callsign(&aircraft.callsign)
            .await
        {
            if let Err(e) = self
                .client
                .send_to_station(&aircraft_entry.network_address, &aircraft_envelope)
                .await
            {
                error!(error = %e, "failed to send SessionUpdate to aircraft");
            } else {
                debug!(callsign = %aircraft.callsign, "sent SessionUpdate to aircraft");
            }
        } else {
            debug!(callsign = %aircraft.callsign, "aircraft not found in registry, skipping SessionUpdate");
        }

        // Send a SessionUpdate to each connected ground station
        let mut station_endpoints: Vec<openlink_models::AcarsRoutingEndpoint> = Vec::new();
        if let Some(ref conn) = session.active_connection {
            station_endpoints.push(conn.station.clone());
        }
        if let Some(ref conn) = session.inactive_connection {
            station_endpoints.push(conn.station.clone());
        }

        for station_endpoint in station_endpoints {
            let station_view = session.to_station_view(&station_endpoint.callsign);

            let station_msg = MessageBuilder::cpdlc(
                aircraft.callsign.to_string(),
                aircraft.address.to_string(),
            )
                .from("SERVER")
                .to(station_endpoint.callsign.to_string())
                .session_update(station_view)
                .build();

            let station_envelope = MessageBuilder::envelope(station_msg)
                .source_server(self.network_id.as_str())
                .destination_address(self.network_id.as_str(), "station")
                .correlation_id(original_envelope.id.to_string())
                .build();

            if let Ok(Some(station_entry)) = self
                .station_registry
                .lookup_callsign(&station_endpoint.callsign)
                .await
            {
                if let Err(e) = self
                    .client
                    .send_to_station(&station_entry.network_address, &station_envelope)
                    .await
                {
                    error!(error = %e, station = %station_endpoint.callsign, "failed to send SessionUpdate to station");
                } else {
                    debug!(station = %station_endpoint.callsign, "sent SessionUpdate to station");
                }
            }
        }
    }
}