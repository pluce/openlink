---
title: Spec Reference
description: Mirrored documentation from spec/README.md
slug: generated/spec
sidebar:
  order: 1
---

> Source: spec/README.md (synced automatically)

# OpenLink Protocol Spec (Language-Agnostic)

This folder is the canonical, language-neutral source of protocol metadata for SDKs and integrators.

## Goals

- Avoid requiring Rust execution just to discover protocol rules.
- Keep SDKs (Rust/C#/TS/Python/...) aligned with one single source.
- Enable both high-level SDK integration and raw NATS integration.

## Contents

- `cpdlc/catalog.v1.json`: generated CPDLC catalog (messages, args, response rules, suggested replies).
- `cpdlc/catalog.schema.json`: JSON Schema for catalog validation.

## Regeneration

From repo root:

`cargo run -p openlink-models --example export_cpdlc_catalog -- spec/cpdlc/catalog.v1.json`

This command exports the catalog from `MESSAGE_REGISTRY` and helper functions in `openlink-models`.
