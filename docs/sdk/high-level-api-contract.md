# High-level SDK API contract

## Purpose

Define the expected surface of an SDK intended for production aircraft/ATC integrators.

## Transport capabilities

The SDK should expose:

- connect/disconnect,
- inbox subscription management,
- envelope publish/send,
- explicit transport error reporting.

## CPDLC workflow capabilities

The SDK should include helpers for common operational flows:

- logon,
- connection/contact,
- end service,
- generic operational message send.

## Compliance capabilities

The SDK should enforce or provide:

- ID and argument validation from catalog,
- response-attribute aware reply helpers,
- dialogue helpers (closing, standby, suggestions),
- catalog version exposure.

## UI-oriented capabilities

For cockpit/ATC usability, the SDK should provide:

- suggested responses by context,
- constrained closing-reply sets where defined,
- clear metadata to render dialogue status.

## Low-level escape hatch

Even with high-level APIs, integrators should be able to:

- publish/consume raw JSON envelopes,
- run explicit validation against the catalog.

## Example API shape (conceptual)

- `connect(credentials)`
- `subscribe_inbox(address)`
- `send_cpdlc(message_id, args, context)`
- `get_suggested_replies(context)`

## Related pages

- [Conformance profile](conformance-profile.md)
- [Raw NATS integration quickstart](quickstart-raw-nats.md)
