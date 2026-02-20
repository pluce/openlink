---
title: mock-oidc
description: Mirrored documentation from crates/mock-oidc/README.md
slug: generated/crates/mock-oidc
sidebar:
  order: 8
---

> Source: crates/mock-oidc/README.md (synced automatically)

# Mock OIDC Provider

A minimal **Identity Provider (IDP)** for local development and testing of OpenLink authentication flows.
Simulates an OAuth2 / OIDC compliant server (like DEMONETWORK Connect or IVAO SSO).

## Purpose
- Accepts client redirect requests.
- Issues Authorization Codes.
- Validates codes and issues standard OIDC ID Tokens (JWT) signed with RS256.
- Provides standard OIDC discovery endpoints (`.well-known/openid-configuration`, `jwks`).

## Dynamic Identity Generation
The service creates identities dynamically based on the **authorization code** provided during the token exchange.
This allows the CLI to simulate any network address without pre-registration.

| Code | Role | Sub (Subject ID) | Name | Email |
|------|------|------------------|------|-------|
| `PILOT` | Pilot | `100000` | Captain Smith | `pilot@demonetwork.net` |
| `ATC` | ATC | `888888` | Generic ATC | `atc@demonetwork.net` |
| *<ANY_STRING>* | Custom | *<ANY_STRING>* | User *<ANY_STRING>* | *<ANY_STRING>@demonetwork.net* |

**Example:**
Requesting a token with `code="AFR123"` will generate an ID Token for a user with `sub="AFR123"` and `name="User AFR123"`.

## API Endpoints

- **`GET /.well-known/openid-configuration`**: Discovery document.
- **`GET /jwks`**: JSON Web Key Set (Public Keys).
- **`GET /authorize`**: (Browser Flow) Simulates user login and redirect.
- **`POST /token`**: (Back-channel) Exchanges code for ID Token (JWT).

## Usage

Start the server:
```bash
cargo run -p mock-oidc
```
Runs on `http://localhost:4000`.

### Integration
Used by `openlink-cli` to fetch an ID Token which is then exchanged for a NATS JWT via the `openlink-auth` service (or directly via NATS 2.10+ Auth Callout in future versions).
