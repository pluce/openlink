use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use clap::{Parser, ValueEnum};
use futures::StreamExt;
use openlink_models::{
    AcarsEndpointAddress, AcarsEndpointCallsign, AcarsMessage, CpdlcArgument, MessageBuilder,
    MessageElement, MetaMessage, NetworkAddress, NetworkId, OpenLinkEnvelope, OpenLinkMessage,
    StationId, StationStatus,
};
use openlink_sdk::{NatsSubjects, OpenLinkClient};
use tokio::sync::Mutex;

#[derive(Parser, Debug, Clone)]
#[command(name = "openlink-loadtest")]
#[command(about = "OpenLink server load test tool")]
struct Args {
    #[arg(long, default_value = "nats://localhost:4222")]
    nats_url: String,

    #[arg(long, default_value = "http://localhost:3001")]
    auth_url: String,

    #[arg(long, default_value = "demonetwork")]
    network_id: String,

    #[arg(long, value_enum, default_value_t = Scenario::OneWay)]
    scenario: Scenario,

    #[arg(long, default_value_t = 50)]
    pairs: usize,

    #[arg(long, default_value_t = 1)]
    pilots_per_atc: usize,

    #[arg(long, default_value_t = 30)]
    duration_seconds: u64,

    #[arg(long, default_value_t = 0)]
    rate_per_pair: u64,

    #[arg(long, default_value_t = 5)]
    settle_seconds: u64,

    #[arg(long, default_value_t = 0)]
    warmup_seconds: u64,

    #[arg(long, default_value_t = 5)]
    preflight_timeout_seconds: u64,

    #[arg(long, default_value_t = false)]
    skip_preflight: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
enum Scenario {
    OneWay,
    Echo,
    Mixed,
}

#[derive(Default)]
struct Metrics {
    sent: AtomicU64,
    received: AtomicU64,
    received_without_correlation: AtomicU64,
    errors: AtomicU64,
    latencies_us: Mutex<Vec<u64>>,
}

#[derive(Clone)]
struct Endpoint {
    cid: String,
    callsign: AcarsEndpointCallsign,
    address: AcarsEndpointAddress,
    client: OpenLinkClient,
}

#[derive(Clone)]
struct Pair {
    atc: Endpoint,
    pilot: Endpoint,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    let network = NetworkId::new(&args.network_id);

    println!(
        "Starting load test: scenario={:?}, target_pairs={}, pilots_per_atc={}, duration={}s, rate_per_pair={} msg/s",
        args.scenario, args.pairs, args.pilots_per_atc, args.duration_seconds, args.rate_per_pair
    );

    let pilots_per_atc = args.pilots_per_atc.max(1);
    let atc_count = args.pairs.div_ceil(pilots_per_atc);
    let mut pairs = Vec::with_capacity(args.pairs);
    let mut atc_endpoints = Vec::with_capacity(atc_count);

    for i in 0..atc_count {
        let atc_cid = format!("LT-ATC-{i:05}");

        let atc_client = OpenLinkClient::connect_with_authorization_code(
            &args.nats_url,
            &args.auth_url,
            &atc_cid,
            &network,
        )
        .await?;

        let atc = Endpoint {
            cid: atc_client.cid().to_string(),
            callsign: AcarsEndpointCallsign::new(&format!("LTATC{i:05}")),
            address: AcarsEndpointAddress::new(&format!("A{i:06}")),
            client: atc_client,
        };
        register_online(&network, &atc).await?;
        atc_endpoints.push(atc.clone());

        for j in 0..pilots_per_atc {
            if pairs.len() >= args.pairs {
                break;
            }
            let pilot_idx = i * pilots_per_atc + j;
            let pilot_cid = format!("LT-PILOT-{pilot_idx:05}");

            let pilot_client = OpenLinkClient::connect_with_authorization_code(
                &args.nats_url,
                &args.auth_url,
                &pilot_cid,
                &network,
            )
            .await?;

            let pilot = Endpoint {
                cid: pilot_client.cid().to_string(),
                callsign: AcarsEndpointCallsign::new(&format!("LTPIL{pilot_idx:05}")),
                address: AcarsEndpointAddress::new(&format!("P{pilot_idx:06}")),
                client: pilot_client,
            };

            register_online(&network, &pilot).await?;
            pairs.push(Pair {
                atc: atc.clone(),
                pilot,
            });
        }
    }

