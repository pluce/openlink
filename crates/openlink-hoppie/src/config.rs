//! Bridge configuration.

use clap::Parser;

/// Hoppie ↔ OpenLink CPDLC bridge.
#[derive(Parser, Debug, Clone)]
#[command(name = "openlink-hoppie", about = "Bridge Hoppie ACARS with the OpenLink network")]
pub struct BridgeConfig {
    /// Hoppie logon code (API key).
    #[arg(long)]
    pub hoppie_logon: String,

    /// Hoppie API base URL.
    #[arg(long, default_value = "https://www.hoppie.nl/acars/system/connect.html")]
    pub hoppie_url: String,

    /// Polling interval in seconds.
    #[arg(long, default_value_t = 5)]
    pub poll_interval_secs: u64,

    /// Callsigns to proxy on the Hoppie side (comma-separated).
    /// The bridge will register these as stations on OpenLink and relay
    /// messages from/to Hoppie for them.
    #[arg(long, value_delimiter = ',')]
    pub callsigns: Vec<String>,

    /// Bridge mode: `ground`, `aircraft`, or `full`.
    #[arg(long, default_value = "full")]
    pub mode: BridgeMode,

    /// OpenLink network id.
    #[arg(long, default_value = "vatsim")]
    pub network_id: String,

    /// NATS server URL.
    #[arg(long, default_value = "nats://localhost:4222")]
    pub nats_url: String,

    /// OpenLink auth server URL.
    #[arg(long, default_value = "http://localhost:3001")]
    pub auth_url: String,

    /// OIDC authorization code for OpenLink authentication.
    #[arg(long)]
    pub auth_code: String,
}

/// Bridge operating mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeMode {
    /// Proxy ground stations: Hoppie ATC ↔ OpenLink pilots.
    Ground,
    /// Proxy aircraft: Hoppie pilots ↔ OpenLink ATC.
    Aircraft,
    /// Both directions (default).
    Full,
}

impl std::str::FromStr for BridgeMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ground" => Ok(Self::Ground),
            "aircraft" => Ok(Self::Aircraft),
            "full" => Ok(Self::Full),
            other => Err(format!("unknown bridge mode: {other}")),
        }
    }
}

impl std::fmt::Display for BridgeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ground => write!(f, "ground"),
            Self::Aircraft => write!(f, "aircraft"),
            Self::Full => write!(f, "full"),
        }
    }
}
