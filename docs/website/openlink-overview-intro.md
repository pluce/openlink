## Why OpenLink?

OpenLink provides a modern, implementation-ready foundation for ACARS/CPDLC exchanges.

- **Interoperability first**: align message semantics with aviation protocols.
- **Operational realism**: station presence, session lifecycle, and authority transfer are explicit.
- **Integration speed**: SDK + raw NATS transport options for staged adoption.

## What is OpenLink?

OpenLink is a reference platform composed of:

- a protocol/domain layer (**openlink-models**),
- an integration SDK (**openlink-sdk**),
- an auth bridge (**openlink-auth** + OIDC flow),
- a routing/session authority server (**openlink-server**),
- operator-facing clients (CLI, GUI),
- and load/performance tooling.

## How does it work?

At runtime, clients publish outbound envelopes to network outbox subjects, the server applies routing and CPDLC session rules, then forwards to destination inboxes.

<div class="ol-arch-wrap">
  <img class="ol-arch ol-arch-dark" src="../../diagrams/openlink-architecture.svg" alt="OpenLink architecture diagram with bidirectional flows" />
  <img class="ol-arch ol-arch-light" src="../../diagrams/openlink-architecture-light.svg" alt="OpenLink architecture diagram with bidirectional flows" />
</div>

Direct runtime interactions:
- Clients ↔ openlink-auth (OIDC / token exchange)
- Clients ↔ NATS (publish/subscribe)
- openlink-server ↔ NATS/JetStream (routing + state)
- Clients ↔ openlink-server (logical messaging path via NATS)

## Connection to existing networks and standards

OpenLink is positioned as an integration layer for aviation-style operations:

- It can support workflows similar to **VATSIM/IVAO** controller–pilot exchanges.
- It models **ICAO CPDLC/ACARS** message behavior for realistic semantics.
- It does not replace public networks directly; it provides a programmable backbone to integrate with them.

---
