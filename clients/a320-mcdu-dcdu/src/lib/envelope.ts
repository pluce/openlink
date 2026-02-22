/**
 * envelope.ts — Helper functions to build OpenLink message envelopes.
 *
 * These builders mirror the Rust `MessageBuilder` from `openlink-sdk`
 * and produce the exact same JSON wire format expected by the server.
 *
 * The 3-layer nesting is:
 *   OpenLinkEnvelope → AcarsEnvelope → CpdlcEnvelope
 *
 * @see docs/sdk/envelopes-and-stack.md
 */

import { v4 as uuidv4 } from "uuid";
import type {
  OpenLinkEnvelope,
  OpenLinkMessage,
  AcarsEnvelope,
  CpdlcEnvelope,
  CpdlcMessageType,
} from "./types";

// ──────────────────────────────────────────────────────────────────────
// OpenLink Envelope builder
// ──────────────────────────────────────────────────────────────────────

/**
 * Build a complete OpenLink envelope addressed to the server.
 *
 * @param networkId      - The network identifier (e.g. "demonetwork")
 * @param networkAddress - The client's runtime address (e.g. "CID_AFR123")
 * @param payload        - The wrapped payload (Acars or Meta)
 */
export function buildEnvelope(
  networkId: string,
  networkAddress: string,
  payload: OpenLinkMessage,
  token: string
): OpenLinkEnvelope {
  return {
    id: uuidv4(),
    timestamp: new Date().toISOString(),
    routing: {
      source: { Address: [networkId, networkAddress] },
      destination: { Server: networkId },
    },
    payload,
    token,
  };
}

// ──────────────────────────────────────────────────────────────────────
// Station status (presence) messages
// ──────────────────────────────────────────────────────────────────────

/**
 * Build a "station online" envelope.
 *
 * The station status payload advertises our presence on the network
 * with our callsign and ACARS address.
 *
 * @see docs/sdk/stations-presence.md
 */
export function buildStationOnline(
  networkId: string,
  networkAddress: string,
  callsign: string,
  acarsAddress: string,
  token: string
): OpenLinkEnvelope {
  return buildEnvelope(networkId, networkAddress, {
    type: "Meta",
    data: {
      StationStatus: [
        networkAddress,
        "Online",
        { callsign, address: acarsAddress },
      ],
    },
  }, token);
}

/**
 * Build a "station offline" envelope for graceful disconnection.
 */
export function buildStationOffline(
  networkId: string,
  networkAddress: string,
  callsign: string,
  acarsAddress: string,
  token: string
): OpenLinkEnvelope {
  return buildEnvelope(networkId, networkAddress, {
    type: "Meta",
    data: {
      StationStatus: [
        networkAddress,
        "Offline",
        { callsign, address: acarsAddress },
      ],
    },
  }, token);
}

// ──────────────────────────────────────────────────────────────────────
// CPDLC message builders
// ──────────────────────────────────────────────────────────────────────

/**
 * Build a complete ACARS+CPDLC message for a given aircraft.
 *
 * This is the core builder that wraps a CpdlcMessageType inside
 * the ACARS and CPDLC envelopes.
 */
function buildCpdlcMessage(
  aircraftCallsign: string,
  acarsAddress: string,
  source: string,
  destination: string,
  message: CpdlcMessageType
): OpenLinkMessage {
  const cpdlcEnvelope: CpdlcEnvelope = {
    source,
    destination,
    message,
  };

  const acarsEnvelope: AcarsEnvelope = {
    routing: {
      aircraft: {
        callsign: aircraftCallsign,
        address: acarsAddress,
      },
    },
    message: {
      type: "CPDLC",
      data: cpdlcEnvelope,
    },
  };

  return {
    type: "Acars",
    data: acarsEnvelope,
  };
}

