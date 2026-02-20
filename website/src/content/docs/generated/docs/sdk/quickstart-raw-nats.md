---
title: Quickstart (Raw NATS)
description: Mirrored documentation from docs/sdk/quickstart-raw-nats.md
sidebar:
  order: 8
---

> Source: docs/sdk/quickstart-raw-nats.md (synced automatically)

# Raw NATS integration quickstart

## Purpose

Provide a minimal end-to-end path for teams integrating directly on transport.

## When to use this mode

Use raw mode when you need full control of NATS, serialization, and runtime behavior.

## Prerequisites

- NATS client library in your language,
- JSON serialization/deserialization,
- CPDLC catalog file: `spec/cpdlc/catalog.v1.json`.

## Step-by-step flow

1. Authenticate and connect to NATS
2. Subscribe to your inbox subject
3. Build a valid CPDLC payload from catalog IDs/arguments
4. Wrap CPDLC in ACARS, then OpenLink envelope
5. Publish to your outbox subject
6. Parse and process inbound inbox messages

## Validation you must enforce

- message ID exists in catalog,
- argument count and types are valid,
- response attribute is respected,
- dialogue closing/suggestion rules are applied on replies.

## Server-authoritative behavior to account for

Even in raw mode, clients must assume OpenLink server is authoritative for session/dialogue state.

Client guidance:

- do not persist an independent protocol state machine as source of truth,
- consume and apply incoming session updates as canonical state,
- treat local state as UI projection/cache only,
- reconcile UI after reconnect using server-provided snapshots.

## Minimal scenario example

- Pilot sends a CPDLC request using valid message ID and arguments.
- ATC receives it from inbox.
- ATC replies with a catalog-compliant response.
- Client marks dialogue state using response semantics.

## Recommended client skeleton (pseudo-code)

Use this structure as a baseline runtime loop:

```text
startup():
	creds = authenticate()
	nats = connect(creds)
	inbox_sub = subscribe(inbox_subject)
	publish_station_online()

	spawn receive_loop(inbox_sub)
	spawn send_loop()

receive_loop(sub):
	for msg in sub:
		envelope = parse_openlink_envelope(msg)
		if invalid(envelope):
			log_error_and_continue()
			continue

		cpdlc = extract_cpdlc_payload(envelope)
		update_local_dialogue_view(cpdlc)
		render_ui(cpdlc)

send_loop():
	while running:
		request = wait_user_or_system_action()
		cpdlc = build_cpdlc_from_catalog(request)
		validate(cpdlc)
		envelope = wrap_acars_and_openlink(cpdlc)
		publish(outbox_subject, envelope)

on_disconnect():
	retry_with_backoff()
	reconnect()
	resubscribe(inbox_subject)
	publish_station_online()
	restore_pending_ui_state_if_needed()
```

Implementation references:

- NATS examples by language: https://examples.nats.io/
- Auto-reconnect behavior: https://docs.nats.io/using-nats/developer/connecting/reconnect
- Publish/subscribe usage: https://docs.nats.io/nats-concepts/core-nats/pubsub

## Common mistakes

- sending CPDLC payload without envelope layers,
- skipping argument validation,
- publishing before inbox subscription is active.

## Related pages

- [NATS transport](../nats-transport/)
- [Envelopes and message stack](../envelopes-and-stack/)
