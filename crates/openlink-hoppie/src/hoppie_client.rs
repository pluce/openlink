//! Hoppie ACARS HTTP client.
//!
//! Implements polling and sending via the Hoppie ACARS HTTP API.
//! Reference: <https://www.hoppie.nl/acars/system/tech.html>

use anyhow::{Context, Result};
use tracing::{debug, warn};

/// A parsed message received from Hoppie.
#[derive(Debug, Clone)]
pub struct HoppieMessage {
    /// Sender callsign.
    pub from: String,
    /// Message type (cpdlc, telex, etc.).
    pub msg_type: HoppieMessageType,
    /// Raw packet data.
    pub packet: String,
}

/// Hoppie message types relevant to the bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoppieMessageType {
    /// CPDLC message.
    Cpdlc,
    /// Free-text telex.
    Telex,
    /// Unknown / unsupported type.
    Other(String),
}

impl HoppieMessageType {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "cpdlc" => Self::Cpdlc,
            "telex" => Self::Telex,
            other => Self::Other(other.to_string()),
        }
    }

    /// Return the string representation suitable for Hoppie API.
    #[allow(dead_code)]
    fn as_str(&self) -> &str {
        match self {
            Self::Cpdlc => "cpdlc",
            Self::Telex => "telex",
            Self::Other(s) => s,
        }
    }
}

/// HTTP client for the Hoppie ACARS system.
pub struct HoppieClient {
    http: reqwest::Client,
    base_url: String,
    logon: String,
}

impl HoppieClient {
    /// Create a new Hoppie client.
    pub fn new(base_url: &str, logon: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.to_string(),
            logon: logon.to_string(),
        }
    }

    /// Poll for pending messages for a given callsign.
    ///
    /// Returns a list of parsed messages. Hoppie delivers each message
    /// only once — subsequent polls return new messages only.
    pub async fn poll(&self, callsign: &str) -> Result<Vec<HoppieMessage>> {
        let params = [
            ("logon", self.logon.as_str()),
            ("from", callsign),
            ("to", "SERVER"),
            ("type", "poll"),
        ];

        let resp = self
            .http
            .post(&self.base_url)
            .form(&params)
            .send()
            .await
            .context("Hoppie poll request failed")?;

        let body = resp
            .text()
            .await
            .context("failed to read Hoppie poll response")?;

        debug!(callsign, "Hoppie poll response: {body}");

        parse_poll_response(&body)
    }

    /// Send a CPDLC message via Hoppie.
    pub async fn send_cpdlc(&self, from: &str, to: &str, packet: &str) -> Result<()> {
        self.send_message(from, to, "cpdlc", packet).await
    }

    /// Send a telex message via Hoppie.
    pub async fn send_telex(&self, from: &str, to: &str, packet: &str) -> Result<()> {
        self.send_message(from, to, "telex", packet).await
    }

    /// Send a raw message via Hoppie.
    async fn send_message(
        &self,
        from: &str,
        to: &str,
        msg_type: &str,
        packet: &str,
    ) -> Result<()> {
        let params = [
            ("logon", self.logon.as_str()),
            ("from", from),
            ("to", to),
            ("type", msg_type),
            ("packet", packet),
        ];

        let resp = self
            .http
            .post(&self.base_url)
            .form(&params)
            .send()
            .await
            .context("Hoppie send request failed")?;

        let body = resp
            .text()
            .await
            .context("failed to read Hoppie send response")?;

        if body.starts_with("ok") {
            debug!(from, to, msg_type, "Hoppie send ok");
            Ok(())
        } else {
            anyhow::bail!("Hoppie send error: {body}")
        }
    }
}

