---
title: Concepts
description: Mirrored documentation from docs/sdk/concepts.md
sidebar:
  order: 2
---

> Source: docs/sdk/concepts.md (synced automatically)

# General concepts

## Purpose

Explain the minimum concepts an aircraft or ATC integrator needs before implementing OpenLink.

## Audience

- Aircraft software teams (EFB, DCDU, CDU/FMC integrations)
- ATC software teams (controller client, station software)

## What you are integrating

OpenLink carries CPDLC exchanges between aircraft and ground stations over NATS.

In most products, integration means:

1. A transport adapter (NATS connection + inbox/outbox handling)
2. A message layer (OpenLink/ACARS/CPDLC JSON)
3. A UI workflow (message display, reply actions, dialogue state)

## Architecture at a glance

OpenLink is built around a clear split between infrastructure responsibilities and product responsibilities.

### What OpenLink infrastructure does

- authenticates participants and provides runtime identity,
- routes messages via network subjects,
- maintains server-authoritative CPDLC session state,
- broadcasts session updates to participants,
- provides protocol catalog/reference used by SDKs and raw integrations.

Business logic handled by the server includes:

- CPDLC session lifecycle (logon, connection, transfer, end-service),
- authoritative active/inactive connection state for each participant,
- dialogue state transitions (opened/waiting/closed based on exchanged messages),
- MIN/MRN assignment/validation and cross-message consistency,
- session snapshot replay on reconnect/online,
- server-side normalization or protocol safety rules when required.

### What integrators must implement

- connect product runtime to NATS,
- publish outbound envelopes and consume inbound envelopes,
- render operational UI workflows (ATC/DCDU, dialogue status, reply actions),
- enforce local validation and user flow guardrails,
- handle reconnect and restore local runtime behavior.

In short: clients should **project** protocol state, not **own** it.
Your client consumes authoritative session updates from OpenLink and adapts UI behavior accordingly.

### End-to-end flow (simplified)

1. Client authenticates and connects to NATS.
2. Client subscribes its inbox subject.
3. Client sends CPDLC payload wrapped in ACARS/OpenLink envelope.
4. OpenLink server validates/routes and updates authoritative session state.
5. Destination client receives message and session updates.

## Integration levels

### Level 1: High-level SDK (recommended)

Use SDK helpers for common flows and protocol validation.

Best when you want:

- faster implementation,
- fewer protocol mistakes,
- lower maintenance cost.

Important: this level is available only for languages/platforms where an OpenLink SDK exists.

### Level 2: Raw NATS

Build and parse JSON envelopes directly.

Best when you need:

- full control over transport/runtime behavior,
- custom platform constraints.

Raw mode still requires strict validation using `spec/cpdlc/catalog.v1.json`.

## What is NATS (and why OpenLink uses it)

NATS is a high-performance messaging system based on subjects and publish/subscribe.

In OpenLink, NATS is used as the transport bus between clients and server components.
Clients publish to outbox subjects and subscribe to inbox subjects.

Useful documentation:

- NATS concepts: https://docs.nats.io/nats-concepts/overview
- Core publish/subscribe: https://docs.nats.io/nats-concepts/core-nats/pubsub
- Subject naming: https://docs.nats.io/nats-concepts/subjects
- Official examples (multi-language): https://examples.nats.io/

## Key identity model

You must track two identity spaces at the same time:

- **Operational identity** (callsign + ACARS address)
- **Runtime identity** (network + runtime address on NATS)

Example:

- Operational: `AFR123` / `AY213`
- Runtime: `demonetwork` / `CID_AFR123`

## Dialogues and responses

A CPDLC dialogue is a request/response sequence.

From the catalog, you can derive:

- expected response type (`response_attr`),
- short-response options,
- closing-response behavior,
- constrained suggested reply sets.

OpenLink provides the protocol contract and server-side state authority; integrators apply these rules in product UX.

Practical implication for client implementation:

- use message `response_attr` to show valid user actions,
- use authoritative session/dialogue updates to drive state badges and interaction locks,
- avoid local protocol state machines that can diverge from server truth.

## What stays outside client scope

A standard client integration should not re-implement server internals.

Focus on:

- sending valid envelopes,
- rendering messages clearly,
- enforcing user input and response rules from the catalog.

## Related pages

- [Integration architecture](../integration-architecture/)
- [Envelopes and message stack](../envelopes-and-stack/)
- [NATS transport](../nats-transport/)
