# openlink-auth

Authentication gateway for the OpenLink network. Exchanges an OIDC
authorization code (obtained from an external identity provider) for a
scoped NATS user JWT, granting the caller publish/subscribe access to
their personal subjects on the network.

## Architecture

```
┌───────────┐       ┌──────────────────┐       ┌──────────────────┐
│  Client   │──────▶│  openlink-auth   │──────▶│  OIDC Provider   │
│ (GUI/CLI) │  POST │  /exchange       │  POST │  /token          │
│           │◀──────│                  │◀──────│  (mock-oidc)     │
│           │  JWT  │  sign NATS JWT   │  CID  │                  │
└───────────┘       └──────────────────┘       └──────────────────┘
```

### Modules

| Module      | Description |
|-------------|-------------|
| `main.rs`   | Axum HTTP server — routes, shared state, entry point. |
| `config.rs` | `AppConfig` — maps each `NetworkId` to its OIDC provider parameters. Loaded from env vars at startup. |
| `oidc.rs`   | `exchange_code()` — sends the authorization code to the provider's token endpoint and extracts the CID. |
| `jwt.rs`    | `sign_user_jwt()` — builds and signs a NATS user JWT with scoped permissions derived from `NatsSubjects`. |
| `error.rs`  | `AuthError` — unified error type implementing `IntoResponse` with proper HTTP status codes. |

## Authentication flow

1. **Client** authenticates with the identity provider and obtains an
   authorization code.
2. **Client** generates an ephemeral Ed25519 NKey pair.
3. **Client** calls `POST /exchange` with `{ oidc_code, user_nkey_public, network }`.
4. **Auth service** resolves the OIDC provider for the requested network.
5. **Auth service** exchanges the code with the provider → receives the
   user's CID.
6. **Auth service** signs a NATS JWT containing:
   - `sub` = client's NKey public key
   - `name` = CID
   - publish allow = `openlink.v1.{network}.outbox.{cid}`
   - subscribe allow = `openlink.v1.{network}.inbox.{cid}`
7. **Client** receives `{ jwt, cid, network }` and connects to NATS.

## API

### `POST /exchange`

Exchange an OIDC code for a NATS JWT.

**Request:**

```json
{
  "oidc_code": "PILOT",
  "user_nkey_public": "UABC...",
   "network": "demonetwork"
}
```

`network` defaults to `"demonetwork"` if omitted.

**Success (200):**

```json
{
  "jwt": "eyJ0eXAi...",
  "cid": "100000",
   "network": "demonetwork"
}
```

**Errors:**

| Status | Meaning |
|--------|---------|
| 400    | Unknown network (no OIDC provider configured) |
| 401    | OIDC code exchange failed |
| 502    | Could not reach the identity provider |
| 500    | Internal error (NKey or serialisation) |

### `GET /public-key`

Returns the NATS account public key as plain text.

## Configuration

| Env var                  | Default                         | Description |
|--------------------------|---------------------------------|-------------|
| `AUTH_PORT`              | `3001`                          | HTTP listen port |
| `OIDC_DEMONETWORK_TOKEN_URL`  | `http://localhost:4000/token`   | OIDC token endpoint for the `demonetwork` network |
| `RUST_LOG`               | `info`                          | Logging level filter (`tracing-subscriber` `EnvFilter`) |

Additional networks can be added by setting `OIDC_{NETWORK}_TOKEN_URL`
(upper-cased network key).

## Running

```bash
# Start mock-oidc (in another terminal)
cargo run -p mock-oidc

# Start the auth service
cargo run -p openlink-auth

# With debug logging
RUST_LOG=debug cargo run -p openlink-auth
```

## Tests

Unit tests cover configuration loading, CID extraction from tokens,
and NATS JWT generation (structure, permissions, expiry, signatures).

```bash
cargo test -p openlink-auth
```

## Dependencies

| Crate               | Role |
|----------------------|------|
| `openlink-models`   | `NetworkId`, `NetworkAddress` |
| `openlink-sdk`      | `NatsSubjects` — canonical subject format for JWT permissions |
| `axum`               | HTTP framework |
| `tokio`              | Async runtime |
| `reqwest`            | OIDC token endpoint calls |
| `nkeys`              | Ed25519 NKey generation + JWT signing |
| `uuid`               | JWT `jti` claim |
| `base64`             | URL-safe Base64 encoding for NATS JWT format |
| `serde` / `serde_json` | Request/response (de)serialisation |
| `thiserror`          | Error types |
| `tracing` / `tracing-subscriber` | Structured logging |