/**
 * Build a CPDLC Logon Request message.
 *
 * This is the first step in the CPDLC session lifecycle.
 * The aircraft sends this to the target ATC station to begin
 * the logon/identification process.
 *
 * @param aircraftCallsign - Our callsign (e.g. "AFR123")
 * @param acarsAddress     - Our ACARS address (e.g. "AY213")
 * @param targetStation    - The ATC station ICAO code (e.g. "LFPG")
 * @param origin           - Flight plan origin airport
 * @param destination      - Flight plan destination airport
 *
 * @see docs/acars-ref-gold/logon_connection.md — Logon phase
 */
export function buildLogonRequest(
  aircraftCallsign: string,
  acarsAddress: string,
  targetStation: string,
  origin: string = "ZZZZ",
  destination: string = "ZZZZ"
): OpenLinkMessage {
  return buildCpdlcMessage(
    aircraftCallsign,
    acarsAddress,
    aircraftCallsign,
    targetStation,
    {
      type: "Meta",
      data: {
        type: "LogonRequest",
        data: {
          station: targetStation,
          flight_plan_origin: origin,
          flight_plan_destination: destination,
        },
      },
    }
  );
}

/**
 * Build a CPDLC Application downlink response (e.g. DM0=WILCO, DM1=UNABLE).
 *
 * Used when the pilot presses a response button on the DCDU.
 *
 * @param aircraftCallsign - Our callsign
 * @param acarsAddress     - Our ACARS address
 * @param atcStation       - The ATC station we're responding to
 * @param downlinkId       - The DM element id (e.g. "DM0" for WILCO)
 * @param mrn              - Message Reference Number (MIN of the uplink being responded to)
 */
export function buildApplicationResponse(
  aircraftCallsign: string,
  acarsAddress: string,
  atcStation: string,
  downlinkId: string,
  mrn: number | null
): OpenLinkMessage {
  return buildCpdlcMessage(
    aircraftCallsign,
    acarsAddress,
    aircraftCallsign,
    atcStation,
    {
      type: "Application",
      data: {
        min: 0, // Server assigns MINs
        mrn,
        elements: [{ id: downlinkId, args: [] }],
        timestamp: new Date().toISOString(),
      },
    }
  );
}

/**
 * Build a pilot-initiated CPDLC Application downlink message.
 *
 * Used when the pilot composes a message from the MCDU (e.g. REQUEST FL340)
 * and sends it from the DCDU.
 *
 * @param aircraftCallsign - Our callsign
 * @param acarsAddress     - Our ACARS address
 * @param atcStation       - The ATC station to send to
 * @param elements         - Array of message elements with args
 * @param mrn              - Optional MRN linking to the uplink being responded to
 */
export function buildApplicationDownlink(
  aircraftCallsign: string,
  acarsAddress: string,
  atcStation: string,
  elements: { id: string; args: { type: string; value: string | number }[] }[],
  mrn: number | null = null
): OpenLinkMessage {
  return buildCpdlcMessage(
    aircraftCallsign,
    acarsAddress,
    aircraftCallsign,
    atcStation,
    {
      type: "Application",
      data: {
        min: 0, // Server assigns MINs
        mrn,
        elements,
        timestamp: new Date().toISOString(),
      },
    }
  );
}

/**
 * Build a CPDLC Connection Response message.
 *
 * Aircraft auto-accepts connection requests from the ground station
 * after a successful logon.
 *
 * @param aircraftCallsign - Our callsign
 * @param acarsAddress     - Our ACARS address
 * @param atcCallsign      - The ATC station responding to
 * @param accepted         - Whether to accept the connection
 */
export function buildConnectionResponse(
  aircraftCallsign: string,
  acarsAddress: string,
  atcCallsign: string,
  accepted: boolean
): OpenLinkMessage {
  return buildCpdlcMessage(
    aircraftCallsign,
    acarsAddress,
    aircraftCallsign,
    atcCallsign,
    {
      type: "Meta",
      data: {
        type: "ConnectionResponse",
        data: { accepted },
      },
    }
  );
}
