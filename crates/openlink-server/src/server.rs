//! Core server that subscribes to outbox messages and routes them to destination inboxes.

use anyhow::Result;
use chrono::Duration as ChronoDuration;
use futures::StreamExt;
use std::collections::HashSet;
use std::time::Duration as StdDuration;
use openlink_models::{
    AcarsEndpointCallsign, AcarsEnvelope, MetaMessage, NetworkAddress, NetworkId,
    OpenLinkEnvelope, OpenLinkMessage, OpenLinkRouting, StationStatus,
};
use openlink_sdk::{MessageBuilder, NatsSubjects, OpenLinkClient};
use tracing::{debug, error, info, warn};

use crate::acars::{CPDLCServer, CPDLCSession};
use crate::station_registry;

#[derive(Debug, Clone, Copy)]
pub struct PresenceConfig {
    pub lease_ttl_seconds: i64,
    pub sweep_interval_seconds: u64,
    pub auto_end_service_on_station_offline: bool,
}

impl Default for PresenceConfig {
    fn default() -> Self {
        Self {
            lease_ttl_seconds: 90,
            sweep_interval_seconds: 20,
            auto_end_service_on_station_offline: true,
        }
    }
}

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
    presence_config: PresenceConfig,
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
        presence_config: PresenceConfig,
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
            presence_config,
        })
    }

    /// Subscribe to the network-wide outbox wildcard and route every envelope
    /// to the appropriate handler, then forward the result to the destination
    /// station's inbox.
    pub async fn run(&self) {
        let subject = NatsSubjects::outbox_wildcard(&self.network_id);
        info!(
            network = %self.network_id,
            %subject,
            lease_ttl_seconds = self.presence_config.lease_ttl_seconds,
            sweep_interval_seconds = self.presence_config.sweep_interval_seconds,
            auto_end_service_on_station_offline = self.presence_config.auto_end_service_on_station_offline,
            "server listening"
        );

        let mut subscription = match self.client.subscribe_all_outbox().await {
            Ok(sub) => sub,
            Err(e) => {
                error!(network = %self.network_id, error = %e, "failed to subscribe");
                return;
            }
        };

        let ttl = ChronoDuration::seconds(self.presence_config.lease_ttl_seconds.max(1));
        let mut presence_ticker = tokio::time::interval(StdDuration::from_secs(
            self.presence_config.sweep_interval_seconds.max(1),
        ));

        loop {
            tokio::select! {
                maybe_message = subscription.next() => {
                    let Some(message) = maybe_message else {
                        break;
                    };

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
                _ = presence_ticker.tick() => {
                    match self.station_registry.expire_stale_online(ttl).await {
                        Ok(expired) if !expired.is_empty() => {
                            for entry in expired {
                                info!(network = %self.network_id, station = %entry.station_id, callsign = %entry.acars_endpoint.callsign, "presence lease expired: station marked offline");
                                if let Err(e) = self
                                    .handle_station_offline(
                                        &entry.acars_endpoint.callsign,
                                        format!("presence-expire-{}", entry.station_id),
                                    )
                                    .await
                                {
                                    warn!(
                                        error = %e,
                                        station = %entry.station_id,
                                        callsign = %entry.acars_endpoint.callsign,
                                        "failed to process station offline transition"
                                    );
                                }
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            warn!(network = %self.network_id, error = %e, "presence sweeper failed");
                        }
                    }
                }
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
                    } else if *status == StationStatus::Online {
                        if let Err(e) = self
                            .sync_session_snapshots_for_callsign(
                                address,
                                &acars_endpoint.callsign,
                                root.id.to_string(),
                            )
                            .await
                        {
                            warn!(error = %e, callsign = %acars_endpoint.callsign, "failed to sync session snapshots on station online");
                        }
                    } else if *status == StationStatus::Offline
                        && let Err(e) = self
                            .handle_station_offline(
                                &acars_endpoint.callsign,
                                root.id.to_string(),
                            )
                            .await
                    {
                        warn!(
                            error = %e,
                            callsign = %acars_endpoint.callsign,
                            "failed to process station offline transition"
                        );
                    }
                }
            }
        }
        Ok(None)
    }

    async fn handle_station_offline(
        &self,
        station_callsign: &AcarsEndpointCallsign,
        correlation_id: String,
    ) -> Result<()> {
        let updated_sessions = self
            .cpdlc_server
            .terminate_sessions_for_station(station_callsign)
            .await?;

        for session in updated_sessions {
            let aircraft = &session.aircraft;
            let end_service_envelope = MessageBuilder::envelope(
                MessageBuilder::cpdlc(
                    aircraft.callsign.to_string(),
                    aircraft.address.to_string(),
                )
                .from(station_callsign.to_string())
                .to(aircraft.callsign.to_string())
                .end_service()
                .build(),
            )
            .source_server(self.network_id.as_str())
            .destination_address(self.network_id.as_str(), "aircraft")
            .correlation_id(correlation_id.clone())
            .build();

            if self.presence_config.auto_end_service_on_station_offline {
                if let Ok(Some(aircraft_entry)) = self
                    .station_registry
                    .lookup_callsign(&aircraft.callsign)
                    .await
                {
                    if let Err(e) = self
                        .client
                        .send_to_station(&aircraft_entry.network_address, &end_service_envelope)
                        .await
                    {
                        warn!(
                            error = %e,
                            aircraft = %aircraft.callsign,
                            station = %station_callsign,
                            "failed to send automatic END SERVICE"
                        );
                    }
                }
            }

            self.broadcast_session_update(&session, &end_service_envelope)
                .await;
        }

        Ok(())
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

        // Send a SessionUpdate to relevant ground stations.
        // Include both currently-connected stations and stations that took part
        // in the triggering exchange (e.g. END SERVICE initiator), so ATC can
        // clear UI state even when connection slots are now empty.
        let mut station_callsigns: HashSet<String> = HashSet::new();
        if let Some(ref conn) = session.active_connection {
            station_callsigns.insert(conn.station.callsign.to_string());
        }
        if let Some(ref conn) = session.inactive_connection {
            station_callsigns.insert(conn.station.callsign.to_string());
        }
        if let OpenLinkMessage::Acars(acars_env) = &original_envelope.payload {
            let openlink_models::AcarsMessage::CPDLC(cpdlc) = &acars_env.message;
            if cpdlc.source != aircraft.callsign {
                station_callsigns.insert(cpdlc.source.to_string());
            }
            if cpdlc.destination != aircraft.callsign {
                station_callsigns.insert(cpdlc.destination.to_string());
            }
        }

        for station_callsign in station_callsigns {
            let station_callsign = AcarsEndpointCallsign::new(&station_callsign);
            let station_view = session.to_station_view(&station_callsign);

            let station_msg = MessageBuilder::cpdlc(
                aircraft.callsign.to_string(),
                aircraft.address.to_string(),
            )
                .from("SERVER")
                .to(station_callsign.to_string())
                .session_update(station_view)
                .build();

            let station_envelope = MessageBuilder::envelope(station_msg)
                .source_server(self.network_id.as_str())
                .destination_address(self.network_id.as_str(), "station")
                .correlation_id(original_envelope.id.to_string())
                .build();

            if let Ok(Some(station_entry)) = self
                .station_registry
                .lookup_callsign(&station_callsign)
                .await
            {
                if let Err(e) = self
                    .client
                    .send_to_station(&station_entry.network_address, &station_envelope)
                    .await
                {
                    error!(error = %e, station = %station_callsign, "failed to send SessionUpdate to station");
                } else {
                    debug!(station = %station_callsign, "sent SessionUpdate to station");
                }
            }
        }
    }

    /// Replay all current session snapshots relevant to a participant callsign.
    async fn sync_session_snapshots_for_callsign(
        &self,
        network_address: &NetworkAddress,
        callsign: &AcarsEndpointCallsign,
        correlation_id: String,
    ) -> Result<()> {
        let sessions = self
            .cpdlc_server
            .list_sessions_for_callsign(callsign)
            .await?;

        for session in sessions {
            let view = if session.aircraft.callsign == *callsign {
                session.to_aircraft_view()
            } else {
                session.to_station_view(callsign)
            };

            let msg = MessageBuilder::cpdlc(
                session.aircraft.callsign.to_string(),
                session.aircraft.address.to_string(),
            )
            .from("SERVER")
            .to(callsign.to_string())
            .session_update(view)
            .build();

            let envelope = MessageBuilder::envelope(msg)
                .source_server(self.network_id.as_str())
                .destination_address(self.network_id.as_str(), network_address.as_str())
                .correlation_id(correlation_id.clone())
                .build();

            self.client.send_to_station(network_address, &envelope).await?;
        }

        Ok(())
    }
}