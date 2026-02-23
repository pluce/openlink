# OpenLink SDK – Aircraft / ATC Integration Guide

This section is written for teams integrating OpenLink into:

- an aircraft product (EFB, DCDU, FMC/CDU integration, cockpit plugin),
- an ATC product (controller client, ground station software),
- or a new language SDK implementation.

## What you can do with OpenLink

- exchange CPDLC messages over OpenLink envelopes,
- run realistic CPDLC operational flows (logon, connection, authority transfer, end service),
- consume authoritative session updates and station presence,
- integrate through high-level SDKs or low-level raw NATS,
- validate interoperability with catalog-driven conformance fixtures.

See [OpenLink capabilities](capabilities.md).

## How OpenLink works (architecture and concepts)

Core reading order:

1. [General concepts](concepts.md)
2. [Integration architecture](integration-architecture.md)
3. [NATS transport](nats-transport.md)
4. [Envelopes and message stack](envelopes-and-stack.md)
5. [Addressing and routing](addressing-routing.md)
6. [Stations and presence (online/offline)](stations-presence.md)

## Integrating OpenLink with existing applications

Recommended path for product teams:

1. [Integrate with SDKs](integrate-with-sdks.md)
2. [High-level API contract](high-level-api-contract.md)
3. [Integration checklist](integration-checklist.md)
4. [Conformance profile](conformance-profile.md)
5. [Conformance test matrix](conformance-test-matrix.md)

If needed, use [Raw NATS quickstart](quickstart-raw-nats.md).

## Developing a new SDK

Start here:

1. [Develop a new SDK](develop-new-sdk.md)
2. [Polyglot SDK contract](polyglot-sdk-contract.md)
3. [TypeScript SDK compliance profile](typescript-sdk-compliance.md)
4. [Conformance profile](conformance-profile.md)
5. [Conformance test matrix](conformance-test-matrix.md)
6. [CPDLC reference](reference/README.md)

## Design principle

- Protocol truth is provided by the catalog: `spec/cpdlc/catalog.v1.json`.
- High-level SDK integrations and low-level raw NATS integrations should use the same catalog.
- Reference documentation is generated to stay aligned with the model.

## Regenerating reference documentation

From repository root:

`cargo run -p openlink-models --example export_cpdlc_catalog -- spec/cpdlc/catalog.v1.json`

`cargo run -p openlink-models --example generate_cpdlc_reference -- spec/cpdlc/catalog.v1.json docs/sdk/reference/cpdlc-messages.md`
