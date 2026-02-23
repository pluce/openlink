---
title: SDK Reference
description: Mirrored documentation from docs/sdk/reference/README.md
slug: generated/docs/sdk/reference
sidebar:
  order: 18
---

> Source: docs/sdk/reference/README.md (synced automatically)

# CPDLC Reference

This section is generated from `spec/cpdlc/catalog.v1.json`.

Use it when you need protocol-level lookup details:

- message identifiers,
- templates and arguments,
- response attributes and behavior metadata.

It is the canonical reference used by both integration and SDK conformance work.

- [CPDLC message reference](cpdlc-messages/)

Related pages:

- [Integrate with SDKs](../integrate-with-sdks/)
- [Develop a new SDK](../develop-new-sdk/)
- [Conformance test matrix](../conformance-test-matrix/)

Regenerate:

`cargo run -p openlink-models --example generate_cpdlc_reference -- spec/cpdlc/catalog.v1.json docs/sdk/reference/cpdlc-messages.md`
