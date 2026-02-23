# OpenLink CLI

A simplified Command Line Interface for demonstrating CPDLC flows using the OpenLink SDK.
This CLI supports both:
- **fire-and-forget** message sending (good for scripts), and
- **presence-aware** station lifecycle (`ONLINE` heartbeat + `OFFLINE` on stop) for long-running listeners.

> **Note:** The TUI (Terminal User Interface) mode is currently disabled in favor of this direct command mode for protocol development.

## Usage

The CLI requires defining the **Global Scope** (Network Identity) and the **ACARS Endpoint** properties before specifying the action.

### General Syntax
```bash
cargo run -p openlink-cli -- \
  --network-id <NETWORK> \
  --network-address <NETWORK_ADDR> \
  acars \
  --callsign <CALLSIGN> \
  --address <ICAO_ADDRESS> \
  [SUBCOMMAND]
```

### 1. Start NATS Server
Ensure your NATS server is running locally on port 4222.
```bash
docker-compose up -d nats
```

### 2. Presence Management

#### One-shot Online (legacy fire-and-forget)
Send a single `ONLINE` status message and exit.
```bash
cargo run -p openlink-cli -- \
  --network-id demonetwork --network-address LFPG \
  acars --callsign LFPG --address LFPGCYA \
  online
```

#### Hold Online with heartbeat (recommended for active station sessions)
Sends `ONLINE` periodically until `Ctrl+C`, then sends `OFFLINE`.
```bash
cargo run -p openlink-cli -- \
  --network-id demonetwork --network-address LFPG \
  acars --callsign LFPG --address LFPGCYA \
  online --hold --heartbeat-seconds 25
```

#### Explicit Offline
```bash
cargo run -p openlink-cli -- \
  --network-id demonetwork --network-address LFPG \
  acars --callsign LFPG --address LFPGCYA \
  offline
```

### 3. CPDLC Commands

All CPDLC commands are nested under the `cpdlc` subcommand of `acars`.
You must specify whether you are acting as a **pilot** or **atc**, and provide the aircraft details (even if you are the ATC, to contextualize the message).

#### Listen for Messages
Subscribes to the inbox and prints received envelopes.

`listen` now manages presence automatically:
- sends `ONLINE` at startup,
- refreshes it periodically (heartbeat),
- sends `OFFLINE` on shutdown (`Ctrl+C`).
```bash
cargo run -p openlink-cli -- \
  --network-id demonetwork --network-address ATC \
  acars --callsign LFPG --address LFPGCYA \
  cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --atc \
  listen
```

#### Send Logon Request (Pilot -> ATC)
```bash
# As Pilot AFR123 (Addr: AY213)
cargo run -p openlink-cli -- \
  --network-id demonetwork --network-address PILOT \
  acars --callsign AFR123 --address AY213 \
  cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --pilot \
  send logon-request --station LFPG --origin LFPG --destination EGLL
```

#### Send Logon Response (ATC -> Pilot)
```bash
# As ATC LFPG
cargo run -p openlink-cli -- \
  --network-id demonetwork --network-address ATC \
  acars --callsign LFPG --address LFPGCYA \
  cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --atc \
  send logon-response --accepted
```

## Supported Messages

### Session / Handover helpers
- `logon-request`: Initiate a session (Pilot).
- `logon-response`: Accept/Reject a session (ATC).
- `connection-request`: Open a CPDLC connection (ATC).
- `connection-response --accepted --station <ATC>`: Confirm connection (Pilot).
- `contact-request --station <NEXT_ATC>`: Sends standard `UM117 CONTACT [unit] [frequency]` helper (ATC).
- `contact-response --accepted --station <ATC>`: Sends short response helper (`DM0`/`DM1`) (Pilot).
- `contact-complete --station <ATC_OR_AIRCRAFT>`: Sends standard `DM89 MONITORING [unit] [frequency]` helper.
- `next-data-authority`: Sends standard `UM160 NEXT DATA AUTHORITY` helper (ATC).
- `logon-forward`: Forward session to next unit (ATC).
- `end-service`: Sends standard `UM161 END SERVICE` helper (ATC).

Notes:
- Only logon/connection/session-update/forward remain CPDLC protocol meta messages.
- Handover/termination helpers above are emitted as CPDLC **application** messages.

### Operational (Planned)
- `climb-to --level FL350`: Uplink instruction (ATC).
- `request-level-change --level FL350 --station <ATC>`: Downlink request (Pilot).

### Generic all UM/DM support

Use `um-dm` to send any CPDLC application message from the registry by ID.

```bash
# ATC sends UM20 CLIMB TO FL350 to aircraft
cargo run -p openlink-cli -- \
  --network-id demonetwork --network-address ATC \
  acars --callsign LFPG --address LFPGCYA \
  cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --atc \
  send um-dm --id UM20 --args FL350

# Pilot sends DM67 free text to ATC LFPG
cargo run -p openlink-cli -- \
  --network-id demonetwork --network-address PILOT \
  acars --callsign AFR123 --address AY213 \
  cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --pilot \
  send um-dm --id DM67 --args "REQUEST DIRECT" --to LFPG
```

Notes:
- `--args` order must match the ICAO message definition argument order.
- `--mrn` can be provided for response messages.
- For `--pilot`, `--to` is required unless using dedicated commands.

## Architecture

The CLI uses `clap` for argument parsing and `openlink-sdk` for:
1. **Authentication**: Fetches an ID Token from `mock-oidc` using the `--network-address` as the authorization code.
2. **Connection**: Connects to NATS (`nats://localhost:4222`).
3. **Messaging**: Constructs nested `OpenLinkEnvelope` -> `AcarsEnvelope` -> `CpdlcEnvelope` structures.

## Presence notes

- `cpdlc send ...` remains **fire-and-forget** (no long-lived online session required).
- `cpdlc listen` and `online --hold` are **presence-aware** and compatible with server-side online/offline tracking.
- Heartbeat interval default is `25s`, configurable via env var `CLI_PRESENCE_HEARTBEAT_SECONDS`.

## Sample Sequence

```
cargo run -p openlink-cli -- --network-id demonetwork --network-address PILOT acars --callsign AFR123 --address AY213 online

cargo run -p openlink-cli -- --network-id demonetwork --network-address ATC acars --callsign LFPG --address LFPGCYA online

# in two different terms:
cargo run -p openlink-cli -- --network-id demonetwork --network-address PILOT acars --callsign AFR123 --address AY213 cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --pilot listen

cargo run -p openlink-cli -- --network-id demonetwork --network-address ATC acars --callsign LFPG --address LFPGCYA cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --atc listen


cargo run -p openlink-cli -- --network-id demonetwork --network-address PILOT acars --callsign AFR123 --address AY213 cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --pilot send logon-request --station LFPG --origin LFPG --destination EGLL

cargo run -p openlink-cli -- --network-id demonetwork --network-address ATC acars --callsign LFPG --address LFPGCYA cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --atc send logon-response --accepted
```