/**
 * StatusBar.tsx — Connection and session status indicator.
 *
 * Displayed between the DCDU and MCDU, this bar shows:
 * - NATS connection status (connecting / connected / disconnected)
 * - Callsign and CID
 * - CPDLC session state (active ATC, connection phase)
 * - A disconnect button
 */

import type { ConnectionStatus } from "../hooks/useOpenLink";
import type { CpdlcSessionView } from "../lib/types";

interface StatusBarProps {
  /** Current NATS connection status. */
  status: ConnectionStatus;
  /** Aircraft callsign. */
  callsign: string;
  /** CID from the auth service. */
  cid: string | null;
  /** CPDLC session state. */
  session: CpdlcSessionView | null;
  /** Callback to disconnect. */
  onDisconnect: () => void;
}

export default function StatusBar({
  status,
  callsign,
  cid,
  session,
  onDisconnect,
}: StatusBarProps) {
  const activeAtc = session?.active_connection?.peer;
  const phase = session?.active_connection?.phase;

  return (
    <div className="status-bar">
      <div className="status-left">
        {/* Connection indicator dot */}
        <span
          className={`status-dot ${
            status === "connected"
              ? "green"
              : status === "connecting"
              ? "amber"
              : "red"
          }`}
        />
        <span className="status-label">
          {status === "connected"
            ? "CONNECTED"
            : status === "connecting"
            ? "CONNECTING..."
            : "DISCONNECTED"}
        </span>
      </div>

      <div className="status-center">
        <span className="status-callsign">{callsign}</span>
        {cid && <span className="status-cid">CID: {cid}</span>}
        {activeAtc && (
          <span className="status-atc">
            ATC: {activeAtc}{" "}
            <span className={`phase-${phase?.toLowerCase()}`}>
              [{phase}]
            </span>
          </span>
        )}
      </div>

      <div className="status-right">
        <button className="disconnect-btn" onClick={onDisconnect}>
          DISCONNECT
        </button>
      </div>
    </div>
  );
}