/// Parse the Hoppie poll response format.
///
/// Format: `ok {messages}` or `ok` (no messages).
/// Each message: `{from} {type} {packet}`
/// Messages are separated by `}` (closing brace at end of each block).
///
/// Actual format from Hoppie:
/// ```text
/// ok {AFR123 cpdlc {/data2/LFPG/NE/1//CLIMB TO FL350}}
/// ```
fn parse_poll_response(body: &str) -> Result<Vec<HoppieMessage>> {
    let body = body.trim();
    if body == "ok" || body == "ok {}" {
        return Ok(Vec::new());
    }

    if !body.starts_with("ok") {
        anyhow::bail!("unexpected Hoppie response: {body}");
    }

    // Strip the leading "ok " and parse message blocks.
    // Each block is enclosed in `{from type {data}}`.
    let content = &body[2..].trim();
    let mut messages = Vec::new();

    // Parse blocks: `{CALLSIGN type {packet_data}}`
    let mut remaining = content.as_bytes();
    while !remaining.is_empty() {
        // Skip whitespace
        while remaining.first() == Some(&b' ') || remaining.first() == Some(&b'\n') {
            remaining = &remaining[1..];
        }
        if remaining.is_empty() {
            break;
        }

        // Expect opening brace
        if remaining.first() != Some(&b'{') {
            warn!(
                "unexpected char in Hoppie response: {}",
                remaining[0] as char
            );
            break;
        }
        remaining = &remaining[1..];

        // Find the matching closing brace (accounting for nested braces)
        let mut depth = 1;
        let mut end = 0;
        for (i, &b) in remaining.iter().enumerate() {
            if b == b'{' {
                depth += 1;
            } else if b == b'}' {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
        }
        if depth != 0 {
            warn!("unbalanced braces in Hoppie response");
            break;
        }

        let block = std::str::from_utf8(&remaining[..end])
            .context("invalid UTF-8 in Hoppie block")?
            .trim();
        remaining = &remaining[end + 1..];

        // Parse block: "CALLSIGN type {packet}"
        if let Some(msg) = parse_message_block(block) {
            messages.push(msg);
        }
    }

    Ok(messages)
}

/// Parse a single message block: `CALLSIGN type {packet}` or `CALLSIGN type packet`.
fn parse_message_block(block: &str) -> Option<HoppieMessage> {
    // Split on first space to get callsign
    let (from, rest) = block.split_once(' ')?;
    // Split on second space to get type
    let (msg_type_str, packet_raw) = rest.split_once(' ')?;

    // Packet may be wrapped in braces
    let packet = packet_raw
        .trim()
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or(packet_raw.trim());

    Some(HoppieMessage {
        from: from.to_string(),
        msg_type: HoppieMessageType::from_str(msg_type_str),
        packet: packet.to_string(),
    })
}

// ── Hoppie CPDLC packet format helpers ──────────────────────────────

/// A parsed Hoppie CPDLC packet (`/data2/...` format).
///
/// Real Hoppie format: `/data2/{min}/{mrn}/{response_attr}/{message_text}`
#[derive(Debug, Clone)]
pub struct HoppieCpdlcPacket {
    /// Message Identification Number (running counter).
    pub min: String,
    /// Message Reference Number (the MIN of the message being replied to).
    pub mrn: Option<String>,
    /// Response attribute code (Y, N, WU, AN, R, NE).
    pub response_attr: String,
    /// The rendered message text (e.g. "CLIMB TO FL350", "WILCO", "REQUEST LOGON").
    pub body: String,
}

/// Parse a Hoppie CPDLC packet string.
///
/// Format: `/data2/{min}/{mrn}/{response_attr}/{message_text}`
pub fn parse_cpdlc_packet(packet: &str) -> Option<HoppieCpdlcPacket> {
    let packet = packet.trim();
    let data = packet.strip_prefix("/data2/")?;

    let parts: Vec<&str> = data.splitn(4, '/').collect();
    if parts.len() < 3 {
        warn!("malformed Hoppie CPDLC packet: {packet}");
        return None;
    }

    Some(HoppieCpdlcPacket {
        min: parts[0].to_string(),
        mrn: if !parts[1].is_empty() {
            Some(parts[1].to_string())
        } else {
            None
        },
        response_attr: parts[2].to_string(),
        body: if parts.len() > 3 {
            parts[3].to_string()
        } else {
            String::new()
        },
    })
}

/// Format a Hoppie CPDLC packet for sending.
///
/// Produces the `/data2/{min}/{mrn}/{response_attr}/{message_text}` string.
pub fn format_cpdlc_packet(
    min: &str,
    mrn: Option<&str>,
    response_attr: &str,
    body: &str,
) -> String {
    format!(
        "/data2/{}/{}/{}/{}",
        min,
        mrn.unwrap_or(""),
        response_attr,
        body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_poll() {
        let msgs = parse_poll_response("ok").unwrap();
        assert!(msgs.is_empty());
    }

    #[test]
    fn parse_single_message() {
        let body = "ok {AFR123 cpdlc {/data2/5//WU/CLIMB TO FL350}}";
        let msgs = parse_poll_response(body).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].from, "AFR123");
        assert_eq!(msgs[0].msg_type, HoppieMessageType::Cpdlc);
        assert_eq!(msgs[0].packet, "/data2/5//WU/CLIMB TO FL350");
    }

    #[test]
    fn parse_multiple_messages() {
        let body = "ok {AFR123 cpdlc {/data2/5//WU/CLIMB TO FL350}}{BAW456 telex {HELLO}}";
        let msgs = parse_poll_response(body).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].from, "AFR123");
        assert_eq!(msgs[1].from, "BAW456");
        assert_eq!(msgs[1].msg_type, HoppieMessageType::Telex);
    }

    #[test]
    fn parse_cpdlc_packet_basic() {
        let pkt = parse_cpdlc_packet("/data2/5//WU/CLIMB TO FL350").unwrap();
        assert_eq!(pkt.min, "5");
        assert!(pkt.mrn.is_none());
        assert_eq!(pkt.response_attr, "WU");
        assert_eq!(pkt.body, "CLIMB TO FL350");
    }

    #[test]
    fn parse_cpdlc_packet_with_mrn() {
        let pkt = parse_cpdlc_packet("/data2/6/5/N/WILCO").unwrap();
        assert_eq!(pkt.min, "6");
        assert_eq!(pkt.mrn.as_deref(), Some("5"));
        assert_eq!(pkt.response_attr, "N");
        assert_eq!(pkt.body, "WILCO");
    }

    #[test]
    fn parse_cpdlc_logon_request() {
        let pkt = parse_cpdlc_packet("/data2/5//Y/REQUEST LOGON").unwrap();
        assert_eq!(pkt.min, "5");
        assert!(pkt.mrn.is_none());
        assert_eq!(pkt.response_attr, "Y");
        assert_eq!(pkt.body, "REQUEST LOGON");
    }

    #[test]
    fn format_cpdlc_packet_basic() {
        let pkt = format_cpdlc_packet("5", None, "WU", "CLIMB TO FL350");
        assert_eq!(pkt, "/data2/5//WU/CLIMB TO FL350");
    }

    #[test]
    fn format_cpdlc_packet_with_mrn() {
        let pkt = format_cpdlc_packet("6", Some("5"), "N", "WILCO");
        assert_eq!(pkt, "/data2/6/5/N/WILCO");
    }

    #[test]
    fn hoppie_error_response() {
        let result = parse_poll_response("error {illegal logon code}");
        assert!(result.is_err());
    }
}
