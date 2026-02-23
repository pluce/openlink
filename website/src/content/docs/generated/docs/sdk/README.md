---
title: SDK Integrator Guide
description: Mirrored documentation from docs/sdk/README.md
slug: generated/docs/sdk
sidebar:
  order: 1
---

> Source: docs/sdk/README.md (synced automatically)

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

See [OpenLink capabilities](capabilities/).

## How OpenLink works (architecture and concepts)

Core reading order:

1. [General concepts](concepts/)
2. [Integration architecture](integration-architecture/)
3. [NATS transport](nats-transport/)
4. [Envelopes and message stack](envelopes-and-stack/)
5. [Addressing and routing](addressing-routing/)
6. [Stations and presence (online/offline)](stations-presence/)

## Integrating OpenLink with existing applications

Recommended path for product teams:

1. [Integrate with SDKs](integrate-with-sdks/)
2. [High-level API contract](high-level-api-contract/)
3. [Integration checklist](integration-checklist/)
4. [Conformance profile](conformance-profile/)
5. [Conformance test matrix](conformance-test-matrix/)

If needed, use [Raw NATS quickstart](quickstart-raw-nats/).

## Developing a new SDK

Start here:

1. [Develop a new SDK](develop-new-sdk/)
2. [Polyglot SDK contract](polyglot-sdk-contract/)
3. [TypeScript SDK compliance profile](typescript-sdk-compliance/)
4. [Conformance profile](conformance-profile/)
5. [Conformance test matrix](conformance-test-matrix/)
6. [CPDLC reference](reference/)

## Design principle

- Protocol truth is provided by the catalog: `spec/cpdlc/catalog.v1.json`.
- High-level SDK integrations and low-level raw NATS integrations should use the same catalog.
- Reference documentation is generated to stay aligned with the model.

## Regenerating reference documentation

From repository root:

`cargo run -p openlink-models --example export_cpdlc_catalog -- spec/cpdlc/catalog.v1.json`

`cargo run -p openlink-models --example generate_cpdlc_reference -- spec/cpdlc/catalog.v1.json docs/sdk/reference/cpdlc-messages.md`
