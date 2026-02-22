# OpenLink A320 MCDU/DCDU Demonstrator

A browser-based example client simulating an **Airbus A320** cockpit interface for the [OpenLink](../../README.md) datalink network.

This project demonstrates how to integrate with OpenLink using **raw NATS over WebSocket** from a pure client-side React application — no Rust SDK needed.

## Architecture

The interface is split into two avionics units, just like in a real A320:

```
┌─────────────────────────────────────────┐
│              D C D U                    │
│   Datalink Control and Display Unit     │
│   • Displays received CPDLC messages    │
│   • Shows response options (WILCO, etc) │
│   • Message history navigation          │
├─────────────────────────────────────────┤
│           Status Bar                    │
│   • Connection status • Callsign • ATC  │
├─────────────────────────────────────────┤
│              M C D U                    │
│   Multifunction Control Display Unit    │
│   • ATC Menu navigation                 │
│   • Connection Status page              │
│   • Notification page (LOGON)           │
│   • Scratchpad for keyboard input       │
│   • Line Select Keys (LSK) on sides     │
└─────────────────────────────────────────┘
```

## Features Implemented

- **Authentication**: OIDC code exchange with the OpenLink auth service
- **NATS WebSocket**: Direct connection using `nats.ws` library
- **Station Presence**: Online/offline status with heartbeat
- **CPDLC LOGON**: Full logon flow from MCDU Notification page
- **Session State**: Server-authoritative session updates
- **Auto-accept**: Automatic connection response (avionics behavior)

## MCDU Pages

| Page | Description |
|------|------------|
| **ATC MENU** | Main menu with navigation to all ATC functions |
| **CONNECTION STATUS** | Shows active/next ATC and connection phase |
| **NOTIFICATION** | Enter ATC center code and send LOGON request |

## How to Use

### Prerequisites

Make sure the OpenLink infrastructure is running:

```bash
# From the project root
docker-compose up -d          # NATS server
cargo run -p mock-oidc &      # Mock OIDC provider (port 4000)
cargo run -p openlink-auth &  # Auth service (port 3001)
cargo run -p openlink-server  # OpenLink server
```

### Run the A320 Client

```bash
cd clients/a320-mcdu-dcdu
npm install
npm run dev
```

Open `http://localhost:5173` in your browser.

### LOGON Flow

1. **Home Screen**: Fill in connection parameters and click CONNECT
2. **MCDU** → ATC MENU → Press LSK L6 (**NOTIFICATION**)
3. **Type** the ATC station code on your keyboard (e.g. `EDDS`)
4. Press LSK R2 (**NOTIFY**) to send the logon request
5. The **DCDU** will display the logon response from ATC

### Keyboard Controls

Your computer keyboard simulates the MCDU keypad:

| Key | MCDU Function |
|-----|--------------|
| A-Z, 0-9 | Alphanumeric input → scratchpad |
| Backspace | CLR (delete last character) |
| Delete / Escape | Clear entire scratchpad |

## Technical Details

### Raw NATS Integration

This client uses the "raw NATS" approach described in `docs/sdk/quickstart-raw-nats.md`:

1. **Authenticate** → POST to `/exchange` with OIDC code → get NATS JWT
2. **Connect** → `nats.ws` WebSocket connection with JWT token
3. **Subscribe** → `openlink.v1.{network}.inbox.{address}`
4. **Publish** → `openlink.v1.{network}.outbox.{address}`

### Message Format

Messages follow the 3-layer envelope stack:
- **OpenLink Envelope** (transport) → **ACARS Envelope** (aero identity) → **CPDLC Message** (operational)

See `src/lib/types.ts` for the full TypeScript type definitions.

### Project Structure

```
src/
├── lib/
│   ├── types.ts          # TypeScript types for OpenLink wire format
│   ├── envelope.ts       # Envelope builders (station status, logon, etc.)
│   └── nats-client.ts    # Raw NATS WebSocket client
├── hooks/
│   └── useOpenLink.ts    # React hook for connection + session state
├── components/
│   ├── HomeScreen.tsx    # Connection setup form
│   ├── McduScreen.tsx    # MCDU display with ATC pages + scratchpad
│   ├── DcduScreen.tsx    # DCDU message display
│   └── StatusBar.tsx     # Connection/session status indicator
├── styles/
│   └── index.css         # A320 cockpit-inspired styling
├── App.tsx               # Main app component
└── main.tsx              # Entry point
```

## Tech Stack

- **React 19** + **TypeScript** — UI framework
- **Vite** — Build tool
- **nats.ws** — NATS WebSocket client for the browser
- **uuid** — Unique message ID generation
