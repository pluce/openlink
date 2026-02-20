# OpenLink SDK – Aircraft / ATC Integration Guide

This section is written for teams integrating OpenLink into:

- an aircraft product (EFB, DCDU, FMC/CDU integration, cockpit plugin),
- an ATC product (controller client, ground station software).

The goal is to provide a clear integration path without requiring server-internal knowledge.

## Recommended reading path

1. [General concepts](concepts.md)
2. [Integration architecture](integration-architecture.md)
3. [NATS transport](nats-transport.md)
4. [Envelopes and message stack](envelopes-and-stack.md)
5. [Addressing and routing](addressing-routing.md)
6. [Stations and presence (online/offline)](stations-presence.md)
7. [Raw NATS quickstart](quickstart-raw-nats.md)
8. [High-level API contract](high-level-api-contract.md)
9. [Conformance profile](conformance-profile.md)
10. [Integration checklist](integration-checklist.md)
11. [CPDLC reference](reference/README.md)

## Design principle

- Protocol truth is provided by the catalog: `spec/cpdlc/catalog.v1.json`.
- High-level SDK integrations and low-level raw NATS integrations should use the same catalog.
- Reference documentation is generated to stay aligned with the model.

## Regenerating reference documentation

From repository root:

`cargo run -p openlink-models --example export_cpdlc_catalog -- spec/cpdlc/catalog.v1.json`

`cargo run -p openlink-models --example generate_cpdlc_reference -- spec/cpdlc/catalog.v1.json docs/sdk/reference/cpdlc-messages.md`
