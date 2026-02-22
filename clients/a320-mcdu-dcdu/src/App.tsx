/**
 * App.tsx — Main application component for the A320 MCDU/DCDU Demonstrator.
 *
 * This component orchestrates the two main screens:
 *
 * 1. HomeScreen — Connection setup (shown when disconnected)
 * 2. CockpitView — Split view with DCDU (top) and MCDU (bottom),
 *    shown when connected to the OpenLink network.
 *
 * The useOpenLink hook manages all NATS connection lifecycle,
 * CPDLC session state, and message handling.
 */

import { useOpenLink } from "./hooks/useOpenLink";
import HomeScreen from "./components/HomeScreen";
import McduScreen from "./components/McduScreen";
import DcduScreen from "./components/DcduScreen";
import StatusBar from "./components/StatusBar";

export default function App() {
  const openlink = useOpenLink();

  // ──────────────────────────────────────────────────────────────────
  // Disconnected → show the home/setup screen
  // ──────────────────────────────────────────────────────────────────

  if (openlink.status !== "connected") {
    return (
      <HomeScreen
        status={openlink.status}
        error={openlink.error}
        onConnect={openlink.connect}
      />
    );
  }

  // ──────────────────────────────────────────────────────────────────
  // Connected → show the cockpit view (DCDU + MCDU)
  // ──────────────────────────────────────────────────────────────────

  return (
    <div className="app-page">
      {/* Discreet page header — connection info */}
      <StatusBar
        status={openlink.status}
        callsign={openlink.settings?.callsign ?? ""}
        cid={openlink.cid}
        session={openlink.session}
        onDisconnect={openlink.disconnect}
      />

      <div className="cockpit-view">
        {/* DCDU — top half: displays incoming/outgoing CPDLC messages */}
        <DcduScreen
          messages={openlink.dcduMessages}
          session={openlink.session}
          onSendResponse={openlink.sendResponse}
          onSendDraft={openlink.sendDraft}
        />

        {/* MCDU — bottom half: pilot interaction for ATC/CPDLC */}
        <McduScreen
          callsign={openlink.settings?.callsign ?? ""}
          session={openlink.session}
          onLogonRequest={openlink.sendLogonRequest}
          onTransferToDcdu={openlink.transferToDcdu}
        />
      </div>
    </div>
  );
}