    println!(
        "Topology ready: atc_count={}, pilot_count={}, effective_pairs={}",
        atc_endpoints.len(),
        pairs.len(),
        pairs.len()
    );

    if !args.skip_preflight {
        if let Some(first_pair) = pairs.first().cloned() {
            preflight_routing(&first_pair, &network, args.preflight_timeout_seconds).await?;
            println!("Preflight: OK");
        } else {
            return Err(anyhow!("no pair created for preflight"));
        }
    }

    if args.warmup_seconds > 0 {
        println!("Warmup {}s...", args.warmup_seconds);
        tokio::time::sleep(Duration::from_secs(args.warmup_seconds)).await;
    }

    let metrics = Arc::new(Metrics::default());
    let deadline = Instant::now() + Duration::from_secs(args.duration_seconds);

    let mut tasks = Vec::new();

    for pair in pairs.clone() {
        let metrics_recv = metrics.clone();
        let scenario = args.scenario;
        let network_clone = network.clone();
        tasks.push(tokio::spawn(async move {
            run_pilot_receiver(pair, scenario, network_clone, deadline, metrics_recv).await
        }));
    }

    if args.scenario == Scenario::Echo {
        for atc in atc_endpoints {
            let metrics_recv = metrics.clone();
            tasks.push(tokio::spawn(async move {
                run_atc_receiver(atc, deadline, metrics_recv).await
            }));
        }
    }

    for pair in pairs {
        let metrics_send = metrics.clone();
        let scenario = args.scenario;
        let rate = args.rate_per_pair;
        let network_clone = network.clone();
        tasks.push(tokio::spawn(async move {
            run_sender(pair, scenario, network_clone, deadline, rate, metrics_send).await
        }));
    }

