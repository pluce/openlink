---
title: Polyglot SDK Contract
description: Mirrored documentation from docs/sdk/polyglot-sdk-contract.md
sidebar:
  order: 14
---

> Source: docs/sdk/polyglot-sdk-contract.md (synced automatically)

# Polyglot SDK Contract (Language-neutral)

This document is the canonical base for building OpenLink SDKs in **any language**.

Use it as the source contract for Rust, TypeScript, and future SDKs (Go, Java, C#, Python, etc.).

## 1. Compatibility model

Every SDK implementation MUST be compatible on three axes:

- **Wire compatibility**: JSON payloads and envelope structure match OpenLink models.
- **Behavior compatibility**: runtime protocol decisions produce equivalent outcomes.
- **API intent compatibility**: helper methods map to the same CPDLC operations.

## 2. Canonical runtime symbols

SDKs SHOULD expose these canonical runtime operations (exact naming optional, semantics mandatory):

- `LOGICAL_ACK_DOWNLINK_ID` = `DM100`
- `LOGICAL_ACK_UPLINK_ID` = `UM227`
- `is_logical_ack_element_id(id)`
- `message_contains_logical_ack(elements)`
- `should_auto_send_logical_ack(elements, min)`
- `response_attr_to_intents(attr)`
- `choose_short_response_intents(elements, catalog_lookup)`
- `closes_dialogue_response_elements(elements)`
- `cpdlc_logical_ack(aircraft, sender, receiver, mrn)`

## 3. Canonical high-level client capabilities

SDKs SHOULD provide high-level helpers equivalent to:

- `connect_with_authorization_code(...)`
- `subscribe_inbox(...)`
- `send_to_server(...)`
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

## 4. Required protocol rules

- Short response precedence: $WU > AN > R > Y > N$ (`NE` behaves as `N`).
- Logical acknowledgement must not acknowledge logical acknowledgements.
- Logical acknowledgement must reference received message `MIN` via `MRN`.
- MIN domain awareness: valid operational range is `1..63`.

## 5. Conformance validation

A new SDK for another language MUST include tests that prove:

- identical response-intent outcomes for shared fixture messages,
- identical logical-ack eligibility outcomes,
- identical dialogue close/standby outcomes,
- envelope and CPDLC payload JSON compatibility.

Canonical fixture assets:

- `spec/sdk-conformance/runtime-vectors.v1.json`
- `spec/sdk-conformance/wire-examples.v1.json`

## 6. Drift management

If one SDK changes runtime semantics:

1. update this contract,
2. update all SDK-specific compliance docs,
3. add migration notes.

Related:
- [Conformance profile](../conformance-profile/)
- [Conformance test matrix](../conformance-test-matrix/)
- [TypeScript SDK compliance profile](../typescript-sdk-compliance/)
- [High-level API contract](../high-level-api-contract/)
