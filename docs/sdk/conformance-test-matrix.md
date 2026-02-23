# SDK conformance test matrix

This page defines the **complete validation set** to declare an SDK or integration conformant.

It complements:

- [Conformance profile](conformance-profile.md)
- [Polyglot SDK contract](polyglot-sdk-contract.md)

## 1) Mandatory fixture-based tests

Source fixtures:

- `spec/sdk-conformance/runtime-vectors.v1.json`
- `spec/sdk-conformance/wire-examples.v1.json`

### 1.0 How to read runtime vectors

`runtime-vectors.v1.json` is not an implementation; it is the canonical expected-behavior dataset.

Every vector is structured as:

- `id`: stable test case identifier,
- `operation`: semantic runtime function to evaluate,
- `input`: operation arguments,
- `expected` or `expected_downlink_ids`: required output.

Operation mapping (semantic names):

- `is_logical_ack_element_id`
- `message_contains_logical_ack`
- `should_auto_send_logical_ack`
- `response_attr_to_intents`
- `choose_short_response_intents`
- `closes_dialogue_response_elements`

SDKs may use different language-specific naming, but semantics must match exactly.

### 1.1 Runtime rule parity (MUST)

For each vector, assert exact expected result:

- logical ACK helpers:
  - `is_logical_ack_element_id`
  - `message_contains_logical_ack`
  - `should_auto_send_logical_ack`
- short response logic:
  - `response_attr_to_intents`
  - `choose_short_response_intents`
- dialogue closing logic:
  - `closes_dialogue_response_elements`

Pass criteria:

- 100% vectors pass.
- Returned downlink IDs preserve expected order.
- No fallback behavior different from vector expectations.

### 1.2 Wire compatibility (MUST)

For each wire example:

1. deserialize JSON into SDK types,
2. serialize back to JSON,
3. compare semantically (same fields/values, ignoring key order).

Pass criteria:

- 100% examples round-trip without loss.
- No type coercion changing domain values (`min`, `mrn`, IDs, enum tags).

## 2) Catalog compliance tests (MUST)

Using `spec/cpdlc/catalog.v1.json`:

- unknown message IDs are rejected,
- invalid argument count/type is rejected,
- invalid direction usage is rejected,
- response metadata is available for each valid element.

Pass criteria:

- tests include positive and negative cases,
- errors are actionable and identify the invalid field.

## 3) Protocol behavior tests (MUST)

### 3.1 Logical ACK behavior

- ACK emitted only when `min > 0`.
- ACK never ACKs logical ACK elements (`DM100`/`UM227`).
- ACK uses `mrn = received.min`.

### 3.2 Short response behavior

- precedence is $WU > AN > R > Y > N$ (`NE` behaves as `N`),
- fallback behavior is deterministic when definition is missing.

### 3.3 Dialogue closing behavior

- closing intents close dialogues unless standby element is present.

## 4) Integration behavior tests (MUST)

### 4.1 Connection lifecycle

- authenticated connect succeeds,
- inbox subscription is active before first send,
- reconnect restores subscriptions.

### 4.2 Send/receive contract

- outbox subject usage is correct,
- inbound envelope parsing is resilient,
- transport/protocol errors are surfaced to integrator.

### 4.3 Presence and station state

- `online` published on readiness,
- `offline` published on graceful shutdown,
- station identity mapping stays coherent.

## 5) Cross-language parity checks (MUST)

For every language SDK:

- run the same runtime vectors,
- run the same wire examples,
- compare outputs against canonical expectations.

Pass criteria:

- no undocumented divergence.
- any intentional divergence requires contract/doc updates before release.

## 6) Release gate (MUST)

An SDK release is conformant only if all below are true:

1. fixture-based tests pass,
2. catalog compliance tests pass,
3. protocol/integration behavior tests pass,
4. compatible catalog version is documented,
5. changelog includes compatibility notes.
