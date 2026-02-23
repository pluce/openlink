---
title: Conformance Profile
description: Mirrored documentation from docs/sdk/conformance-profile.md
sidebar:
  order: 16
---

> Source: docs/sdk/conformance-profile.md (synced automatically)

# SDK conformance profile

## Purpose

Define acceptance criteria for SDKs used by external integrators.

## Mandatory criteria

### 1) Message compliance

- Message IDs exist in catalog
- Direction is valid
- Arguments are valid in count/type

### 2) Response compliance

- Response attribute is respected
- Short responses allowed only when supported
- Constrained closing replies surfaced when defined

### 3) Transport compliance

- Envelope format is compatible
- NATS subjects follow convention
- Reconnect behavior is stable

### 4) Integrator experience

- Errors are explicit and actionable
- API surface is coherent and documented
- SDK docs declare compatible catalog version

### 5) Cross-language parity

- Rust and TypeScript SDK runtime rules remain behaviorally aligned
- Logical acknowledgement and short-response selection use equivalent rules
- Any intentional divergence is documented in both SDK docs and changelogs

### 6) Fixture conformance

- Runtime vectors are machine-readable expected-behavior cases (`operation` + `input` + expected output)
- SDKs execute shared runtime vectors from `spec/sdk-conformance/runtime-vectors.v1.json`
- SDKs execute shared wire examples from `spec/sdk-conformance/wire-examples.v1.json`
- All vectors/examples pass with exact expected outcomes

## Recommended test coverage

- valid/invalid message construction,
- dialogue transitions (open/respond/close),
- suggested and constrained response behavior,
- reconnect/recovery and subscription restoration.

## Release and versioning rules

- Every SDK release should declare target catalog version.
- Any breaking behavior change should be clearly marked.

## Related pages

- [High-level API contract](../high-level-api-contract/)
- [Integration checklist](../integration-checklist/)
- [Conformance test matrix](../conformance-test-matrix/)
