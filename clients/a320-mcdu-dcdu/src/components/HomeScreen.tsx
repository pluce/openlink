/**
 * HomeScreen.tsx — Initial connection setup screen.
 *
 * This screen is displayed before the pilot is connected to the OpenLink
 * network. It allows entering connection parameters:
 *   - Network ID (e.g. "demonetwork")
 *   - NATS WebSocket URL
 *   - Auth service URL
 *   - OIDC code (demo: any string used as identity)
 *   - Callsign (e.g. "AFR123")
 *   - ACARS address (e.g. "AY213")
 *
 * The form pre-fills with sensible defaults for local development.
 */

import { useState } from "react";
import type { ConnectionSettings } from "../lib/types";
import type { ConnectionStatus } from "../hooks/useOpenLink";

interface HomeScreenProps {
  /** Current NATS connection status. */
  status: ConnectionStatus;
  /** Error message from last connection attempt. */
  error: string | null;
  /** Callback to initiate connection. */
  onConnect: (settings: ConnectionSettings) => void;
}

export default function HomeScreen({ status, error, onConnect }: HomeScreenProps) {
  // Pre-fill with local development defaults
  const [form, setForm] = useState<ConnectionSettings>({
    networkId: "demonetwork",
    natsUrl: "ws://localhost:9222",
    authUrl: "http://localhost:3001",
    oidcCode: "PILOT",
    callsign: "FBW291",
    acarsAddress: "AY291",
  });

  const isConnecting = status === "connecting";

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onConnect(form);
  };

  const updateField = (field: keyof ConnectionSettings, value: string) => {
    setForm((prev) => ({ ...prev, [field]: value }));
  };

  return (
    <div className="home-screen">
      <div className="home-header">
        <h1>OPENLINK A320 DEMONSTRATOR</h1>
        <p className="home-subtitle">MCDU / DCDU Integration Example</p>
      </div>

      <form className="home-form" onSubmit={handleSubmit}>
        <div className="form-section">
          <h2>Network Configuration</h2>
          <div className="form-row">
            <label htmlFor="networkId">NETWORK</label>
            <input
              id="networkId"
              type="text"
              value={form.networkId}
              onChange={(e) => updateField("networkId", e.target.value)}
              disabled={isConnecting}
            />
          </div>
          <div className="form-row">
            <label htmlFor="natsUrl">NATS URL</label>
            <input
              id="natsUrl"
              type="text"
              value={form.natsUrl}
              onChange={(e) => updateField("natsUrl", e.target.value)}
              disabled={isConnecting}
            />
          </div>
          <div className="form-row">
            <label htmlFor="authUrl">AUTH URL</label>
            <input
              id="authUrl"
              type="text"
              value={form.authUrl}
              onChange={(e) => updateField("authUrl", e.target.value)}
              disabled={isConnecting}
            />
          </div>
          <div className="form-row">
            <label htmlFor="oidcCode">OIDC CODE</label>
            <input
              id="oidcCode"
              type="text"
              value={form.oidcCode}
              onChange={(e) => updateField("oidcCode", e.target.value)}
              disabled={isConnecting}
            />
          </div>
        </div>

        <div className="form-section">
          <h2>Aircraft Identity</h2>
          <div className="form-row">
            <label htmlFor="callsign">CALLSIGN</label>
            <input
              id="callsign"
              type="text"
              value={form.callsign}
              onChange={(e) => updateField("callsign", e.target.value.toUpperCase())}
              disabled={isConnecting}
              maxLength={7}
            />
          </div>
          <div className="form-row">
            <label htmlFor="acarsAddress">ACARS ADDR</label>
            <input
              id="acarsAddress"
              type="text"
              value={form.acarsAddress}
              onChange={(e) => updateField("acarsAddress", e.target.value.toUpperCase())}
              disabled={isConnecting}
              maxLength={7}
            />
          </div>
        </div>

        {error && (
          <div className="form-error">
            <span className="error-icon">⚠</span> {error}
          </div>
        )}

        <button
          type="submit"
          className="connect-btn"
          disabled={isConnecting}
        >
          {isConnecting ? "CONNECTING..." : "CONNECT"}
        </button>
      </form>

      <div className="home-footer">
        <p>
          OpenLink A320 Demonstrator — Raw NATS integration example.
          <br />
          See <code>docs/sdk/quickstart-raw-nats.md</code> for documentation.
        </p>
      </div>
    </div>
  );
}
