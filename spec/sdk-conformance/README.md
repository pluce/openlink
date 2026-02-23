# SDK Conformance Fixtures

Machine-readable fixtures for validating SDK conformance across languages.

## What are "runtime vectors"?

`runtime-vectors.v1.json` is a list of deterministic test cases for SDK runtime rules.

Each vector contains:

- `operation`: the runtime rule to execute,
- `input`: the payload to pass to that rule,
- `expected` or `expected_downlink_ids`: the exact expected result.

Goal: every SDK (Rust, TypeScript, or another language) produces the same outputs for the same vectors.

## How to execute vectors in a test suite

1. Load `runtime-vectors.v1.json`.
2. Iterate every vector in each section (`logical_ack`, `response_attr`, `short_response_selection`, `dialogue_close`).
3. Dispatch by `operation` to the corresponding SDK function.
4. Assert exact equality with the expected value.

Notes:

- For `expected_downlink_ids`, compare ordered lists.
- Any mismatch is a conformance failure.

## Files

- `runtime-vectors.v1.json`: runtime rule vectors (short-response selection, logical-ack eligibility, dialogue close rule).
- `wire-examples.v1.json`: canonical OpenLink envelope JSON examples for serialization/deserialization compatibility.

## Intended use

Each SDK implementation should load these fixtures in its test suite and assert identical outcomes.

Normative execution and pass criteria are documented in:

- `docs/sdk/conformance-test-matrix.md`

## Versioning

- Backward-compatible additions: append vectors/examples.
- Breaking semantic changes: bump file version suffix (`v2`) and update docs in `docs/sdk/*`.
