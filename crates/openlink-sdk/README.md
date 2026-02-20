# OpenLink SDK (Rust)

A high-level **Client Library** for building OpenLink applications — cockpit (EFB)
or ground (ATC station). Abstracts NATS connectivity, authentication, subject
naming, and message framing so consumers only deal with domain types from
`openlink-models`.

Both aircraft-side clients **and** the OpenLink server use this crate to ensure
a single, consistent definition of NATS subjects and messaging patterns.

## Modules

| Module | Purpose |
|--------|---------|
| `client` | `OpenLinkClient` — connect, send, subscribe |
| `subjects` | `NatsSubjects` — canonical NATS subject & KV bucket names |
| `error` | `SdkError` — unified error type |
| `credentials` | `OpenLinkCredentials` — seed / JWT / CID bundle |

All builder types from `openlink-models` are re-exported at the crate root for
convenience: `MessageBuilder`, `CpdlcMessageBuilder`, `StationStatusBuilder`,
`EnvelopeBuilder`.

## NATS Subject Scheme

The SDK is the **single source of truth** for NATS topic naming:

| Method | Pattern | Used by |
|--------|---------|---------|
| `NatsSubjects::outbox(net, addr)` | `openlink.v1.{net}.outbox.{addr}` | Clients publish here |
| `NatsSubjects::inbox(net, addr)` | `openlink.v1.{net}.inbox.{addr}` | Clients subscribe here |
| `NatsSubjects::outbox_wildcard(net)` | `openlink.v1.{net}.outbox.>` | Server subscribes to all |
| `NatsSubjects::inbox_wildcard(net)` | `openlink.v1.{net}.inbox.>` | Server subscribes to all |
| `NatsSubjects::kv_cpdlc_sessions(net)` | `openlink-v1-{net}-cpdlc-sessions` | KV bucket |
| `NatsSubjects::kv_station_registry(net)` | `openlink-v1-{net}-station-registry` | KV bucket |

## Quick Start

```rust
use openlink_sdk::{OpenLinkClient, MessageBuilder, NatsSubjects};
use openlink_models::NetworkId;

// 1. Authenticate & connect
let network = NetworkId::new("demonetwork");
let client = OpenLinkClient::connect_with_authorization_code(
    "nats://localhost:4222",
    "http://localhost:3001",
    "<oidc-code>",
    &network,
).await?;

// 2. Subscribe to your inbox
let mut inbox = client.subscribe_inbox().await?;

// 3. Send a message through the server
let msg = MessageBuilder::cpdlc("AFR123", "1234")
    .from("AFR123")
    .to("LFPG")
    .logon_request("LFPG", "LFPG", "KJFK")
    .build();
client.send_to_server(msg).await?;

// 4. Receive
while let Some(raw) = inbox.next().await {
    let envelope: openlink_models::OpenLinkEnvelope =
        serde_json::from_slice(&raw.payload)?;
    println!("Received: {:?}", envelope);
}
```

## Features

### Authentication
- **OIDC exchange** – trades an authorization code for a NATS JWT via the
  OpenLink Auth service.
- **NKey management** – generates ephemeral Ed25519 user keys and signs server
  nonces during the NATS handshake.

### Connectivity
- **Typed client** – `OpenLinkClient` wraps `async-nats` and exposes domain
  methods (`send_to_server`, `send_to_station`, `subscribe_inbox`).
- **Resilience** – automatic reconnection handled by `async-nats`.

### Messaging
- **Envelope framing** – `send_to_server` automatically wraps an
  `OpenLinkMessage` in an `OpenLinkEnvelope` with UUID, timestamp, routing, and
  auth token.
- **Station-to-station** – `send_to_station` lets the server (or any
  authorised peer) push an envelope directly into another station's inbox.
- **Subject authority** – `NatsSubjects` is the single place that defines every
  subject and KV bucket name used across the platform.
