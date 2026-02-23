---
title: Integrate with SDKs
description: Mirrored documentation from docs/sdk/integrate-with-sdks.md
sidebar:
  order: 9
---

> Source: docs/sdk/integrate-with-sdks.md (synced automatically)

# Integrate OpenLink with SDKs

Use this page as the practical entry point for application integration.

## Target audience

- aircraft clients (EFB, DCDU, FMC/CDU integrations),
- ATC/controller clients,
- backend services consuming OpenLink envelopes.

## Integration flow

1. **Establish connectivity**
   - authenticate against OpenLink auth,
   - connect to NATS,
   - subscribe inbox before first send.

2. **Send/receive OpenLink envelopes**
   - publish only to outbox subject,
   - parse inbound envelopes safely,
   - expose transport/protocol errors explicitly.

3. **Use CPDLC helpers**
   - logon/connection/handover helpers,
   - response-intent and logical-ack helpers,
   - message/catalog validation before send.

4. **Project session and presence into UX**
   - consume session update events,
   - render dialogue and message states,
   - surface short responses and constrained replies.

5. **Run conformance checks before go-live**
   - integration checklist,
   - conformance profile,
   - fixture-based conformance matrix.

## Minimum implementation checklist

- [ ] transport connected and resilient to reconnect,
- [ ] inbox/outbox subject usage compliant,
- [ ] CPDLC IDs/arguments validated from catalog,
- [ ] logical-ack and short-response rules delegated to SDK runtime,
- [ ] conformance fixtures pass.

## Related pages

- [High-level API contract](../high-level-api-contract/)
- [Integration checklist](../integration-checklist/)
- [Conformance profile](../conformance-profile/)
- [Conformance test matrix](../conformance-test-matrix/)
- [Raw NATS quickstart](../quickstart-raw-nats/)
