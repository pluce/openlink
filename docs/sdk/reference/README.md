# CPDLC Reference

This section is generated from `spec/cpdlc/catalog.v1.json`.

Use it when you need protocol-level lookup details:

- message identifiers,
- templates and arguments,
- response attributes and behavior metadata.

It is the canonical reference used by both integration and SDK conformance work.

- [CPDLC message reference](cpdlc-messages.md)

Related pages:

- [Integrate with SDKs](../integrate-with-sdks.md)
- [Develop a new SDK](../develop-new-sdk.md)
- [Conformance test matrix](../conformance-test-matrix.md)

Regenerate:

`cargo run -p openlink-models --example generate_cpdlc_reference -- spec/cpdlc/catalog.v1.json docs/sdk/reference/cpdlc-messages.md`
