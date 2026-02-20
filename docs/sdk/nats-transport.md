# NATS transport

## Purpose

Define the transport contract your aircraft or ATC client must implement.

## NATS references

- NATS clients and SDKs: https://docs.nats.io/using-nats/developer
- Publish/subscribe basics: https://docs.nats.io/nats-concepts/core-nats/pubsub
- Subject hierarchy: https://docs.nats.io/nats-concepts/subjects
- Example code by language: https://examples.nats.io/

## Subject convention

Use these subject patterns:

- `openlink.v1.{network}.outbox.{address}`
- `openlink.v1.{network}.inbox.{address}`

Parameters:

- `{network}`: logical network (example: `vatsim`)
- `{address}`: runtime client address on that network

## Required client behavior

1. Connect to NATS with valid credentials
2. Subscribe to your own inbox subject
3. Publish outbound envelopes to your own outbox subject
4. Consume inbox messages continuously and parse safely

## Authentication flow (recommended)

1. User/service gets authorization code from identity flow
2. Code is exchanged for NATS JWT + runtime identity
3. Client connects with those credentials

## Reconnection behavior

On disconnect/reconnect:

- restore NATS connection,
- restore inbox subscription,
- preserve pending UI state if needed.

## Example subject resolution

For `network=vatsim` and `address=CID_AFR123`:

- outbox: `openlink.v1.vatsim.outbox.CID_AFR123`
- inbox: `openlink.v1.vatsim.inbox.CID_AFR123`

## Common mistakes

- publishing to another client inbox,
- using callsign in place of runtime address in subject names,
- not re-subscribing inbox after reconnect.

## Related pages

- [Addressing and routing](addressing-routing.md)
- [Raw NATS integration quickstart](quickstart-raw-nats.md)
