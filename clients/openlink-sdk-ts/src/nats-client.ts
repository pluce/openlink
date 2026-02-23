import { connect, type NatsConnection, type Subscription, StringCodec } from "nats.ws";
import type { OpenLinkEnvelope } from "./types";

const sc = StringCodec();

function outboxSubject(networkId: string, address: string): string {
  return `openlink.v1.${networkId}.outbox.${address}`;
}

function inboxSubject(networkId: string, address: string): string {
  return `openlink.v1.${networkId}.inbox.${address}`;
}

interface AuthResponse {
  jwt: string;
  cid: string;
  network: string;
}

async function authenticate(authUrl: string, oidcCode: string, network: string): Promise<AuthResponse> {
  const exchangeUrl = authUrl.startsWith("http") ? "/api/auth/exchange" : `${authUrl}/exchange`;

  const response = await fetch(exchangeUrl, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      oidc_code: oidcCode,
      user_nkey_public: "UA" + "A".repeat(54),
      network,
    }),
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(`Auth failed (${response.status}): ${text}`);
  }

  return response.json();
}

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

  get networkId(): string {
    return this._networkId;
  }

  get networkAddress(): string {
    return this._networkAddress;
  }

  get cid(): string {
    return this._cid;
  }

  get jwt(): string {
    return this._jwt;
  }

  static async connect(opts: {
    natsUrl: string;
    authUrl: string;
    oidcCode: string;
    networkId: string;
  }): Promise<OpenLinkNatsClient> {
    const auth = await authenticate(opts.authUrl, opts.oidcCode, opts.networkId);

    const nc = await connect({
      servers: opts.natsUrl,
      token: auth.jwt,
    });

    return new OpenLinkNatsClient(nc, opts.networkId, auth.cid, auth.cid, auth.jwt);
  }

  onMessage(handler: (envelope: OpenLinkEnvelope) => void): void {
    const subject = inboxSubject(this._networkId, this._networkAddress);
    this.sub = this.nc.subscribe(subject);

    void (async () => {
      for await (const msg of this.sub!) {
        try {
          const json = sc.decode(msg.data);
          const envelope: OpenLinkEnvelope = JSON.parse(json);
          handler(envelope);
        } catch {
          // ignore malformed payloads
        }
      }
    })();
  }

  async publish(envelope: OpenLinkEnvelope): Promise<void> {
    const subject = outboxSubject(this._networkId, this._networkAddress);
    this.nc.publish(subject, sc.encode(JSON.stringify(envelope)));
    await this.nc.flush();
  }

  async disconnect(): Promise<void> {
    if (this.sub) {
      this.sub.unsubscribe();
      this.sub = null;
    }
    await this.nc.drain();
  }
}
