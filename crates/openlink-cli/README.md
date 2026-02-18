# OpenLink CLI

A simplified Command Line Interface for demonstrating CPDLC flows using the OpenLink SDK.
This CLI focuses on sending and receiving specific messages via CLI arguments, suitable for scripting and automated testing.

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

### 2. Connect as a Station (Online Status)
Send an `Online` status message to the network.
```bash
cargo run -p openlink-cli -- \
  --network-id vatsim --network-address LFPG \
  acars --callsign LFPG --address LFPGCYA \
  online
```

### 3. CPDLC Commands

All CPDLC commands are nested under the `cpdlc` subcommand of `acars`.
You must specify whether you are acting as a **pilot** or **atc**, and provide the aircraft details (even if you are the ATC, to contextualize the message).

#### Listen for Messages
Subscribes to the inbox and prints received envelopes.
```bash
cargo run -p openlink-cli -- \
  --network-id vatsim --network-address ATC \
  acars --callsign LFPG --address LFPGCYA \
  cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --atc \
  listen
```

#### Send Logon Request (Pilot -> ATC)
```bash
# As Pilot AFR123 (Addr: AY213)
cargo run -p openlink-cli -- \
  --network-id vatsim --network-address PILOT \
  acars --callsign AFR123 --address AY213 \
  cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --pilot \
  send logon-request --station LFPG --origin LFPG --destination EGLL
```

#### Send Logon Response (ATC -> Pilot)
```bash
# As ATC LFPG
cargo run -p openlink-cli -- \
  --network-id vatsim --network-address ATC \
  acars --callsign LFPG --address LFPGCYA \
  cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --atc \
  send logon-response --accepted
```

## Supported Messages

### Meta / Session Management
- `logon-request`: Initiate a session (Pilot).
- `logon-response`: Accept/Reject a session (ATC).
- `connection-request`: Open a CPDLC connection (ATC).
- `connection-response`: Confirm connection (Pilot).
- `next-data-authority`: Designate next unit (ATC).
- `logon-forward`: Forward session to next unit (ATC).

### Operational (Planned)
- `climb-to`: Instruction (ATC).
- `request-level-change`: Request (Pilot).

## Architecture

The CLI uses `clap` for argument parsing and `openlink-sdk` for:
1. **Authentication**: Fetches an ID Token from `mock-oidc` using the `--network-address` as the authorization code.
2. **Connection**: Connects to NATS (`nats://localhost:4222`).
3. **Messaging**: Constructs nested `OpenLinkEnvelope` -> `AcarsEnvelope` -> `CpdlcEnvelope` structures.

## Sample Sequence

```
cargo run -p openlink-cli -- --network-id vatsim --network-address PILOT acars --callsign AFR123 --address AY213 online

cargo run -p openlink-cli -- --network-id vatsim --network-address ATC acars --callsign LFPG --address LFPGCYA online

# in two different terms:
cargo run -p openlink-cli -- --network-id vatsim --network-address PILOT acars --callsign AFR123 --address AY213 cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --pilot listen

cargo run -p openlink-cli -- --network-id vatsim --network-address ATC acars --callsign LFPG --address LFPGCYA cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --atc listen


cargo run -p openlink-cli -- --network-id vatsim --network-address PILOT acars --callsign AFR123 --address AY213 cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --pilot send logon-request --station LFPG --origin LFPG --destination EGLL

cargo run -p openlink-cli -- --network-id vatsim --network-address ATC acars --callsign LFPG --address LFPGCYA cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --atc send logon-response --accepted
```