---
title: Integration Architecture
description: Mirrored documentation from docs/sdk/integration-architecture.md
sidebar:
  order: 4
---

> Source: docs/sdk/integration-architecture.md (synced automatically)

# Integration architecture

## Purpose

Provide a practical architecture blueprint for aircraft and ATC client implementations.

## Recommended architecture layers

### 1) Protocol contract layer

Use `spec/cpdlc/catalog.v1.json` as the source of truth for:

- message definitions,
- argument constraints,
- response semantics,
- suggested/constrained reply logic.

### 2) Transport adapter layer

Responsibilities:

- connect/authenticate to NATS,
- subscribe inbox,
- publish outbox,
- reconnect safely.

### 3) Message composition layer

Responsibilities:

- build/parse OpenLink envelopes,
- apply catalog validation,
- expose reusable send/reply helpers.

### 4) Product UI layer

Responsibilities:

- render inbound/outbound messages,
- support fast user workflows (search, keyboard submit, suggested replies),
- maintain lightweight local state.

## Responsibility split

### Integrator responsibilities

- implement robust transport + UI workflows,
- keep identity mapping coherent,
- enforce catalog-driven message validity.

### Infrastructure responsibilities

- route and deliver messages,
- maintain protocol session authority,
- synchronize network-level state.

## Server business logic vs client logic

### Server-side (OpenLink)

The server is the source of truth for protocol behavior:

- validates and routes CPDLC message flows,
- maintains session state (active/inactive connections, handover context),
- determines authoritative dialogue progression from exchanged messages,
- publishes session updates to clients after state mutations,
- ensures reconnecting participants can recover current session context.

### Client-side (Integrator)

The client focuses on product integration and UX:

- transport connection/subscription lifecycle,
- message rendering and command/reply input flows,
- local pre-send validation against catalog,
- operator ergonomics (search, keyboard, composition workflows),
- resilience behaviors (retry, reconnect, resubscribe).

### Implementation rule of thumb

If logic affects shared protocol truth between participants, it belongs server-side.
If logic affects local user interaction, it belongs client-side.

## Example deployment mapping

Aircraft product:

- UI module: DCDU panel
- Integration module: OpenLink adapter
- Runtime module: NATS client

ATC product:

- UI module: controller message pane
- Integration module: OpenLink adapter
- Runtime module: NATS client

## Design rule

If logic must behave identically across multiple languages, keep it in the spec/shared contract, not inside one specific UI implementation.

## Related pages

- [General concepts](../concepts/)
- [High-level API contract](../high-level-api-contract/)
