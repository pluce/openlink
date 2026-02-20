# Envelopes and message stack

## Purpose

Explain exactly what you need to serialize/parse in a client integration.

## 3-layer stack

1. OpenLink envelope (transport + routing)
2. ACARS envelope (aeronautical identity)
3. CPDLC message (operational content)

## Why this separation matters

- You can keep UI logic independent from transport details.
- You can validate operational content separately from network fields.
- You can reuse the same message layer in multiple products/languages.

## Field-level summary

### OpenLink envelope

- `id`: unique message id
- `timestamp`: creation time (UTC)
- `correlation_id`: optional request/response linkage
- `routing`: network source/destination
- `payload`: ACARS or system-level message
- `token`: auth token

### ACARS envelope

- `aircraft.callsign`
- `aircraft.address`
- payload message family (currently CPDLC)

### CPDLC envelope

- `source`
- `destination`
- `message` (`Application` or `Meta`)

## JSON example (short)

```json
{
  "id": "...",
  "timestamp": "...",
  "routing": {
    "source": { "Address": ["demonetwork", "CID_AFR123"] },
    "destination": { "Server": "demonetwork" }
  },
  "payload": {
    "type": "Acars",
    "data": {
      "routing": {
        "aircraft": {
          "callsign": "AFR123",
          "address": "AY213"
        }
      },
      "message": {
        "type": "CPDLC",
        "data": {
          "source": "AFR123",
          "destination": "LFPG",
          "message": {
            "type": "Application",
            "data": { "...": "..." }
          }
        }
      }
    }
  },
  "token": "..."
}
```

## Integrator best practices

- Keep the three layers intact and explicit.
- Do not add undocumented fields.
- Validate CPDLC content using `spec/cpdlc/catalog.v1.json` before send.
- Log invalid inbound payloads clearly for support/debug.

## Related pages

- [Addressing and routing](addressing-routing.md)
- [General concepts](concepts.md)
