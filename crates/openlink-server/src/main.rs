//! OpenLink server — routes messages between stations on one or more networks.

use clap::Parser;
use openlink_models::NetworkId;

mod acars;
mod server;
mod station_registry;

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

    let networks = vec![NetworkId::new("afrv"), NetworkId::new("demonetwork")];

    let mut handles = Vec::new();
    for network in networks {
        let server =
            server::OpenLinkServer::new(network, &nats_url, &auth_url, &server_secret, args.clean).await?;
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
