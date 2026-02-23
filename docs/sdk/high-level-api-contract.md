# High-level SDK API contract

## Purpose

Define the expected surface of an SDK intended for production aircraft/ATC integrators.

This contract is language-neutral and complements the dedicated
[Polyglot SDK contract](polyglot-sdk-contract.md).

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

## Server authority and SDK boundary

The SDK contract should clearly reflect server-authoritative protocol behavior.

### Server-side business logic (OpenLink)

OpenLink server remains the source of truth for:

- CPDLC session state,
- dialogue progression and closure state,
- protocol-level consistency across participants.

### Client-side SDK behavior

The SDK should help integrators:

- consume authoritative session updates,
- map protocol state into UI-friendly models,
- avoid duplicating protocol state machines locally.

SDK APIs should make this boundary explicit so product teams implement UX logic, not protocol ownership.

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

## Canonical CPDLC helper set (recommended)

- `cpdlc_logon_request(...)`
- `cpdlc_logon_response(...)`
- `cpdlc_connection_request(...)`
- `cpdlc_connection_response(...)`
- `cpdlc_next_data_authority(...)`
- `cpdlc_contact_request(...)`
- `cpdlc_end_service(...)`
- `cpdlc_logon_forward(...)`
- `cpdlc_station_application(...)`
- `cpdlc_aircraft_application(...)`
- `cpdlc_logical_ack(...)`

## Related pages

- [Conformance profile](conformance-profile.md)
- [Polyglot SDK contract](polyglot-sdk-contract.md)
- [Raw NATS integration quickstart](quickstart-raw-nats.md)
