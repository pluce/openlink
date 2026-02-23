# Develop a new OpenLink SDK

This guide defines the expected path to build a compliant SDK in a new language.

## Goal

Implement a language SDK that is:

- wire-compatible,
- behavior-compatible,
- and contract-compatible with existing OpenLink SDKs.

## Recommended implementation order

1. **Wire models**
   - implement OpenLink/ACARS/CPDLC JSON types,
   - verify deserialize/serialize compatibility.

2. **Transport layer**
   - implement inbox/outbox subject conventions,
   - add connect/subscribe/publish/disconnect APIs.

3. **Catalog integration**
   - load CPDLC catalog,
   - validate IDs, direction, arguments,
   - expose rendering helpers.

4. **Runtime protocol helpers**
   - logical-ack eligibility and builder,
   - response attribute to intents,
   - short-response selection precedence,
   - dialogue close/standby behavior.

5. **High-level workflow helpers**
   - logon/connection/handover/end-service helpers,
   - station and aircraft application helpers.

6. **Conformance suite**
   - execute shared runtime vectors,
   - execute shared wire examples,
   - add integration behavior tests.

## Mandatory conformance artifacts

- `spec/sdk-conformance/runtime-vectors.v1.json`
- `spec/sdk-conformance/wire-examples.v1.json`
- `spec/cpdlc/catalog.v1.json`

## Definition of done

A new SDK is considered compliant when all conditions in:

- [Conformance profile](conformance-profile.md)
- [Conformance test matrix](conformance-test-matrix.md)
- [Polyglot SDK contract](polyglot-sdk-contract.md)

are satisfied and versioned in release notes.

## Related pages

- [Polyglot SDK contract](polyglot-sdk-contract.md)
- [High-level API contract](high-level-api-contract.md)
- [TypeScript SDK compliance profile](typescript-sdk-compliance.md)
- [CPDLC reference](reference/README.md)
