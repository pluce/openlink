//! Bridge orchestration: runs the Hoppie polling loop and the OpenLink NATS
//! subscription concurrently, relaying messages between the two systems.

use std::collections::HashSet;

use anyhow::{Context, Result};
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn};

use openlink_models::{
    AcarsEndpointAddress, CpdlcMessageType, CpdlcMetaMessage, MessageDirection, NetworkId,
    OpenLinkEnvelope, OpenLinkMessage,
};
use openlink_sdk::OpenLinkClient;

use crate::config::{BridgeConfig, BridgeMode};
use crate::hoppie_client::{HoppieClient, HoppieMessageType};
use crate::session::{hoppie_message_hash, SessionKey, SessionTracker};
use crate::translator;

/// Run the bridge until cancelled.
pub async fn run_bridge(config: BridgeConfig) -> Result<()> {
    info!(
        mode = %config.mode,
        callsigns = ?config.callsigns,
        network = %config.network_id,
        "starting Hoppie ↔ OpenLink bridge"
    );

    // Connect to OpenLink
    let network = NetworkId::new(&config.network_id);
    let client = OpenLinkClient::connect_with_authorization_code(
        &config.nats_url,
        &config.auth_url,
        &config.auth_code,
        &network,
    )
    .await
    .context("failed to connect to OpenLink")?;

    info!("connected to OpenLink network: {}", config.network_id);

    // Create Hoppie client
    let hoppie = HoppieClient::new(&config.hoppie_url, &config.hoppie_logon);

    // Session tracker (shared mutable state — single-threaded tokio)
    let mut tracker = SessionTracker::new();

    // Callsigns managed by this bridge (Hoppie-side identities)
    let bridge_callsigns: HashSet<String> = config
        .callsigns
        .iter()
        .map(|c| c.to_uppercase())
        .collect();

    // Subscribe to OpenLink inbox
    let mut inbox = client
        .subscribe_inbox()
        .await
        .context("failed to subscribe to OpenLink inbox")?;

    info!("subscribed to OpenLink inbox, starting relay loops");

    let poll_interval = Duration::from_secs(config.poll_interval_secs);
    let mut poll_timer = interval(poll_interval);

    loop {
        tokio::select! {
            // ── Hoppie → OpenLink (polling) ─────────────────────
            _ = poll_timer.tick() => {
                if config.mode == BridgeMode::Ground || config.mode == BridgeMode::Full {
                    // Poll each bridge callsign on Hoppie
                    for cs in &bridge_callsigns {
                        match hoppie.poll(cs).await {
                            Ok(messages) => {
                                for msg in messages {
                                    if let Err(e) = handle_hoppie_message(
                                        &msg,
                                        cs,
                                        &client,
                                        &mut tracker,
                                        &bridge_callsigns,
                                    ).await {
                                        warn!(error = %e, from = %msg.from, "failed to relay Hoppie→OpenLink");
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(error = %e, callsign = %cs, "Hoppie poll failed");
                            }
                        }
                    }
                }
            }

            // ── OpenLink → Hoppie (NATS subscription) ───────────
            Some(nats_msg) = async { futures::StreamExt::next(&mut inbox).await } => {
                if config.mode == BridgeMode::Aircraft || config.mode == BridgeMode::Full {
                    match serde_json::from_slice::<OpenLinkEnvelope>(&nats_msg.payload) {
                        Ok(envelope) => {
                            if let Err(e) = handle_openlink_message(
                                &envelope,
                                &hoppie,
                                &client,
                                &mut tracker,
                                &bridge_callsigns,
                            ).await {
                                warn!(error = %e, id = %envelope.id, "failed to relay OpenLink→Hoppie");
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "failed to deserialize OpenLink envelope");
                        }
                    }
                }
            }
        }
    }
}

/// Handle an incoming Hoppie message and relay to OpenLink.
async fn handle_hoppie_message(
    msg: &crate::hoppie_client::HoppieMessage,
    polled_as: &str,
    client: &OpenLinkClient,
    tracker: &mut SessionTracker,
    bridge_callsigns: &HashSet<String>,
) -> Result<()> {
    // Only handle CPDLC messages
    if msg.msg_type != HoppieMessageType::Cpdlc {
        debug!(from = %msg.from, msg_type = ?msg.msg_type, "ignoring non-CPDLC Hoppie message");
        return Ok(());
    }

    // Dedup
    let hash = hoppie_message_hash(&msg.from, &msg.packet);
    if tracker.is_hoppie_seen(hash) {
        debug!("skipping duplicate Hoppie message");
        return Ok(());
    }
    tracker.mark_hoppie_seen(hash);

    // Skip echoes from our own callsigns
    if bridge_callsigns.contains(&msg.from.to_uppercase()) {
        return Ok(());
    }

    // When polling as a ground station, the external sender is the aircraft
    // and our polled callsign is the station.
    let aircraft_cs = msg.from.as_str();
    let station_cs = polled_as;
    let aircraft_address = AcarsEndpointAddress::new(aircraft_cs);

    // Parse the CPDLC packet
    let pkt = crate::hoppie_client::parse_cpdlc_packet(&msg.packet)
        .context("failed to parse CPDLC packet")?;

    // ── Hoppie LOGON ("REQUEST LOGON" body) → OpenLink LogonRequest ──
    if translator::is_logon_body(&pkt.body) {
        info!(aircraft = %aircraft_cs, station = %station_cs, "Hoppie logon request → OpenLink LogonRequest");

        // Register the Hoppie aircraft on OpenLink so the server can route
        // messages (LogonResponse, Application msgs) to the bridge's inbox.
        let registration = openlink_models::MessageBuilder::station_status(
            client.cid(),      // bridge's network address (separate from GUI)
            aircraft_cs,       // aircraft callsign
            aircraft_cs,       // use callsign as ACARS address
        )
        .online()
        .build();
        client.send_to_server(registration).await
            .context("failed to register Hoppie aircraft on OpenLink")?;
        info!(aircraft = %aircraft_cs, "registered Hoppie aircraft in station registry");

        let openlink_msg = client.cpdlc_logon_request(aircraft_cs, &aircraft_address, station_cs);
        client.send_to_server(openlink_msg).await
            .context("failed to relay logon request to OpenLink")?;
        return Ok(());
    }

    // ── Regular CPDLC application message ──
    // When polling as a ground station, incoming messages from aircraft are
    // always Downlinks. Don't rely on guess_direction_from_registry which
    // tries UM first and gets it wrong.
    let direction = MessageDirection::Downlink;

    let session_key = SessionKey::new(aircraft_cs, station_cs);
    let bridge_min = tracker.next_min(&session_key);

    let mut env = translator::hoppie_to_openlink(aircraft_cs, station_cs, &msg.packet, bridge_min, direction)?;

    if let CpdlcMessageType::Application(ref mut app) = env.message {
        if let Some(mrn) = app.mrn {
            if let Some(mapped) = tracker.translate_hoppie_mrn(&session_key, mrn) {
                app.mrn = Some(mapped);
            }
        }
        tracker.record_hoppie_min(&session_key, bridge_min, bridge_min);
    }

    let openlink_msg = openlink_models::MessageBuilder::cpdlc(aircraft_cs, aircraft_address.to_string())
        .from(env.source.to_string())
        .to(env.destination.to_string())
        .raw_message(env.message)
        .build();

    client
        .send_to_server(openlink_msg)
        .await
        .context("failed to send to OpenLink")?;

    info!(
        from = %aircraft_cs,
        to = %station_cs,
        direction = %direction,
        "relayed Hoppie→OpenLink"
    );

    Ok(())
}

/// Handle an incoming OpenLink message and relay to Hoppie.
async fn handle_openlink_message(
    envelope: &OpenLinkEnvelope,
    hoppie: &HoppieClient,
    client: &OpenLinkClient,
    tracker: &mut SessionTracker,
    bridge_callsigns: &HashSet<String>,
) -> Result<()> {
    // Dedup
    let msg_id = envelope.id.to_string();
    if tracker.is_openlink_seen(&msg_id) {
        debug!("skipping duplicate OpenLink message");
        return Ok(());
    }
    tracker.mark_openlink_seen(&msg_id);

    // Extract CPDLC envelope from the payload
    let cpdlc_env = match &envelope.payload {
        OpenLinkMessage::Acars(acars) => match &acars.message {
            openlink_models::AcarsMessage::CPDLC(cpdlc) => cpdlc,
        },
        OpenLinkMessage::Meta(_) => {
            debug!("ignoring Meta message from OpenLink");
            return Ok(());
        }
    };

    let dest = cpdlc_env.destination.to_string().to_uppercase();
    let source = cpdlc_env.source.to_string().to_uppercase();

    // Skip echoes: if the envelope was sent from the bridge's own network
    // address, it's our own message bouncing back.  We must NOT filter on
    // the CPDLC source callsign because the GUI shares the same station
    // callsign (e.g. "LFXB") as the bridge.
    let is_own_message = match &envelope.routing.source {
        openlink_models::OpenLinkRoutingEndpoint::Address(_, addr) => {
            addr.to_string() == client.cid()
        }
        _ => false,
    };
    if is_own_message {
        debug!("skipping echo from bridge's own CID");
        return Ok(());
    }

    // ── Handle Meta messages (LogonResponse → auto-connect + relay) ──
    if let CpdlcMessageType::Meta(ref meta) = cpdlc_env.message {
        return handle_openlink_meta(meta, &source, &dest, hoppie, client, bridge_callsigns).await;
    }

    // ── Regular Application message → Hoppie ──
    let out = match translator::openlink_to_hoppie(cpdlc_env) {
        Some(out) => out,
        None => {
            debug!("skipping non-translatable CPDLC message");
            return Ok(());
        }
    };

    if let CpdlcMessageType::Application(app) = &cpdlc_env.message {
        let (aircraft_cs, station_cs) = determine_aircraft_station(
            &source,
            &dest,
            bridge_callsigns,
        );
        let session_key = SessionKey::new(&aircraft_cs, &station_cs);
        tracker.record_openlink_min(&session_key, app.min, app.min);
    }

    hoppie
        .send_cpdlc(&out.from, &out.peer, &out.packet)
        .await
        .context("failed to send to Hoppie")?;

    info!(
        from = %source,
        to = %dest,
        "relayed OpenLink→Hoppie"
    );

    Ok(())
}

/// Handle an OpenLink meta message.
///
/// When the server routes a LogonResponse to the bridge (because the Hoppie
/// aircraft is registered with the bridge's CID), we:
/// 1. Auto-inject ConnectionRequest + ConnectionResponse to establish the CPDLC session
/// 2. Relay the logon acceptance to the Hoppie aircraft
async fn handle_openlink_meta(
    meta: &CpdlcMetaMessage,
    source: &str,
    dest: &str,
    hoppie: &HoppieClient,
    client: &OpenLinkClient,
    bridge_callsigns: &HashSet<String>,
) -> Result<()> {
    match meta {
        CpdlcMetaMessage::LogonResponse { accepted } => {
            // source = station (ATC), dest = aircraft (Hoppie)
            let station_cs = source;
            let aircraft_cs = dest;
            let aircraft_address = AcarsEndpointAddress::new(aircraft_cs);

            if *accepted {
                info!(
                    station = %station_cs,
                    aircraft = %aircraft_cs,
                    "LogonResponse(accepted) → auto-connect + relay to Hoppie"
                );

                // 1. Auto-inject ConnectionRequest (station → aircraft)
                let msg = client.cpdlc_connection_request(station_cs, aircraft_cs, &aircraft_address);
                client.send_to_server(msg).await
                    .context("auto-connect: connection request")?;

                // 2. Auto-inject ConnectionResponse (aircraft → station, accepted)
                let msg = client.cpdlc_connection_response(aircraft_cs, &aircraft_address, station_cs, true);
                client.send_to_server(msg).await
                    .context("auto-connect: connection response")?;

                info!(station = %station_cs, aircraft = %aircraft_cs, "auto-connect: session established");
            }

            // Relay logon response to Hoppie
            let body = if *accepted { "LOGON ACCEPTED" } else { "LOGON REJECTED" };
            let packet = crate::hoppie_client::format_cpdlc_packet(
                "1", None, "N", body,
            );

            let from_cs = if bridge_callsigns.contains(&station_cs.to_uppercase()) {
                station_cs
            } else {
                station_cs
            };

            hoppie
                .send_cpdlc(from_cs, aircraft_cs, &packet)
                .await
                .context("failed to relay logon response to Hoppie")?;

            info!(
                station = %station_cs,
                aircraft = %aircraft_cs,
                accepted = %accepted,
                "relayed LogonResponse → Hoppie"
            );

            Ok(())
        }
        CpdlcMetaMessage::SessionUpdate { .. } => {
            debug!("ignoring SessionUpdate from server");
            Ok(())
        }
        other => {
            debug!(?other, "ignoring unhandled meta message");
            Ok(())
        }
    }
}

/// Determine which callsign is the aircraft and which is the station.
fn determine_aircraft_station(
    source: &str,
    dest: &str,
    bridge_callsigns: &HashSet<String>,
) -> (String, String) {
    if bridge_callsigns.contains(dest) {
        (source.to_string(), dest.to_string())
    } else {
        (dest.to_string(), source.to_string())
    }
}
