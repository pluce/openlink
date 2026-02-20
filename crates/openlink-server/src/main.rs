//! OpenLink server — routes messages between stations on one or more networks.

use clap::Parser;
use openlink_models::NetworkId;

mod acars;
mod server;
mod station_registry;

fn read_u64_env(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

fn read_i64_env(name: &str, default: i64) -> i64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(default)
}

fn read_bool_env(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .and_then(|v| match v.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

/// OpenLink CPDLC relay server.
#[derive(Parser, Debug)]
#[command(name = "openlink-server", about = "OpenLink CPDLC relay server")]
struct Args {
    /// Delete all JetStream KV buckets on startup (station registry + CPDLC sessions).
    #[arg(long)]
    clean: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialise structured logging (controlled via RUST_LOG env var).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    let nats_url =
        std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    let auth_url =
        std::env::var("AUTH_URL").unwrap_or_else(|_| "http://localhost:3001".to_string());
    let server_secret =
        std::env::var("SERVER_SECRET").unwrap_or_else(|_| "openlink-dev-secret".to_string());

    let presence_config = server::PresenceConfig {
        lease_ttl_seconds: read_i64_env("PRESENCE_LEASE_TTL_SECONDS", 90).max(1),
        sweep_interval_seconds: read_u64_env("PRESENCE_SWEEP_INTERVAL_SECONDS", 20).max(1),
        auto_end_service_on_station_offline: read_bool_env(
            "AUTO_END_SERVICE_ON_STATION_OFFLINE",
            true,
        ),
    };

    let networks = vec![NetworkId::new("afrv"), NetworkId::new("demonetwork")];

    let mut handles = Vec::new();
    for network in networks {
        let server =
            server::OpenLinkServer::new(
                network,
                &nats_url,
                &auth_url,
                &server_secret,
                args.clean,
                presence_config,
            )
            .await?;
        let handle = tokio::spawn(async move {
            server.run().await;
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}
