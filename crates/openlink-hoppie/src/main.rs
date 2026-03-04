use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use openlink_hoppie::bridge;
use openlink_hoppie::config::BridgeConfig;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = BridgeConfig::parse();
    bridge::run_bridge(config).await
}
