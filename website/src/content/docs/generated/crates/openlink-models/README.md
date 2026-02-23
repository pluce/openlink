---
title: openlink-models
description: Mirrored documentation from crates/openlink-models/README.md
slug: generated/crates/openlink-models
sidebar:
  order: 1
---

> Source: crates/openlink-models/README.md (synced automatically)

# OpenLink Models

Core data structures for the OpenLink protocol.

This crate defines every type exchanged on the OpenLink network — from
top-level envelopes down to individual CPDLC messages — along with builder
helpers for constructing them.

## Modules

| Module | Description |
|---|---|
| `network` | Network-level addressing: `NetworkId` (identifies a network such as *demonetwork* or *icao*) and `NetworkAddress` (identifies a station within that network). |
| `envelope` | `OpenLinkEnvelope` — the standard wrapper for all messages, carrying a UUID, timestamp, routing header and payload. |
| `acars` | ACARS layer: `AcarsEnvelope`, `AcarsRouting`, endpoint callsigns and addresses. |
| `cpdlc` | CPDLC messaging: `CpdlcEnvelope`, protocol meta messages (logon, connection, transfer, session update), application messages (including UM160/UM161/UM117/DM89/DM0 flows), `FlightLevel`, `ICAOAirportCode`, `SerializedMessagePayload`. |
| `station` | Ground-station metadata: `StationId`, `StationStatus`, `MetaMessage`. |
| `error` | `ModelError` — typed errors returned by `TryFrom` / `FromStr` implementations and builders. |
| `message_builder` | Fluent builders (`MessageBuilder`, `EnvelopeBuilder`, `CpdlcMessageBuilder`, `StationStatusBuilder`) for constructing messages and envelopes. |

## Key types

- **`OpenLinkEnvelope`** — Top-level message wrapper (`uuid::Uuid` id, `chrono` timestamp, routing, payload, auth token).
- **`OpenLinkMessage`** — Payload enum: `Acars(AcarsEnvelope)` or `Meta(MetaMessage)`.
- **`CpdlcMetaMessage`** — Protocol-only messages (logon/connection, logon-forward, session update).
- **`CpdlcMessage`** — Application-level CPDLC messages (e.g. climb-to, request-level-change, UM160 NEXT DATA AUTHORITY, UM161 END SERVICE, UM117 CONTACT, DM89 MONITORING) using typed arguments.
- **`ICAOAirportCode`** — Validated 4-letter ICAO code (strict `TryFrom` / `FromStr`).
- **`FlightLevel`** — Typed flight level (`u16`), displays as `"FL350"`, parses from `"FL350"` or `"350"`.
- **`StationStatus`** — Online / Offline with `strum` derives (`Display`, `EnumString`, `EnumIter`).

## Design choices

- **`serde`** — All types derive `Serialize` + `Deserialize`; enums use `#[serde(tag = "type", content = "data")]` (adjacently tagged).
- **Validation** — `ICAOAirportCode` and `FlightLevel` use `TryFrom` / `FromStr` for strict input validation; `ModelError` (via `thiserror`) carries context.
- **Newtype wrappers** — `NetworkId`, `NetworkAddress`, `AcarsEndpointCallsign`, `AcarsEndpointAddress`, `StationId` prevent stringly-typed mix-ups; all implement `Display`, `FromStr`, `Eq`, `Hash`.
- **Builders** — `MessageBuilder` and `EnvelopeBuilder` provide a fluent API for assembling messages without manually constructing nested structs.
