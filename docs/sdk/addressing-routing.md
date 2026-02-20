# Addressing and routing

## Purpose

Clarify the difference between transport routing identity and operational identity.

## Two identity layers

- **Network address**: routing key used in NATS subjects
- **Operational identity**: callsign used in CPDLC content

## Core routing rules

1. Route transport with network address only.
2. Put callsigns in CPDLC source/destination fields.
3. Keep network address stable during a live session.

## Subject mapping

- outbound: `openlink.v1.{network}.outbox.{network_address}`
- inbound: `openlink.v1.{network}.inbox.{network_address}`

## Practical example

Input:

- network: `vatsim`
- network address: `CID_987654`
- callsign: `AFR123`

Usage:

- publish to `openlink.v1.vatsim.outbox.CID_987654`
- subscribe to `openlink.v1.vatsim.inbox.CID_987654`
- set CPDLC `source` to `AFR123`

## Common mistakes

- deriving network address from callsign,
- using callsign directly in NATS subject,
- changing routing address mid-session.

## Related pages

- [NATS transport](nats-transport.md)
- [Stations and presence](stations-presence.md)
