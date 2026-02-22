/**
 * types.ts — TypeScript type definitions mirroring the OpenLink / ACARS / CPDLC
 * wire format used over NATS.
 *
 * These types are derived from the Rust models in `openlink-models` and
 * follow the exact same JSON serialization produced by serde.
 *
 * @see docs/sdk/envelopes-and-stack.md   — 3-layer message stack
 * @see docs/sdk/addressing-routing.md    — routing conventions
 */

// ──────────────────────────────────────────────────────────────────────
// OpenLink Envelope (outermost transport layer)
// ──────────────────────────────────────────────────────────────────────

/** The top-level OpenLink envelope wrapping every message on NATS. */
export interface OpenLinkEnvelope {
  id: string;
  timestamp: string;
  correlation_id?: string | null;
  routing: {
    source: OpenLinkRoutingEndpoint;
    destination: OpenLinkRoutingEndpoint;
  };
  payload: OpenLinkMessage;
  /** Bearer / JWT token for authentication. */
  token: string;
}

/**
 * Routing endpoint — determines how a message is addressed.
 *
 * - `{ "Server": "<network_id>" }` → routed through the OpenLink server
 * - `{ "Address": ["<network_id>", "<network_address>"] }` → direct peer
 */
export type OpenLinkRoutingEndpoint =
  | { Server: string }
  | { Address: [string, string] };

// ──────────────────────────────────────────────────────────────────────
// Payload variants (OpenLinkMessage)
// ──────────────────────────────────────────────────────────────────────

/** Externally-tagged enum matching the Rust `OpenLinkMessage`. */
export type OpenLinkMessage =
  | { type: "Acars"; data: AcarsEnvelope }
  | { type: "Meta"; data: MetaPayload };

// ──────────────────────────────────────────────────────────────────────
// Meta payload (station status / presence)
// ──────────────────────────────────────────────────────────────────────

/** Station status payload — tuple-style: [stationId, status, endpoint]. */
export interface MetaPayload {
  StationStatus: [string, "Online" | "Offline", AcarsRoutingEndpoint];
}

export interface AcarsRoutingEndpoint {
  callsign: string;
  address: string;
}

// ──────────────────────────────────────────────────────────────────────
// ACARS Envelope (middle layer)
// ──────────────────────────────────────────────────────────────────────

/** ACARS-level envelope carrying the operational message. */
export interface AcarsEnvelope {
  routing: {
    aircraft: AcarsRoutingEndpoint;
  };
  message: AcarsMessage;
}

/** Currently only CPDLC is supported. */
export type AcarsMessage = { type: "CPDLC"; data: CpdlcEnvelope };

// ──────────────────────────────────────────────────────────────────────
// CPDLC Envelope (innermost operational layer)
// ──────────────────────────────────────────────────────────────────────

/** CPDLC-level envelope identifying source and destination callsigns. */
export interface CpdlcEnvelope {
  source: string;
  destination: string;
  message: CpdlcMessageType;
}

/** The CPDLC message — either an Application message or a Meta message. */
export type CpdlcMessageType =
  | { type: "Application"; data: CpdlcApplicationMessage }
  | { type: "Meta"; data: CpdlcMetaMessage };

// ──────────────────────────────────────────────────────────────────────
// CPDLC Application message (operational clearances, requests, etc.)
// ──────────────────────────────────────────────────────────────────────

export interface CpdlcApplicationMessage {
  /** Message Identification Number (0—63), assigned by the server. */
  min: number;
  /** Message Reference Number — MIN of the message being responded to. */
  mrn: number | null;
  /** Ordered list of message elements (catalog entries + arguments). */
  elements: MessageElement[];
  timestamp: string;
}

export interface MessageElement {
  /** Catalog ID, e.g. "UM20", "DM0". */
  id: string;
  /** Arguments matching the catalog template placeholders. */
  args: CpdlcArgument[];
}

export interface CpdlcArgument {
  type: string;
  value: string | number;
}

// ──────────────────────────────────────────────────────────────────────
// CPDLC Meta messages (session management — logon, connection, etc.)
// ──────────────────────────────────────────────────────────────────────

