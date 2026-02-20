---
title: Integration Checklist
description: Mirrored documentation from docs/sdk/integration-checklist.md
sidebar:
  order: 9
---

> Source: docs/sdk/integration-checklist.md (synced automatically)

# Integration checklist (aircraft / ATC)

## Purpose

Use this list as a go-live gate before production rollout.

## A) Startup and connectivity

- [ ] Authentication is valid
- [ ] NATS connection opens successfully
- [ ] Inbox subscription is active before first send

## B) Send and receive contract

- [ ] Client publishes only to outbox subject
- [ ] Inbound envelopes are parsed safely
- [ ] Transport errors are exposed clearly

## C) CPDLC compliance

- [ ] Message IDs and metadata validated with `spec/cpdlc/catalog.v1.json`
- [ ] Argument count/types validated before send
- [ ] Response rules enforced (attribute, closing, suggestions)

## D) UX behavior

- [ ] Dialogue/message states are visible and correct
- [ ] Suggested responses appear where expected
- [ ] Keyboard flow works (focus, tab order, submit)

## E) Presence correctness

- [ ] `online` is published once station is ready
- [ ] `offline` is published on graceful shutdown
- [ ] station/address/callsign mapping remains coherent

## F) Resilience and recovery

- [ ] reconnect + re-subscribe flow tested
- [ ] post-reconnect state recovery tested
- [ ] no silent message loss on short disconnects

## Related pages

- [Conformance profile](../conformance-profile/)
- [Stations and presence](../stations-presence/)
