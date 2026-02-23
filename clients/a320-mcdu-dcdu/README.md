# OpenLink A320 MCDU/DCDU Demonstrator

A browser-based reproduction of the **Airbus A320 MCDU** (Multifunction Control and Display Unit) and **DCDU** (Datalink Control and Display Unit) implementing the CPDLC (Controller-Pilot Data Link Communications) protocol over the [OpenLink](../../README.md) network.

This project demonstrates how to integrate with OpenLink using **raw NATS over WebSocket** from a pure client-side React application — no Rust SDK needed.

Built with **React 19 + TypeScript + Vite**, communicating via **nats.ws** WebSocket client.

---

## Table of Contents

- [Architecture](#architecture)
- [OpenLink Spec Compliance](#openlink-spec-compliance)
- [CPDLC Protocol Implementation](#cpdlc-protocol-implementation)
- [MCDU — Message Composition](#mcdu--message-composition)
- [DCDU — Message Display & Response](#dcdu--message-display--response)
- [File Structure](#file-structure)
- [Getting Started](#getting-started)

---

## Architecture

```
┌──────────────┐    XFR TO DCDU    ┌──────────────┐     NATS WS      ┌──────────────┐
│              │ ─── elements ───► │              │ ── envelope ───► │              │
│    MCDU      │                   │    DCDU      │                   │  openlink    │
│ (compose)    │                   │  (display)   │ ◄── envelope ─── │   server     │
│              │                   │              │                   │              │
└──────────────┘                   └──────────────┘                   └──────────────┘
        │                                 │
        └────── useOpenLink hook ─────────┘
                (NATS, sessions,
                 presence, auth)
```

The client is split into two avionics displays, just like in a real A320:

- **MCDU** — The pilot composes CPDLC messages by navigating pages (LAT REQ, VERT REQ, TEXT, etc.), filling in parameters via the scratchpad, and accumulating message elements. Once ready, the pilot presses **XFR TO DCDU**.
- **DCDU** — Displays incoming and outgoing CPDLC messages. The pilot reads ATC instructions, sends responses (WILCO, UNABLE, STANDBY…), reviews drafts, and presses **SEND** to transmit.

Both panels share state through the **`useOpenLink`** hook, which manages the NATS connection, station presence, CPDLC session state, and the message queue.

Protocol/runtime logic is now centralized in a shared TypeScript SDK package at [clients/openlink-sdk-ts](../openlink-sdk-ts/README.md) (types, builders, catalog rendering, short-response selection, logical ACK rules).

---

## OpenLink Spec Compliance

### Envelope Stack

> Implements: `docs/sdk/envelopes-and-stack.md`

All messages use the OpenLink 3-layer envelope structure:

```
OpenLinkEnvelope
  └─ AcarsEnvelope
       └─ CpdlcEnvelope
            └─ CpdlcMessageType (Meta | Application)
```

- **OpenLinkEnvelope** — Top-level wrapper with `id`, `timestamp`, `routing` (source/destination addresses), `payload`, and `token` (JWT for authentication).
- **AcarsEnvelope** — ACARS-level routing with aircraft `callsign` and `address`.
- **CpdlcEnvelope** — Protocol-level with `source` (station callsign) and `destination`, plus the actual `CpdlcMessageType`.

The type definitions are in `src/lib/types.ts`, and the builder functions in `src/lib/envelope.ts`.

### NATS Transport

> Implements: `docs/sdk/nats-transport.md`, `docs/sdk/quickstart-raw-nats.md`

Communication uses the **raw NATS WebSocket** integration pattern:

- **Outbox subject:** `openlink.v1.{network}.outbox.{address}` — client publishes here
- **Inbox subject:** `openlink.v1.{network}.inbox.{address}` — client subscribes here
- The server reads from the client's outbox, processes the message, and routes it to the recipient's inbox.

The NATS client is implemented in `src/lib/nats-client.ts` using the `nats.ws` library.

### Station Presence

> Implements: `docs/sdk/stations-presence.md`

On connect, the client publishes a **StationOnline** envelope containing its callsign and ACARS address. A **heartbeat** re-publishes the station status every **25 seconds** to keep the presence alive. On disconnect, a **StationOffline** envelope is sent for graceful cleanup.

### Authentication

> Implements: `docs/sdk/quickstart-raw-nats.md` (auth section)

1. User provides an **OIDC authorization code** (from the mock-oidc service in dev)
2. The client exchanges the code for a **NATS JWT** via the `openlink-auth` service (`POST /exchange`)
3. The JWT is used to connect to NATS over WebSocket (`token` auth)
4. The JWT is included in every published envelope for server-side validation

The auth exchange goes through a **Vite reverse proxy** (`/api/auth/*`) in dev to avoid CORS issues.

---

## CPDLC Protocol Implementation

### Session Lifecycle

> Implements: `docs/acars-ref-gold/logon_connection.md`

The CPDLC session follows a server-authoritative state machine:

```
Idle ──► LogonPending ──► LoggedOn ──► Connected
                                         │
                                    (End Service)
                                         │
                                         ▼
                                       Idle
```

1. **Logon** — Pilot enters the ATC center code on the MCDU NOTIFICATION page and presses NOTIFY. A `LogonRequest` meta message is sent.
2. **LogonResponse** — The ATC station accepts or rejects the logon. Displayed on the DCDU.
3. **ConnectionRequest** — The ATC station sends a connection request. The avionics **auto-accepts** it (per standard A320 behavior).
4. **Connected** — CPDLC messages can now be exchanged.
5. **Handover UMs** — If ATC sends handover instructions as application messages (`UM117 CONTACT`, optionally preceded by `UM160 NEXT DATA AUTHORITY`), the client auto-sends a logon to the new station.

The client never computes session state locally — it trusts the server's authoritative `CpdlcSessionView` delivered via **SessionUpdate** meta messages.

### Message Exchange

> Implements: `docs/acars-ref-gold/messaging.md`

Once connected, CPDLC messages are exchanged as **Application** messages:

- **Uplinks (ATC → Aircraft):** UM (Uplink Message) elements, e.g. `UM20 CLIMB TO [level]`
- **Downlinks (Aircraft → ATC):** DM (Downlink Message) elements, e.g. `DM0 WILCO`, `DM9 REQUEST CLIMB TO [level]`

Each Application message contains:
- `min` — Message Identification Number (assigned by the server, unique per dialogue, range `1..63` cyclic)
- `mrn` — Message Reference Number (references the MIN of the message being responded to)
- `elements` — Array of `{ id, args }` objects referencing the CPDLC catalog
- `timestamp` — ISO 8601 timestamp

### Response Attributes

> Implements: `docs/acars-ref-gold/cpdlc_message_reference.md`

Each CPDLC message in the catalog has a **response attribute** determining what response is allowed:

| Attribute | Meaning | Allowed Responses | Priority |
|-----------|---------|-------------------|----------|
| **W/U** | Wilco / Unable | WILCO, UNABLE, STANDBY | 5 (highest) |
| **A/N** | Affirm / Negative | AFFIRM, NEGATIVE, STANDBY | 4 |
| **R** | Roger | ROGER, STANDBY | 3 |
| **Y** | Specific response required | Pilot composes a response from MCDU | 2 |
| **N** | No response required | Message closes immediately | 1 (lowest) |

When a multi-element uplink is received, the client scans **all** elements and picks the **highest-priority** response attribute. For example, if element 1 has `R` (priority 3) and element 2 has `W/U` (priority 5), the DCDU shows WILCO/UNABLE buttons (not ROGER).

This logic is implemented in `getResponseIntents()` in `src/lib/catalog.ts`.

### Dialog Management

> Implements: `docs/acars-ref-gold/messaging.md` (dialog lifecycle)

Each CPDLC exchange forms a **dialog** linked by MIN/MRN:

```
ATC sends: CLIMB TO FL360 (min=42)
  ↓
Pilot responds: STANDBY (mrn=42) → dialog stays OPEN
  ↓
Pilot responds: REQUEST FL380 // DUE TO WEATHER (mrn=42, server assigns min=99)
  ↓
ATC responds: ROGER (mrn=99) → dialog CLOSED
```

**Key behaviors:**

- **STANDBY (DM2)** keeps the dialog OPEN — the pilot can still send a definitive response later. The DCDU badge shows "STBY" in green highlight, and the STANDBY button is removed.
- **WILCO/UNABLE/ROGER** close the dialog — the DCDU badge shows the response label.
- **Server-assigned MINs** — The server assigns MINs to all downlinks, but never sends them back to the emitting client. The client resolves this with a **heuristic**: when an incoming message references an unknown MRN, the client finds the most recent outgoing "sent" message and assigns the incoming MRN as its MIN. This allows the dialog chain to be walked back to the root uplink.

### Message Catalog

> Implements: `spec/cpdlc/catalog.v1.json`, `docs/acars-ref-gold/cpdlc_message_reference.md`

The CPDLC catalog (`src/data/catalog.v1.json`) contains all DM and UM messages with their:
- `id` — Message identifier (e.g. "DM9", "UM20")
- `text` — Template text with `[arg]` placeholders
- `args` — Argument type definitions (Level, Speed, Position, etc.)
- `response_attr` — Required response type (WU, AN, R, Y, N, NE)

The catalog is loaded at module init time and used by:
- `elementsToTextParts()` — Converts element arrays to rich text (static vs. parameter spans)
- `formatArgValue()` — Formats argument values for display (e.g. 340 → "FL340" for Level type)
- `getResponseIntents()` — Determines which response buttons to show on the DCDU
- `textPartsToString()` — Flattens TextParts to a plain string

---

## MCDU — Message Composition

### Page Structure

The MCDU follows the standard A320 screen layout:
- **24 characters wide × 14 lines** (monospace)
- **Line 1** — Title
- **Lines 2–13** — 6 label/data row pairs, each aligned with physical LSK buttons (L1–L6, R1–R6)
- **Line 14** — Scratchpad (independent input buffer)

**Available pages:**

| Page | Purpose | DM Messages |
|------|---------|-------------|
| ATC MENU | Navigation hub + XFR TO DCDU | — |
| LAT REQ | Lateral requests | DM22 (Direct To), DM27 (Weather Dev), DM70 (Heading), DM65/66 (Due To) |
| VERT REQ | Vertical requests | DM9 (Climb), DM10 (Descent), DM6 (Altitude), DM18 (Speed), DM65/66 (Due To) |
| WHEN CAN WE | Timing requests | DM49 (Speed), DM50 (Level) |
| OTHER REQ | Miscellaneous | DM18 (Speed), DM70 (Heading), DM20 (Voice), DM25 (Clearance) |
| TEXT | Free text | DM67 (Free Text) |
| REPORTS | Position reports | DM32 (Level), DM48 (Position), DM34 (Speed) |
| NOTIFICATION | Logon to ATC | LogonRequest meta message |
| CONN STATUS | Connection info | — (display only) |

### Multi-Element Composition

The pilot can **accumulate multiple elements** across different pages before transferring to the DCDU:

1. Navigate to VERT REQ → Enter "FL380" → Press L1 (CLB TO) → DM9 added to pending
2. Navigate to LAT REQ → Press L3 (DUE TO WEATHER) → DM65 added to pending
3. Return to ATC MENU → Press R6 (XFR TO DCDU) → All pending elements sent as a single draft

**Accumulation rules:**
- Elements with arguments (DM9, DM22, etc.) **replace** any existing element with the same DM ID
- No-argument elements (DM65, DM66) and free text (DM67) **allow duplicates** — they stack
- RETURN buttons navigate back without erasing pending elements
- ERASE clears all pending elements and field values

### Argument Parsing

When the pilot enters a value in the scratchpad and presses an LSK:

| Argument Type | Parsing Rule | Example |
|---------------|-------------|---------|
| **Level** | Strip "FL" prefix, parseInt. 0–999 = Flight Level, >999 = altitude in feet | "FL340" → 340, "4000" → 4000 |
| **Speed** | parseInt directly | "280" → 280 |
| **Degrees** | parseInt (heading/track) | "270" → 270 |
| **Position** | String as-is (waypoint name) | "NARAK" → "NARAK" |
| **Distance** | String as-is | "10" → "10" |
| **FreeText** | String as-is | "UNABLE DUE WX" → "UNABLE DUE WX" |

### Keyboard Controls

Your computer keyboard simulates the MCDU keypad:

| Key | MCDU Function |
|-----|--------------|
| A-Z, 0-9, space, /, . | Alphanumeric input → scratchpad |
| Backspace | CLR (delete last character) |
| Delete / Escape | Clear entire scratchpad |

---

## DCDU — Message Display & Response

### Message States

Each message on the DCDU has a status displayed as a badge in the upper-right corner:

| Status | Badge | Background | Meaning |
|--------|-------|------------|---------|
| `open` | — | — | Uplink awaiting pilot response |
| `new` | — | — | New unread message |
| `sent` | STBY (if STANDBY sent) | Green highlight | Pilot sent STANDBY, dialog still open |
| `responding` | Response label | Flashing | Response being sent |
| `responded` | Response label | Green highlight | Dialog closed |
| `draft` | DRAFT | Cyan | Composed message awaiting SEND |
| `sending` | SENDING | — | Draft being transmitted |

### Dialog Chain Rendering

When a dialog has multiple exchanges (uplink → pilot response → ATC response), they are rendered **inline** under the root uplink message:

```
┌─────────────────────────────┐
│ FROM: LFPG           STBY   │  ← root uplink (status badge)
│ CLIMB TO FL360               │  ← uplink body
│ ─────────────────────────── │  ← separator
│ ██ REQUEST FL380 ██████████ │  ← pilot response (highlighted green)
│ ██ DUE TO WEATHER █████████ │
│ ─────────────────────────── │  ← separator
│ ROGER                        │  ← ATC response (green text, no highlight)
└─────────────────────────────┘
```

- **Pilot responses** (outgoing) are shown with a **green background** (or cyan if draft)
- **ATC responses** (incoming) are shown with normal **green text** (no background highlight)
- All linked messages are **hidden from MSG+/MSG- navigation** — only the root uplink is visible
- **PGE+/PGE-** buttons scroll the dialog content when it overflows

The chain is built by finding all messages whose `mrn` matches the root uplink's `min`, sorted by timestamp.

### Response Buttons

The DCDU lower zone shows response buttons based on the message's response attribute:

| Response Attribute | Left Side (L) | Right Side (R) |
|-------------------|---------------|----------------|
| **W/U** | WILCO | UNABLE |
| **A/N** | AFFIRM | NEGATIVE |
| **R** | ROGER | — |

**STANDBY** is always available on R5 (unless already sent).

After STANDBY is sent:
- Badge shows **STBY** in green highlight
- STANDBY button is removed
- Other response buttons remain available

### Draft Workflow

When the pilot composes a response via the MCDU and transfers it to the DCDU:

1. The draft appears in the dialog chain with a **cyan/blue background**
2. The lower zone shows **SEND\*** on R6
3. Normal response buttons are hidden while a draft is pending
4. Pressing SEND transmits the message and the background turns green (sent)

The draft is automatically linked to the most recent OPEN uplink via MRN.

---

## File Structure

```
src/
  main.tsx                    React entry point (StrictMode + DOM mount)
  App.tsx                     Root component — wires MCDU + DCDU + StatusBar
  ├── components/
  │   ├── HomeScreen.tsx      Connection form (OIDC code, callsign, NATS URL)
  │   ├── McduScreen.tsx      A320 MCDU — message composition (→ A320 avionics spec)
  │   ├── DcduScreen.tsx      A320 DCDU — message display & response (→ docs/acars-ref-gold/messaging.md)
  │   └── StatusBar.tsx       Session status bar (connection phase, CID)
  ├── hooks/
  │   └── useOpenLink.ts      Core hook: NATS connection, sessions, messaging (→ docs/sdk/quickstart-raw-nats.md)
  ├── lib/
  │   ├── types.ts            TypeScript types for envelope stack (→ docs/sdk/envelopes-and-stack.md)
  │   ├── envelope.ts         Envelope builders (→ docs/sdk/envelopes-and-stack.md)
  │   ├── catalog.ts          CPDLC catalog utilities (→ docs/acars-ref-gold/cpdlc_message_reference.md)
  │   └── nats-client.ts      Raw NATS WebSocket client (→ docs/sdk/nats-transport.md)
  ├── data/
  │   └── catalog.v1.json     CPDLC message catalog (→ spec/cpdlc/catalog.v1.json)
  └── styles/
      └── index.css           Shared styles (MCDU + DCDU + HomeScreen)
```

---

## Getting Started

### Prerequisites

- **Node.js** ≥ 18
- A running OpenLink stack:

```bash
# From the project root
docker-compose up -d          # NATS server
cargo run -p mock-oidc &      # Mock OIDC provider (port 4000)
cargo run -p openlink-auth &  # Auth service (port 3001)
cargo run -p openlink-server  # OpenLink server
```

### Development

```bash
cd clients/a320-mcdu-dcdu
npm install
npm run dev
```

Open `http://localhost:5173` in your browser. The Vite dev server proxies `/api/auth/*` to the auth service to avoid CORS.

### Build

```bash
npx tsc --noEmit && npx vite build
```

### Usage

1. Open the client in a browser
2. Enter connection settings:
   - **NATS URL:** `ws://localhost:4223` (WebSocket)
   - **Auth URL:** `http://localhost:3001`
   - **OIDC Code:** Any string (mock-oidc uses it as identity, e.g. "PILOT")
   - **Network:** `demonetwork`
   - **Callsign:** Your flight callsign (e.g. "AFR123")
   - **ACARS Address:** Your ACARS address (e.g. "AY213")
3. Click **Connect** — the station goes online and subscribes to its inbox
4. On the MCDU, go to **NOTIFICATION** → Enter ATC center (e.g. "LFPG") → Press **NOTIFY**
5. Wait for the logon acceptance and connection establishment
6. Compose and send CPDLC messages!

## Tech Stack

- **React 19** + **TypeScript** — UI framework
- **Vite** — Build tool with HMR
- **nats.ws** — NATS WebSocket client for the browser
- **uuid** — Unique message ID generation
