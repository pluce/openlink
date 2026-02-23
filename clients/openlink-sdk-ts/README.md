# @openlink/sdk-ts

TypeScript SDK for OpenLink/CPDLC clients.

## Scope

This package centralizes protocol/client-runtime logic that should not be reimplemented in each UI:

- OpenLink/ACARS/CPDLC wire types
- Envelope/message builders
- Catalog-based rendering helpers
- Short-response selection logic (W/U, A/N, R, Y, N)
- Logical acknowledgement helpers (`DM100` / `UM227`)
- Raw NATS WebSocket client integration

The package aims to stay behaviorally aligned with the Rust SDK for runtime protocol rules.

## Main exports

- `client.ts`: high-level `OpenLinkClient` with Rust-aligned helper naming
- `types.ts`: protocol and app-facing TypeScript types
- `envelope.ts`: OpenLink envelope + CPDLC message builders
- `catalog.ts`: catalog loading, message rendering, response derivation
- `cpdlc-runtime.ts`: protocol decisions (logical ack, closing/standby rules, response priorities)
- `nats-client.ts`: raw NATS client (`OpenLinkNatsClient`)

## Cross-language parity

This SDK exposes:

- idiomatic TypeScript APIs,
- Rust-parity aliases for runtime symbols,
- a high-level `OpenLinkClient` with helper methods aligned to Rust SDK intents.

## Compliance

Read [docs/sdk/typescript-sdk-compliance.md](../../docs/sdk/typescript-sdk-compliance.md).
For cross-language SDK implementation guidance, read [docs/sdk/polyglot-sdk-contract.md](../../docs/sdk/polyglot-sdk-contract.md).

### Run runtime-vector conformance tests

From this package directory:

- `npm run test:conformance`

This dynamically loads `spec/sdk-conformance/runtime-vectors.v1.json` and validates runtime parity behavior.
