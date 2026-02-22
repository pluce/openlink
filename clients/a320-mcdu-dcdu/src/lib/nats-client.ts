/**
 * nats-client.ts — Raw NATS WebSocket client for OpenLink.
 *
 * This module implements the "raw NATS" integration pattern described in
 * `docs/sdk/quickstart-raw-nats.md`. It handles:
 *
 * 1. Authenticating with the OpenLink auth service (OIDC code → NATS JWT)
 * 2. Connecting to NATS over WebSocket
 * 3. Subscribing to the client's inbox subject
 * 4. Publishing envelopes to the client's outbox subject
 *
 * All messages are JSON-serialized OpenLinkEnvelopes.
 *
 * @see docs/sdk/nats-transport.md — Subject conventions
 */

import { connect, type NatsConnection, type Subscription, StringCodec } from "nats.ws";
import type { OpenLinkEnvelope } from "./types";

const sc = StringCodec();

// ──────────────────────────────────────────────────────────────────────
// Subject helpers — build NATS subject strings
// ──────────────────────────────────────────────────────────────────────

/** Build the outbox subject: `openlink.v1.{network}.outbox.{address}` */
function outboxSubject(networkId: string, address: string): string {
  return `openlink.v1.${networkId}.outbox.${address}`;
}

/** Build the inbox subject: `openlink.v1.{network}.inbox.{address}` */
function inboxSubject(networkId: string, address: string): string {
  return `openlink.v1.${networkId}.inbox.${address}`;
}

// ──────────────────────────────────────────────────────────────────────
// Authentication — exchange OIDC code for NATS credentials
// ──────────────────────────────────────────────────────────────────────

interface AuthResponse {
  /** Signed NATS user JWT. */
  jwt: string;
  /** Authenticated CID (used to derive network address). */
  cid: string;
  /** Network the JWT was issued for. */
  network: string;
}

/**
 * Exchange an OIDC authorization code for a NATS JWT via the auth service.
 *
 * In demo mode (with mock-oidc), the code can be any string — the mock
 * will use it as the identity. For example, passing "PILOT" gives CID "100000".
 *
 * @param authUrl  - Auth service base URL (e.g. "http://localhost:3001")
 * @param oidcCode - OIDC authorization code
 * @param network  - Target network (e.g. "demonetwork")
 */
async function authenticate(
  authUrl: string,
  oidcCode: string,
  network: string
): Promise<AuthResponse> {
  // In the browser we don't have nkeys, so we pass a dummy public key.
  // The auth service will embed it in the JWT, but for WebSocket connections
  // via nats.ws with token-based auth this is sufficient for demo purposes.
  //
  // In dev mode, we go through Vite's reverse proxy (/api/auth/*)
  // to avoid CORS issues. The proxy is configured in vite.config.ts.
  const exchangeUrl = authUrl.startsWith('http')
    ? '/api/auth/exchange'  // Use Vite proxy in dev
    : `${authUrl}/exchange`;

  const response = await fetch(exchangeUrl, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      oidc_code: oidcCode,
      user_nkey_public: "UA" + "A".repeat(54), // Dummy NKey public key
      network,
    }),
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(`Auth failed (${response.status}): ${text}`);
  }

  return response.json();
}

// ──────────────────────────────────────────────────────────────────────
// OpenLinkNatsClient — main client class
// ──────────────────────────────────────────────────────────────────────

/**
 * A connected OpenLink NATS client.
 *
 * Wraps a nats.ws connection and provides typed publish/subscribe
 * methods for OpenLink envelopes.
 *
 * Usage:
 * ```ts
 * const client = await OpenLinkNatsClient.connect({
 *   natsUrl: "ws://localhost:4223",
 *   authUrl: "http://localhost:3001",
 *   oidcCode: "PILOT",
 *   networkId: "demonetwork",
 * });
 *
 * // Subscribe to incoming messages
 * client.onMessage((envelope) => {
 *   console.log("Received:", envelope);
 * });
 *
 * // Publish an envelope
 * await client.publish(someEnvelope);
 * ```
 */
export class OpenLinkNatsClient {
  private nc: NatsConnection;
  private sub: Subscription | null = null;
  private _networkId: string;
  private _networkAddress: string;
  private _cid: string;
  private _jwt: string;

  private constructor(
    nc: NatsConnection,
    networkId: string,
    networkAddress: string,
    cid: string,
    jwt: string
  ) {
    this.nc = nc;
    this._networkId = networkId;
    this._networkAddress = networkAddress;
    this._cid = cid;
    this._jwt = jwt;
  }

  /** The network ID this client is connected to. */
  get networkId(): string {
    return this._networkId;
  }

  /** The runtime network address (derived from CID). */
  get networkAddress(): string {
    return this._networkAddress;
  }

  /** The CID obtained from the auth service. */
  get cid(): string {
    return this._cid;
  }

  /** The JWT token used for authentication (included in every envelope). */
  get jwt(): string {
    return this._jwt;
  }

  /**
   * Connect to OpenLink via NATS WebSocket.
   *
   * This performs the full authentication + connection flow:
   * 1. Exchange OIDC code for NATS JWT
   * 2. Connect to NATS over WebSocket using the JWT
   */
  static async connect(opts: {
    natsUrl: string;
    authUrl: string;
    oidcCode: string;
    networkId: string;
  }): Promise<OpenLinkNatsClient> {
    // Step 1: Authenticate
    console.log("[OpenLink] Authenticating with auth service...");
    const auth = await authenticate(
      opts.authUrl,
      opts.oidcCode,
      opts.networkId
    );
    console.log(`[OpenLink] Authenticated as CID=${auth.cid}`);

    // Step 2: Connect to NATS over WebSocket
    console.log(`[OpenLink] Connecting to NATS at ${opts.natsUrl}...`);
    const nc = await connect({
      servers: opts.natsUrl,
      token: auth.jwt,
    });
    console.log("[OpenLink] Connected to NATS");

    return new OpenLinkNatsClient(nc, opts.networkId, auth.cid, auth.cid, auth.jwt);
  }

  /**
   * Subscribe to the inbox and call `handler` for each incoming envelope.
   *
   * The subscription is stored so it can be cleaned up on disconnect.
   */
  onMessage(handler: (envelope: OpenLinkEnvelope) => void): void {
    const subject = inboxSubject(this._networkId, this._networkAddress);
    console.log(`[OpenLink] Subscribing to inbox: ${subject}`);

    this.sub = this.nc.subscribe(subject);

    // Process messages asynchronously
    (async () => {
      for await (const msg of this.sub!) {
        try {
          const json = sc.decode(msg.data);
          const envelope: OpenLinkEnvelope = JSON.parse(json);
          handler(envelope);
        } catch (err) {
          console.error("[OpenLink] Failed to parse inbox message:", err);
        }
      }
    })();
  }

  /**
   * Publish an OpenLink envelope to our outbox.
   *
   * The server will read from our outbox, process the message,
   * and route it to the appropriate recipient's inbox.
   */
  async publish(envelope: OpenLinkEnvelope): Promise<void> {
    const subject = outboxSubject(this._networkId, this._networkAddress);
    const json = JSON.stringify(envelope);
    this.nc.publish(subject, sc.encode(json));
    // Flush to ensure the message is sent immediately
    await this.nc.flush();
  }

  /**
   * Gracefully disconnect from NATS.
   *
   * Unsubscribes from the inbox and drains the connection.
   */
  async disconnect(): Promise<void> {
    if (this.sub) {
      this.sub.unsubscribe();
      this.sub = null;
    }
    await this.nc.drain();
    console.log("[OpenLink] Disconnected from NATS");
  }
}
