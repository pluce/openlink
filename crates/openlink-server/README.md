# openlink-server

Message routing server for the OpenLink network. Subscribes to station outbox
subjects, processes envelopes, and forwards them to the correct destination
inbox.

## Architecture

```
┌────────────────────────────────────────────────┐
│                    main.rs                     │
│  Spawns one OpenLinkServer per network (tokio) │
└────────────────┬───────────────────────────────┘
                 │
      ┌──────────▼──────────┐
      │   server.rs          │
      │   OpenLinkServer     │
      │  ┌─────────────────┐ │
      │  │ subscribe outbox │──▶  route to inbox
      │  │ wildcard         │ │
      │  └────────┬────────┘ │
      │           │          │
      │   ┌───────▼───────┐  │
      │   │  dispatch by  │  │
      │   │  payload type │  │
      │   └───┬───────┬───┘  │
      └───────┤       ├──────┘
              │       │
     ┌────────▼──┐ ┌──▼───────────────┐
     │  Meta     │ │  ACARS / CPDLC   │
     │  handler  │ │  acars.rs        │
     │           │ │  CPDLCServer      │
     └────┬──────┘ └──────┬───────────┘
          │               │
     ┌────▼───────────────▼────┐
     │  station_registry.rs    │
     │  StationRegistry        │
     │  (JetStream KV)         │
     └─────────────────────────┘
```

### Modules

| Module               | Description |
|----------------------|-------------|
| `main.rs`            | Entry point — configures `tracing`, reads `NATS_URL`, spawns one `OpenLinkServer` task per network. |
| `server.rs`          | `OpenLinkServer` — subscribes to the outbox wildcard subject, deserialises envelopes, dispatches to the Meta or ACARS handler, then forwards the result to the destination station's inbox. |
| `acars.rs`           | `CPDLCServer` + CPDLC session state machine (`CPDLCSession`, `CPDLCConnection`). Manages per-aircraft sessions in a JetStream KV bucket and processes CPDLC meta-messages (logon, connection, NDA, termination). |
| `station_registry.rs`| `StationRegistry` — maps `StationId`s to their runtime status, network address, and ACARS routing endpoint via a JetStream KV bucket. Provides callsign lookup for message routing. |

### NATS subjects & KV buckets

Subject and bucket names are defined centrally in `openlink-sdk::NatsSubjects`:

| Purpose               | Pattern |
|-----------------------|---------|
| Station outbox        | `openlink.v1.{network}.outbox.{address}` |
| Station inbox         | `openlink.v1.{network}.inbox.{address}` |
| Outbox wildcard (sub) | `openlink.v1.{network}.outbox.>` |
| CPDLC sessions KV     | `openlink-v1-{network}-cpdlc-sessions` |
| Station registry KV   | `openlink-v1-{network}-station-registry` |

## Configuration

| Env var    | Default                   | Description |
|------------|---------------------------|-------------|
| `NATS_URL` | `nats://localhost:4222`   | NATS server URL. |
| `AUTH_URL` | `http://localhost:3001`   | OpenLink auth service URL used to fetch server JWTs. |
| `SERVER_SECRET` | `openlink-dev-secret` | Shared secret used by the server to authenticate with auth service. |
| `PRESENCE_LEASE_TTL_SECONDS` | `90` | Station heartbeat lease TTL; after this delay without refresh, station is marked offline. |
| `PRESENCE_SWEEP_INTERVAL_SECONDS` | `20` | Frequency of stale presence sweep. |
| `AUTO_END_SERVICE_ON_STATION_OFFLINE` | `true` | When `true`, server sends automatic CPDLC `END SERVICE` to aircraft when a station goes offline. |
| `RUST_LOG` | `info`                    | Logging level filter (uses `tracing-subscriber` `EnvFilter`). |

## Running

```bash
# Start NATS (e.g. via docker-compose at the repo root)
docker compose up -d

# Run the server
cargo run -p openlink-server

# With debug logging
RUST_LOG=debug cargo run -p openlink-server
```

## Tests

Unit tests cover the CPDLC session state machine (logon → connection →
NDA → termination, multi-station handover). Integration tests for KV
operations require a running NATS server at `localhost:4222`.

```bash
cargo test -p openlink-server
```

## Dependencies

| Crate               | Role |
|----------------------|------|
| `openlink-models`   | Domain types (envelopes, ACARS, CPDLC, stations) |
| `openlink-sdk`      | NATS subject/bucket naming (`NatsSubjects`) |
| `async-nats` 0.33   | NATS client + JetStream KV |
| `tokio`              | Async runtime |
| `serde` / `serde_json` | Envelope (de)serialisation |
| `chrono`             | Timestamps in station registry entries |
| `anyhow`             | Error handling |
| `tracing` / `tracing-subscriber` | Structured logging |
| `futures`            | `StreamExt` / `TryStreamExt` for subscription + KV key iteration |
