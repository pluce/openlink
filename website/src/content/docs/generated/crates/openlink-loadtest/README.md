---
title: openlink-loadtest
description: Mirrored documentation from crates/openlink-loadtest/README.md
slug: generated/crates/openlink-loadtest
sidebar:
  order: 7
---

> Source: crates/openlink-loadtest/README.md (synced automatically)

# openlink-loadtest

Load testing tool for `openlink-server`.

## Goals

- Simulate traffic with multiple scenarios.
- Scale by number of station pairs and send rate.
- Measure:
  - throughput (send / receive msg/s)
  - latency (p50 / p95 / p99 in microseconds)

## Scenarios

- `one-way`: ATC -> Pilot CPDLC flow (forwarding latency).
- `echo`: ATC -> Pilot, Pilot replies DM0 (bidirectional throughput).
- `mixed`: one-way with mixed UM payload types (level, speed, free text).

## Example runs

```bash
# Small sanity check
cargo run -p openlink-loadtest -- \
  --scenario one-way --pairs 20 --duration-seconds 20 --rate-per-pair 5

# Higher load
cargo run -p openlink-loadtest -- \
  --scenario mixed --pairs 200 --duration-seconds 60 --rate-per-pair 20

# Echo scenario
cargo run -p openlink-loadtest -- \
  --scenario echo --pairs 100 --duration-seconds 45 --rate-per-pair 10

# Shared-ATC topology: 1 ATC handles 10 pilots
cargo run -p openlink-loadtest -- \
  --scenario one-way --pairs 200 --pilots-per-atc 10 --duration-seconds 30 --rate-per-pair 15
```

## Important flags

- `--pairs`: total number of ATC/Pilot traffic pairs.
- `--pilots-per-atc`: fan-out topology control (e.g. `10` => each ATC serves ~10 pilots).
- `--rate-per-pair`: messages per second per pair (`0` = max speed).
- `--duration-seconds`: active measurement window.
- `--settle-seconds`: grace period to drain in-flight messages.
- `--warmup-seconds`: optional warmup before measurement.
- `--preflight-timeout-seconds`: timeout for startup routing probe.
- `--skip-preflight`: disable startup probe.

## Preflight behavior

By default, a startup probe sends one routed message (ATC -> Pilot) and waits for reception.
If routing is broken (server down, stations not online, auth mismatch), the run fails fast with
an explicit preflight error instead of producing misleading `received=0` metrics.

## Dependencies

Requires local stack running:

- NATS
- `mock-oidc`
- `openlink-auth`
- `openlink-server`
