# SDK conformance profile

## Purpose

Define acceptance criteria for SDKs used by external integrators.

## Mandatory criteria

### 1) Message compliance

- Message IDs exist in catalog
- Direction is valid
- Arguments are valid in count/type

### 2) Response compliance

- Response attribute is respected
- Short responses allowed only when supported
- Constrained closing replies surfaced when defined

### 3) Transport compliance

- Envelope format is compatible
- NATS subjects follow convention
- Reconnect behavior is stable

### 4) Integrator experience

- Errors are explicit and actionable
- API surface is coherent and documented
- SDK docs declare compatible catalog version

## Recommended test coverage

- valid/invalid message construction,
- dialogue transitions (open/respond/close),
- suggested and constrained response behavior,
- reconnect/recovery and subscription restoration.

## Release and versioning rules

- Every SDK release should declare target catalog version.
- Any breaking behavior change should be clearly marked.

## Related pages

- [High-level API contract](high-level-api-contract.md)
- [Integration checklist](integration-checklist.md)
