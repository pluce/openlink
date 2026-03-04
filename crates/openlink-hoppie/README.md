# OpenLink Hoppie Bridge

Bidirectional bridge between the **Hoppie ACARS** network and the **OpenLink** datalink network.

## What it does

- Relays CPDLC messages between Hoppie-connected clients and OpenLink-connected clients
- Translates between the Hoppie `/data2/{min}/{mrn}/{response_attr}/{body}` packet format and OpenLink's structured `CpdlcEnvelope`
- Handles logon lifecycle: `REQUEST LOGON` → `LogonRequest` → `LogonResponse` → auto-inject `ConnectionRequest`/`ConnectionResponse` → `LOGON ACCEPTED`
- Registers Hoppie aircraft in the OpenLink station registry so the server can route messages to them
- Tracks MIN/MRN sequences across both systems for correct dialogue correlation
- Uses text-based template matching with specificity scoring to map rendered CPDLC text back to message element IDs
- Deduplicates messages to prevent relay loops

## Modes

| Mode | Description |
|------|-------------|
| `ground` | Proxies ground stations — OpenLink ATC can reach Hoppie pilots |
| `aircraft` | Proxies aircraft — OpenLink pilots can reach Hoppie ATC |
| `full` | Both directions (default) |

## Usage

The bridge requires its own network identity (separate from the GUI/ATC client).
Any unused auth code works — the mock-oidc fallback uses the code itself as the CID.

```bash
cargo run -p openlink-hoppie -- \
  --hoppie-logon YOUR_HOPPIE_KEY \
  --callsigns LFXB \
  --auth-code HOPPIE_BRIDGE \
  --network-id demonetwork \
  --nats-url nats://localhost:4222 \
  --auth-url http://localhost:3001 \
  --poll-interval-secs 20
```

## Architecture

```
Hoppie aircraft ←→ HTTPS ←→ openlink-hoppie ←→ NATS ←→ OpenLink server ←→ NATS ←→ GUI / CLI
```

The bridge is a standard OpenLink client that simultaneously polls the Hoppie HTTP API in a `tokio::select!` loop. Messages are translated and relayed in both directions.

### Identity model

The bridge runs with its own CID (network address), distinct from ATC GUI clients. When a Hoppie aircraft sends `REQUEST LOGON`, the bridge registers it in the OpenLink station registry under its own CID so the server routes messages destined for that aircraft to the bridge's inbox.

### Logon flow

1. Hoppie pilot sends `REQUEST LOGON` → bridge relays as `LogonRequest` and registers the aircraft
2. ATC accepts via GUI → server sends `LogonResponse(accepted)` to the aircraft's inbox (= bridge)
3. Bridge injects `ConnectionRequest` + `ConnectionResponse` to complete the CPDLC session
4. Bridge sends `LOGON ACCEPTED` back to the Hoppie pilot

### Message translation

Hoppie CPDLC packets use a text-based format: `/data2/{min}/{mrn}/{response_attr}/{body}`.
The translator matches the rendered body text against the OpenLink `MESSAGE_REGISTRY` templates using specificity scoring to find the best matching element ID and extract arguments.
