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
- an ATC product (controller client, ground station software).

The goal is to provide a clear integration path without requiring server-internal knowledge.

## Recommended reading path

1. [General concepts](concepts/)
2. [Integration architecture](integration-architecture/)
3. [NATS transport](nats-transport/)
4. [Envelopes and message stack](envelopes-and-stack/)
5. [Addressing and routing](addressing-routing/)
6. [Stations and presence (online/offline)](stations-presence/)
7. [Raw NATS quickstart](quickstart-raw-nats/)
8. [High-level API contract](high-level-api-contract/)
9. [Conformance profile](conformance-profile/)
10. [Integration checklist](integration-checklist/)
11. [CPDLC reference](reference/)

## Design principle

- Protocol truth is provided by the catalog: `spec/cpdlc/catalog.v1.json`.
- High-level SDK integrations and low-level raw NATS integrations should use the same catalog.
- Reference documentation is generated to stay aligned with the model.

## Regenerating reference documentation

From repository root:

`cargo run -p openlink-models --example export_cpdlc_catalog -- spec/cpdlc/catalog.v1.json`

`cargo run -p openlink-models --example generate_cpdlc_reference -- spec/cpdlc/catalog.v1.json docs/sdk/reference/cpdlc-messages.md`
