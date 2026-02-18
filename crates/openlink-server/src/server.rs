//! Core server that subscribes to outbox messages and routes them to destination inboxes.

use anyhow::Result;
use futures::StreamExt;
use openlink_models::{
    AcarsEnvelope, MetaMessage, NetworkId, OpenLinkEnvelope, OpenLinkMessage, OpenLinkRouting,
};
use openlink_sdk::{NatsSubjects, OpenLinkClient};
use tracing::{debug, error, info, warn};

use crate::acars::CPDLCServer;
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
    ) -> Result<Self> {
        let client =
            OpenLinkClient::connect_as_server(nats_url, auth_url, server_secret, &network_id)
                .await
                .map_err(|e| anyhow::anyhow!("SDK connection failed: {e}"))?;

        let js = async_nats::jetstream::new(client.nats_client().clone());

        let station_registry =
            station_registry::StationRegistry::new(network_id.clone(), js.clone()).await?;
        let cpdlc_server = CPDLCServer::new(network_id.clone(), js.clone(), false).await?;

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

            let destination_station = match envelope.payload {
                OpenLinkMessage::Meta(ref meta) => {
                    debug!(?meta, "received meta message");
                    self.handle_meta_message(meta, &envelope).await
                }
                OpenLinkMessage::Acars(ref acars) => {
                    debug!(?acars, "received ACARS message");
                    self.handle_acars_message(acars, &envelope).await
                }
            };

            match destination_station {
                Ok(Some(dest)) => {
                    debug!(?dest, "forwarding to destination station");
                    let mut transferred = envelope.clone();
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
                Ok(None) => {} // no routing needed (e.g. meta-only)
                Err(e) => warn!(error = %e, "handler returned error"),
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
        _envelope: &OpenLinkEnvelope,
    ) -> Result<Option<station_registry::StationEntry>> {
        match acars.message {
            openlink_models::AcarsMessage::CPDLC(ref cpdlc) => {
                debug!(?cpdlc, "routing CPDLC message");
                self.cpdlc_server
                    .handle_cpdlc_message(cpdlc.clone(), acars.clone())
                    .await
            } // future: handle other ACARS types here
        }
    }
}