    for handle in tasks {
        if let Err(e) = handle.await {
            eprintln!("task join error: {e}");
            metrics.errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    tokio::time::sleep(Duration::from_secs(args.settle_seconds)).await;

    // best effort offline updates
    // reconnect list unavailable here; intentionally omitted for load-only execution speed.

    report_metrics(args.duration_seconds, &metrics).await;

    Ok(())
}

async fn register_online(network: &NetworkId, endpoint: &Endpoint) -> Result<()> {
    let msg = OpenLinkMessage::Meta(MetaMessage::StationStatus(
        StationId::new(&endpoint.cid),
        StationStatus::Online,
        openlink_models::AcarsRoutingEndpoint::new(
            endpoint.callsign.to_string(),
            endpoint.address.to_string(),
        ),
    ));
    endpoint.client.send_to_server(msg).await?;
    // tiny delay to let registry/index settle before high-rate sends
    tokio::time::sleep(Duration::from_millis(5)).await;
    let _ = network;
    Ok(())
}

fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

fn build_corr(pair_id: &str, seq: u64) -> String {
    format!("lt:{pair_id}:{seq}:{}", now_nanos())
}

fn parse_corr_nanos(correlation_id: Option<&str>) -> Option<u128> {
    let cid = correlation_id?;
    let mut parts = cid.split(':');
    let tag = parts.next()?;
    if tag != "lt" {
        return None;
    }
    let _pair = parts.next()?;
    let _seq = parts.next()?;
    let ns = parts.next()?;
    ns.parse::<u128>().ok()
}

async fn run_sender(
    pair: Pair,
    scenario: Scenario,
    network: NetworkId,
    deadline: Instant,
    rate_per_pair: u64,
    metrics: Arc<Metrics>,
) -> Result<()> {
    let outbox = NatsSubjects::outbox(&network, &NetworkAddress::new(&pair.atc.cid));
    let pair_id = format!("{}-{}", pair.atc.callsign, pair.pilot.callsign);

    let mut seq = 0_u64;
    let mut ticker = if rate_per_pair > 0 {
        Some(tokio::time::interval(Duration::from_nanos(
            1_000_000_000_u64 / rate_per_pair.max(1),
        )))
    } else {
        None
    };

    while Instant::now() < deadline {
        if let Some(t) = ticker.as_mut() {
            t.tick().await;
        }

        let msg = match scenario {
            Scenario::OneWay | Scenario::Echo => MessageBuilder::cpdlc(
                pair.pilot.callsign.to_string(),
                pair.pilot.address.to_string(),
            )
            .from(pair.atc.callsign.to_string())
            .to(pair.pilot.callsign.to_string())
            .climb_to(openlink_models::FlightLevel::new(350))
            .build(),
            Scenario::Mixed => {
                match seq % 3 {
                    0 => MessageBuilder::cpdlc(
                        pair.pilot.callsign.to_string(),
                        pair.pilot.address.to_string(),
                    )
                    .from(pair.atc.callsign.to_string())
                    .to(pair.pilot.callsign.to_string())
                    .application_message(vec![MessageElement::new(
                        "UM20",
                        vec![CpdlcArgument::Level(openlink_models::FlightLevel::new(350))],
                    )])
                    .build(),
                    1 => MessageBuilder::cpdlc(
                        pair.pilot.callsign.to_string(),
                        pair.pilot.address.to_string(),
                    )
                    .from(pair.atc.callsign.to_string())
                    .to(pair.pilot.callsign.to_string())
                    .application_message(vec![MessageElement::new(
                        "UM106",
                        vec![CpdlcArgument::Speed("M0.78".to_string())],
                    )])
                    .build(),
                    _ => MessageBuilder::cpdlc(
                        pair.pilot.callsign.to_string(),
                        pair.pilot.address.to_string(),
                    )
                    .from(pair.atc.callsign.to_string())
                    .to(pair.pilot.callsign.to_string())
                    .application_message(vec![MessageElement::new(
                        "UM169",
                        vec![CpdlcArgument::FreeText("LOADTEST PAYLOAD".to_string())],
                    )])
                    .build(),
                }
            }
        };

        let envelope = MessageBuilder::envelope(msg)
            .source_address(network.as_str(), pair.atc.cid.clone())
            .destination_server(network.as_str())
            .correlation_id(build_corr(&pair_id, seq))
            .build();

        if pair.atc.client.publish_envelope(&outbox, &envelope).await.is_ok() {
            metrics.sent.fetch_add(1, Ordering::Relaxed);
        } else {
            metrics.errors.fetch_add(1, Ordering::Relaxed);
        }

        seq = seq.saturating_add(1);
    }

    Ok(())
}

async fn run_pilot_receiver(
    pair: Pair,
    scenario: Scenario,
    _network: NetworkId,
    deadline: Instant,
    metrics: Arc<Metrics>,
) -> Result<()> {
    let mut sub = pair.pilot.client.subscribe_inbox().await?;
    while Instant::now() < deadline + Duration::from_secs(5) {
        let maybe = tokio::time::timeout(Duration::from_millis(500), sub.next()).await;
        let Ok(Some(msg)) = maybe else {
            continue;
        };

        let envelope = match serde_json::from_slice::<OpenLinkEnvelope>(&msg.payload) {
            Ok(v) => v,
            Err(_) => {
                metrics.errors.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        };

        metrics.received.fetch_add(1, Ordering::Relaxed);

        if let Some(sent_ns) = parse_corr_nanos(envelope.correlation_id.as_deref()) {
            let now = now_nanos();
            if now >= sent_ns {
                let latency_us = ((now - sent_ns) / 1_000) as u64;
                metrics.latencies_us.lock().await.push(latency_us);
            }
        } else {
            metrics
                .received_without_correlation
                .fetch_add(1, Ordering::Relaxed);
        }

        if scenario == Scenario::Echo
            && let OpenLinkMessage::Acars(acars) = envelope.payload
            && matches!(acars.message, AcarsMessage::CPDLC(_))
        {
            let response = MessageBuilder::cpdlc(
                pair.pilot.callsign.to_string(),
                pair.pilot.address.to_string(),
            )
            .from(pair.pilot.callsign.to_string())
            .to(pair.atc.callsign.to_string())
            .application_message(vec![MessageElement::new("DM0", vec![])])
            .build();
            let _ = pair.pilot.client.send_to_server(response).await;
        }
    }
    Ok(())
}

async fn run_atc_receiver(atc: Endpoint, deadline: Instant, metrics: Arc<Metrics>) -> Result<()> {
    let mut sub = atc.client.subscribe_inbox().await?;
    while Instant::now() < deadline + Duration::from_secs(5) {
        let maybe = tokio::time::timeout(Duration::from_millis(500), sub.next()).await;
        let Ok(Some(msg)) = maybe else {
            continue;
        };

        let envelope = match serde_json::from_slice::<OpenLinkEnvelope>(&msg.payload) {
            Ok(v) => v,
            Err(_) => {
                metrics.errors.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        };

        if let OpenLinkMessage::Acars(_) = envelope.payload {
            // For echo scenario, this is RTT-like for the response hop only unless correlation is preserved by clients.
            // We only count received throughput here.
            metrics.received.fetch_add(1, Ordering::Relaxed);
        }
    }
    Ok(())
}

async fn preflight_routing(pair: &Pair, network: &NetworkId, timeout_seconds: u64) -> Result<()> {
    let outbox = NatsSubjects::outbox(network, &NetworkAddress::new(&pair.atc.cid));
    let corr = build_corr("preflight", 0);
    let mut sub = pair.pilot.client.subscribe_inbox().await?;

    let msg = MessageBuilder::cpdlc(
        pair.pilot.callsign.to_string(),
        pair.pilot.address.to_string(),
    )
    .from(pair.atc.callsign.to_string())
    .to(pair.pilot.callsign.to_string())
    .climb_to(openlink_models::FlightLevel::new(350))
    .build();

    let envelope = MessageBuilder::envelope(msg)
        .source_address(network.as_str(), pair.atc.cid.clone())
        .destination_server(network.as_str())
        .correlation_id(corr.clone())
        .build();

    pair.atc.client.publish_envelope(&outbox, &envelope).await?;

    let timeout = Duration::from_secs(timeout_seconds.max(1));
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let maybe = tokio::time::timeout(remaining.min(Duration::from_millis(500)), sub.next()).await;
        let Ok(Some(msg)) = maybe else {
            continue;
        };
        let env = serde_json::from_slice::<OpenLinkEnvelope>(&msg.payload)?;
        if env.correlation_id.as_deref() == Some(corr.as_str()) {
            return Ok(());
        }
    }

    Err(anyhow!(
        "preflight failed: no routed message received within {}s",
        timeout_seconds.max(1)
    ))
}

async fn report_metrics(duration_seconds: u64, metrics: &Arc<Metrics>) {
    let sent = metrics.sent.load(Ordering::Relaxed);
    let received = metrics.received.load(Ordering::Relaxed);
    let no_corr = metrics.received_without_correlation.load(Ordering::Relaxed);
    let errors = metrics.errors.load(Ordering::Relaxed);

    let mut latencies = metrics.latencies_us.lock().await.clone();
    latencies.sort_unstable();

    let p = |q: f64| -> u64 {
        if latencies.is_empty() {
            return 0;
        }
        let idx = ((latencies.len() - 1) as f64 * q).round() as usize;
        latencies[idx]
    };

    let avg = if latencies.is_empty() {
        0.0
    } else {
        latencies.iter().sum::<u64>() as f64 / latencies.len() as f64
    };

    let send_tps = sent as f64 / duration_seconds.max(1) as f64;
    let recv_tps = received as f64 / duration_seconds.max(1) as f64;

    println!("\n=== Load test report ===");
    println!("sent={} received={} errors={}", sent, received, errors);
    println!("throughput_send={:.2} msg/s throughput_recv={:.2} msg/s", send_tps, recv_tps);
    println!(
        "latency_us: count={} avg={:.1} p50={} p95={} p99={} max={}",
        latencies.len(),
        avg,
        p(0.50),
        p(0.95),
        p(0.99),
        latencies.last().copied().unwrap_or(0),
    );
    if no_corr > 0 {
        println!(
            "diagnostic: received_without_correlation={} (messages received but not usable for latency)",
            no_corr
        );
    }
    if sent > 0 && received == 0 {
        println!(
            "diagnostic: no routed messages received. Check that openlink-server is running and stations are online."
        );
    }
}
