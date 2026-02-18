# OpenLink CPDLC Demo Instructions

This system implements the ICAO GOLD session management (Logon vs Connection) using NATS as the messaging backbone.

## Prerequisites
- **NATS Server**: Must be running on `localhost:4222`.
- **Mock OIDC Server**: (Optional) For authentication, or use the "Auto-Connect" modes.

## Components

### 1. Session Manager (`openlink-cpdlc`)
This is the "Router" and "State Keeper". It enforces the session state machine.

```bash
cargo run -p openlink-cpdlc
```

### 2. Pilot Client (`openlink-cli`)
Simulates the aircraft cockpit.

```bash
# Start Pilot (AFR001)
cargo run -p openlink-cli -- pilot --callsign AFR001
```

**Commands:**
- `logon <ATC_ID> <ORIGIN> <DEST>`: Sends a Logon Request (Identification). 
  - *Note: This does NOT establish an active connection (CDA). It just identifies you to the system.*

- `dcl <GATE>`: Requests Departure Clearance.

### 3. ATC Client (`openlink-cli`)
Simulates the Air Traffic Control station.

```bash
# Start ATC (LFPG)
cargo run -p openlink-cli -- atc --station LFPG
```

**Commands:**
- `connect <PILOT_ID>`: Initiates a CPDLC Connection (Session Establishment).
  - *Must be sent AFTER Pilot logs on.*
  - *The Pilot will auto-accept this in the current demo.*

- `handoff <PILOT_ID> <NEXT_ATC_ID>`: Sends a "Next Data Authority" (NDA) notification.
  - *Sets the Next Authority (UM160).*

## Helper Scripts
You can use `verify_fix.sh` (if updated) to automate these tests, but manual terminal interaction is recommended to see the flow.