export type CpdlcMetaMessage =
  | {
      type: "LogonRequest";
      data: {
        station: string;
        flight_plan_origin: string;
        flight_plan_destination: string;
      };
    }
  | { type: "LogonResponse"; data: { accepted: boolean } }
  | { type: "ConnectionRequest"; data: null }
  | { type: "ConnectionResponse"; data: { accepted: boolean } }
  | { type: "ContactRequest"; data: { station: string } }
  | { type: "ContactResponse"; data: { accepted: boolean } }
  | { type: "ContactComplete"; data: null }
  | {
      type: "LogonForward";
      data: {
        flight: string;
        flight_plan_origin: string;
        flight_plan_destination: string;
        new_station: string;
      };
    }
  | {
      type: "NextDataAuthority";
      data: { nda: AcarsRoutingEndpoint };
    }
  | { type: "EndService"; data: null }
  | { type: "SessionUpdate"; data: { session: CpdlcSessionView } };

// ──────────────────────────────────────────────────────────────────────
// Session view (server-authoritative state snapshot)
// ──────────────────────────────────────────────────────────────────────

export interface CpdlcSessionView {
  aircraft: string | null;
  aircraft_address: string | null;
  active_connection: CpdlcConnectionInfo | null;
  inactive_connection: CpdlcConnectionInfo | null;
  next_data_authority: AcarsRoutingEndpoint | null;
}

export interface CpdlcConnectionInfo {
  peer: string;
  phase: CpdlcConnectionPhase;
}

export type CpdlcConnectionPhase =
  | "LogonPending"
  | "LoggedOn"
  | "Connected"
  | "Terminated";

// ──────────────────────────────────────────────────────────────────────
// Application state for the A320 client
// ──────────────────────────────────────────────────────────────────────

/** Connection settings entered by the user on the home screen. */
export interface ConnectionSettings {
  /** Network to join (e.g. "demonetwork"). */
  networkId: string;
  /** NATS WebSocket URL (e.g. "ws://localhost:4223"). */
  natsUrl: string;
  /** Auth service URL (e.g. "http://localhost:3001"). */
  authUrl: string;
  /** OIDC code to authenticate with (in demo mode: any string). */
  oidcCode: string;
  /** Aircraft callsign (e.g. "AFR123"). */
  callsign: string;
  /** ACARS address (e.g. "AY213"). */
  acarsAddress: string;
}

// ──────────────────────────────────────────────────────────────────────
// text segment for colored rendering on the DCDU
// ──────────────────────────────────────────────────────────────────────

/** A text part: either plain text (white) or a parameter value (blue). */
export interface TextPart {
  text: string;
  /** True for placeholder values (level, position, etc.) → rendered blue. */
  isParam: boolean;
}

// ──────────────────────────────────────────────────────────────────────
// DCDU response intent — mirrors catalog short_response_intents
// ──────────────────────────────────────────────────────────────────────

/** A possible short response the pilot can send via DCDU buttons. */
export interface ResponseIntent {
  /** Display label on the DCDU button (e.g. "WILCO", "UNABLE"). */
  label: string;
  /** The downlink message id to send (e.g. "DM0" for WILCO). */
  downlinkId: string;
}

// ──────────────────────────────────────────────────────────────────────
// DCDU message status
// ──────────────────────────────────────────────────────────────────────

export type DcduMessageStatus =
  /** Message received, not yet opened / read. */
  | "new"
  /** Message opened and displayed — waiting for pilot response. */
  | "open"
  /** Pilot pressed a response button — waiting for server ack. */
  | "responding"
  /** Response confirmed by server. */
  | "responded"
  /** Outgoing message being prepared (from MCDU). */
  | "draft"
  /** Outgoing message sent, waiting for transmission. */
  | "sending"
  /** Outgoing message confirmed sent. */
  | "sent";

/** A received or sent message displayed in the DCDU. */
export interface DcduMessage {
  id: string;
  timestamp: Date;
  /** The source callsign (who sent the message). */
  from: string;

  /** Flat text fallback (for meta / simple messages). */
  text: string;
  /** Rich text parts: [{text, isParam}] for colored rendering. */
  textParts: TextPart[];

  /** Whether this message was sent by us (outgoing). */
  isOutgoing: boolean;

  /** Current message lifecycle status. */
  status: DcduMessageStatus;

  /** Available short-response intents (WILCO, UNABLE, STANDBY…). */
  responseIntents: ResponseIntent[];

  /** The label of the response the pilot chose (e.g. "WILCO"). */
  respondedWith?: string;

  /** MRN of the message this responds to (for linking). */
  mrn?: number | null;
  /** MIN of this message (for linking responses). */
  min?: number | null;

  /** Original message elements (for draft messages that need to be sent). */
  elements?: MessageElement[];
}
