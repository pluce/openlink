# TypeScript SDK Compliance Profile

This profile defines the **minimum required content** for an OpenLink TypeScript SDK to be considered compliant.

It is a specialization of the language-neutral [Polyglot SDK contract](polyglot-sdk-contract.md).

## Cross-SDK parity target (Rust ↔ TypeScript)

A compliant TypeScript SDK SHOULD stay as close as possible to Rust SDK semantics for runtime rules.

### Required parity domains

- logical acknowledgement behavior
- short-response selection behavior
- response-attribute precedence behavior
- dialogue close/standby behavior

### Recommended symbol parity

| Rust SDK | TypeScript SDK |
|---|---|
| `LOGICAL_ACK_DOWNLINK_ID` | `logicalAckDownlinkId()` |
| `LOGICAL_ACK_UPLINK_ID` | `logicalAckUplinkId()` |
| `is_logical_ack_element_id()` | `isLogicalAckElementId()` |
| `message_contains_logical_ack()` | `messageContainsLogicalAck()` |
| `should_auto_send_logical_ack()` | `shouldAutoSendLogicalAck()` |
| `closes_dialogue_response_elements()` | `closesDialogueResponseElements()` |
| `response_attr_to_intents()` | `responseAttrToIntents()` |
| `choose_short_response_intents()` | `chooseShortResponseIntents()` |
| `cpdlc_logical_ack()` | `buildLogicalAckForSender()` |

TypeScript SDKs SHOULD expose both:

- idiomatic `camelCase` API,
- parity aliases compatible with canonical/runtime naming (`snake_case`).

## 1. Transport layer

A compliant SDK MUST provide:

- NATS subject helpers:
  - `openlink.v1.{network}.outbox.{address}`
  - `openlink.v1.{network}.inbox.{address}`
- A client connector for browser/WebSocket transport.
- Envelope publish + inbox subscribe APIs.
- Graceful disconnect (`unsubscribe` + `drain`).

## 2. Authentication integration

A compliant SDK MUST provide:

- OIDC code exchange helper against auth service.
- JWT-based connection setup for NATS.
- Access to resolved identity (`cid`) and runtime network address.

## 3. Wire-format types

A compliant SDK MUST expose typed models matching Rust serialization:

- `OpenLinkEnvelope`, `OpenLinkMessage`
- `AcarsEnvelope`, `CpdlcEnvelope`
- `CpdlcMessageType` (`Meta` / `Application`)
- `CpdlcApplicationMessage` (`min`, `mrn`, `elements`, `timestamp`)
- `CpdlcMetaMessage` (`LogonRequest`, `LogonResponse`, `ConnectionRequest`, `ConnectionResponse`, `LogonForward`, `SessionUpdate`)
- `CpdlcSessionView`

## 4. Message builders

A compliant SDK MUST include builders for:

- envelope creation (`source`, `destination`, `token`)
- station presence (`ONLINE`, `OFFLINE`)
- CPDLC logon/connection messages
- generic CPDLC application downlink
- short response downlink (`DM0..DM5`)
- logical acknowledgement downlink (`DM100` with `MRN = referenced MIN`)

## 5. Catalog-driven rendering

A compliant SDK MUST support catalog-based transformation:

- load catalog entries (`catalog.v1.json`)
- render message elements to display text parts (static/parameter split)
- plain string fallback rendering

## 6. Runtime protocol decisions

A compliant SDK MUST centralize protocol decisions that would otherwise be duplicated across UIs:

- response attribute priority selection: `WU > AN > R > Y > N` (`NE` treated as `N`)
- short response choice derivation from catalog
- logical acknowledgement auto-send eligibility
- standby/closing response helpers

## 7. Safety rules

A compliant SDK MUST enforce:

- no logical-ack loops (`DM100`/`UM227` are never auto-acked)
- `MRN` references use message `MIN` of received message
- message MIN domain awareness (`1..63`) even when sender uses placeholder `min=0`

## 8. Recommended package layout

- `types.ts`
- `envelope.ts`
- `catalog.ts`
- `cpdlc-runtime.ts`
- `nats-client.ts`
- `index.ts` (public exports)

## 9. Conformance test checklist

A compliant SDK SHOULD include automated tests for:

- envelope serialization compatibility with Rust models
- response selection for multi-element messages
- logical-ack eligibility and loop prevention
- template argument rendering (`Level`, `FreeText`, etc.)
- connection/auth happy-path + auth failure handling

## 10. Drift policy

If Rust and TypeScript SDKs diverge in runtime behavior, the change MUST be documented in both SDK changelogs and this profile MUST be updated.
