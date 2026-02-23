# OpenLink capabilities

This page summarizes what OpenLink enables for integrators and product teams.

## 1) CPDLC operations over a modern transport

OpenLink provides CPDLC exchanges on top of OpenLink envelopes and NATS transport.

Typical flows:

- logon request/response,
- connection request/response,
- next data authority handover,
- contact request and end service,
- operational application messages with structured arguments.

## 2) Session-aware operations

OpenLink maintains authoritative session context and distributes updates.

Integrators can:

- detect current active/inactive connection context,
- react to authority transfer updates,
- project operational state into cockpit or controller UI.

## 3) Presence-aware station model

OpenLink includes station presence events and routing identity.

Integrators can:

- publish `Online` / `Offline` station status,
- observe availability of remote endpoints,
- align callsign/address mapping across systems.

## 4) Multiple integration levels

Teams can integrate with:

- high-level SDK helpers for faster product delivery,
- raw envelope + NATS APIs for custom pipelines,
- generated CPDLC reference + catalog for strict protocol validation.

## 5) Cross-language SDK conformance

OpenLink includes language-neutral conformance assets:

- runtime vectors,
- wire examples,
- contract/profile/matrix documentation.

This supports consistent behavior across Rust, TypeScript, and future SDKs.

## Related pages

- [Concepts](concepts.md)
- [Integration architecture](integration-architecture.md)
- [Integrate with SDKs](integrate-with-sdks.md)
- [Develop a new SDK](develop-new-sdk.md)
- [Conformance test matrix](conformance-test-matrix.md)
