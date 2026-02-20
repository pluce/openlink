---
title: OpenLink Overview
description: Why OpenLink, architecture, and full workspace overview.
slug: generated/overview
sidebar:
  order: 1
---

> Source: docs/website/openlink-overview-intro.md + README.md (synced automatically)

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


# OpenLink Reference Workspace

OpenLink is a Rust-based reference implementation for ACARS/CPDLC messaging over NATS.

This repository provides:

- protocol/domain models,
- a reusable SDK,
- authentication and routing services,
- demo clients (CLI and GUI),
- language-agnostic protocol artifacts for external integrators.

## Architecture summary

OpenLink uses an event-driven architecture on top of NATS subjects.

- Clients publish outbound envelopes to `openlink.v1.{network}.outbox.{address}`.
- The server subscribes wildcard outbox subjects, applies protocol/session logic, and forwards to destination inbox subjects.
- Clients subscribe their own inbox subject and update local UI/application state.

Important design rule:

- **Server side is authoritative** for CPDLC protocol/session truth (connection lifecycle, dialogue state, session snapshots).
- **Client side** focuses on transport integration, validation, and user workflow projection.

For a complete integrator guide, see [docs/sdk/README.md](../docs/sdk/).

## Workspace layout

### Core crates

| Crate | Path | Purpose |
|---|---|---|
| `openlink-models` | [crates/openlink-models](crates/openlink-models) | Canonical protocol/domain types and message builders (OpenLink, ACARS, CPDLC). |
| `openlink-sdk` | [crates/openlink-sdk](crates/openlink-sdk) | High-level client API: auth exchange, NATS connection, subject conventions, send/subscribe helpers. |
| `openlink-server` | [crates/openlink-server](crates/openlink-server) | Relay/router service. Handles station registry, CPDLC session state machine, forwarding, session updates. |
| `openlink-auth` | [crates/openlink-auth](crates/openlink-auth) | Auth gateway. Exchanges OIDC authorization codes for scoped NATS JWTs. |

### Demo / tooling crates

| Crate | Path | Purpose |
|---|---|---|
| `openlink-cli` | [crates/openlink-cli](crates/openlink-cli) | Scriptable CLI client for CPDLC scenarios, protocol testing, and automation. |
| `openlink-loadtest` | [crates/openlink-loadtest](crates/openlink-loadtest) | Load generator and benchmark tool (throughput/latency, multiple scenarios and scales). |
| `openlink-gui` | [crates/openlink-gui](crates/openlink-gui) | Dioxus desktop demonstrator (ATC and DCDU views). |
| `mock-oidc` | [crates/mock-oidc](crates/mock-oidc) | Local OIDC provider simulator used in development. |
| `website` | [website](website) | Astro + Starlight product website and synchronized documentation portal. |

### Specs and documentation

| Path | Purpose |
|---|---|
| [spec](spec) | Language-agnostic protocol artifacts (CPDLC catalog JSON + schema). |
| [docs/sdk](docs/sdk) | Integrator documentation: concepts, architecture, transport, envelopes, checklist, reference. |

## Prerequisites

- Rust toolchain (stable) with `cargo`
- Docker / Docker Compose

## Quick start (full local stack)

Run from repository root.

### 1) Start NATS

```bash
docker compose up -d
```

Default ports:

- `4222`: NATS client connections
- `8222`: NATS monitoring

### 2) Start authentication services (separate terminals)

Mock OIDC provider:

```bash
cargo run -p mock-oidc
```

OpenLink auth service:

```bash
cargo run -p openlink-auth
```

### 3) Start OpenLink server

```bash
cargo run -p openlink-server
```

Optional clean start (reset JetStream KV buckets):

```bash
cargo run -p openlink-server -- --clean
```

### 4) Start a client

GUI demonstrator:

```bash
cargo run -p openlink-gui
```

or CLI client (examples below).

## CLI usage examples

### Bring stations online

Pilot station online:

```bash
cargo run -p openlink-cli -- \
  --network-id demonetwork --network-address PILOT \
  acars --callsign AFR123 --address AY213 \
  online
```

ATC station online:

```bash
cargo run -p openlink-cli -- \
  --network-id demonetwork --network-address ATC \
  acars --callsign LFPG --address LFPGCYA \
  online
```

### Listen for CPDLC messages

ATC listener:

```bash
cargo run -p openlink-cli -- \
  --network-id demonetwork --network-address ATC \
  acars --callsign LFPG --address LFPGCYA \
  cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --atc \
  listen
```

Pilot listener:

```bash
cargo run -p openlink-cli -- \
  --network-id demonetwork --network-address PILOT \
  acars --callsign AFR123 --address AY213 \
  cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --pilot \
  listen
```

### Send sample CPDLC flow

Pilot sends logon request:

```bash
cargo run -p openlink-cli -- \
  --network-id demonetwork --network-address PILOT \
  acars --callsign AFR123 --address AY213 \
  cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --pilot \
  send logon-request --station LFPG --origin LFPG --destination EGLL
```

ATC sends logon response:

```bash
cargo run -p openlink-cli -- \
  --network-id demonetwork --network-address ATC \
  acars --callsign LFPG --address LFPGCYA \
  cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --atc \
  send logon-response --accepted
```

## Build, check, and test

Workspace check:

```bash
cargo check
```

Check specific crate:

```bash
cargo check -p openlink-gui
```

Run all tests:

```bash
cargo test
```

Run tests for one crate:

```bash
cargo test -p openlink-server
```

## Protocol catalog and generated reference

OpenLink exports a language-agnostic CPDLC catalog used by docs and external SDKs.

Export catalog JSON:

```bash
cargo run -p openlink-models --example export_cpdlc_catalog -- spec/cpdlc/catalog.v1.json
```

Generate markdown message reference:

```bash
cargo run -p openlink-models --example generate_cpdlc_reference -- spec/cpdlc/catalog.v1.json docs/sdk/reference/cpdlc-messages.md
```

See:

- [spec/README.md](../spec/)
- [docs/sdk/reference/README.md](../docs/sdk/reference/)

## Important environment variables

Most components work with defaults, but these are commonly overridden:

- `NATS_URL` (default `nats://localhost:4222`)
- `AUTH_URL` (default `http://localhost:3001`)
- `SERVER_SECRET` (default `openlink-dev-secret`)
- `AUTH_PORT` for auth service (default `3001`)
- `OIDC_DEMONETWORK_TOKEN_URL` for auth-to-OIDC exchange (default `http://localhost:4000/token`)
- `RUST_LOG` for log filtering

## Troubleshooting

- If auth fails, ensure `mock-oidc` and `openlink-auth` are running.
- If routing fails, ensure `openlink-server` is running and connected to NATS.
- If state looks stale in demos, restart server with `--clean` to reset JetStream KV.
- If GUI cannot connect, verify `NATS_URL` and `AUTH_URL` match your local setup.

## Additional documentation

- Integrator guide: [docs/sdk/README.md](../docs/sdk/)
- Crate-level docs:
  - [crates/openlink-models/README.md](../crates/openlink-models/)
  - [crates/openlink-sdk/README.md](../crates/openlink-sdk/)
  - [crates/openlink-auth/README.md](../crates/openlink-auth/)
  - [crates/openlink-server/README.md](../crates/openlink-server/)
  - [crates/openlink-cli/README.md](../crates/openlink-cli/)
  - [crates/openlink-gui/README.md](../crates/openlink-gui/)
  - [crates/mock-oidc/README.md](../crates/mock-oidc/)
