import { v4 as uuidv4 } from "uuid";
import { logicalAckDownlinkId, logicalAckElementIdForSender } from "./cpdlc-runtime";
import type {
  AcarsEnvelope,
  CpdlcEnvelope,
  CpdlcMessageType,
  OpenLinkEnvelope,
  OpenLinkMessage,
} from "./types";

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

export function buildStationOnline(
  networkId: string,
  networkAddress: string,
  callsign: string,
  acarsAddress: string,
  token: string
): OpenLinkEnvelope {
  return buildEnvelope(
    networkId,
    networkAddress,
    {
      type: "Meta",
      data: {
        StationStatus: [networkAddress, "Online", { callsign, address: acarsAddress }],
      },
    },
    token
  );
}

export function buildStationOffline(
  networkId: string,
  networkAddress: string,
  callsign: string,
  acarsAddress: string,
  token: string
): OpenLinkEnvelope {
  return buildEnvelope(
    networkId,
    networkAddress,
    {
      type: "Meta",
      data: {
        StationStatus: [networkAddress, "Offline", { callsign, address: acarsAddress }],
      },
    },
    token
  );
}

function buildCpdlcMessage(
  aircraftCallsign: string,
  acarsAddress: string,
  source: string,
  destination: string,
  message: CpdlcMessageType
): OpenLinkMessage {
  const cpdlcEnvelope: CpdlcEnvelope = { source, destination, message };
  const acarsEnvelope: AcarsEnvelope = {
    routing: {
      aircraft: {
        callsign: aircraftCallsign,
        address: acarsAddress,
      },
    },
    message: { type: "CPDLC", data: cpdlcEnvelope },
  };

  return { type: "Acars", data: acarsEnvelope };
}

export function buildLogonRequest(
  aircraftCallsign: string,
  acarsAddress: string,
  targetStation: string,
  origin = "ZZZZ",
  destination = "ZZZZ"
): OpenLinkMessage {
  return buildCpdlcMessage(aircraftCallsign, acarsAddress, aircraftCallsign, targetStation, {
    type: "Meta",
    data: {
      type: "LogonRequest",
      data: {
        station: targetStation,
        flight_plan_origin: origin,
        flight_plan_destination: destination,
      },
    },
  });
}

export function buildLogonResponse(
  atcCallsign: string,
  aircraftCallsign: string,
  acarsAddress: string,
  accepted: boolean
): OpenLinkMessage {
  return buildCpdlcMessage(aircraftCallsign, acarsAddress, atcCallsign, aircraftCallsign, {
    type: "Meta",
    data: {
      type: "LogonResponse",
      data: { accepted },
    },
  });
}

export function buildConnectionRequest(
  atcCallsign: string,
  aircraftCallsign: string,
  acarsAddress: string
): OpenLinkMessage {
  return buildCpdlcMessage(aircraftCallsign, acarsAddress, atcCallsign, aircraftCallsign, {
    type: "Meta",
    data: {
      type: "ConnectionRequest",
      data: null,
    },
  });
}

export function buildApplicationResponse(
  aircraftCallsign: string,
  acarsAddress: string,
  atcStation: string,
  downlinkId: string,
  mrn: number | null
): OpenLinkMessage {
  return buildCpdlcMessage(aircraftCallsign, acarsAddress, aircraftCallsign, atcStation, {
    type: "Application",
    data: {
      min: 0,
      mrn,
      elements: [{ id: downlinkId, args: [] }],
      timestamp: new Date().toISOString(),
    },
  });
}

export function buildApplicationDownlink(
  aircraftCallsign: string,
  acarsAddress: string,
  atcStation: string,
  elements: { id: string; args: { type: string; value: string | number }[] }[],
  mrn: number | null = null
): OpenLinkMessage {
  return buildCpdlcMessage(aircraftCallsign, acarsAddress, aircraftCallsign, atcStation, {
    type: "Application",
    data: {
      min: 0,
      mrn,
      elements,
      timestamp: new Date().toISOString(),
    },
  });
}

export function buildLogicalAck(
  aircraftCallsign: string,
  acarsAddress: string,
  atcStation: string,
  referencedMin: number
): OpenLinkMessage {
  return buildCpdlcMessage(aircraftCallsign, acarsAddress, aircraftCallsign, atcStation, {
    type: "Application",
    data: {
      min: 0,
      mrn: referencedMin,
      elements: [{ id: logicalAckDownlinkId(), args: [] }],
      timestamp: new Date().toISOString(),
    },
  });
}

export function buildLogicalAckForSender(
  aircraftCallsign: string,
  acarsAddress: string,
  senderCallsign: string,
  receiverCallsign: string,
  referencedMin: number,
  isAircraftSender: boolean
): OpenLinkMessage {
  return buildCpdlcMessage(aircraftCallsign, acarsAddress, senderCallsign, receiverCallsign, {
    type: "Application",
    data: {
      min: 0,
      mrn: referencedMin,
      elements: [{ id: logicalAckElementIdForSender(isAircraftSender), args: [] }],
      timestamp: new Date().toISOString(),
    },
  });
}

/** Rust-parity alias for sender-aware logical acknowledgement helper. */
export const cpdlc_logical_ack = buildLogicalAckForSender;

export function buildNextDataAuthority(
  atcCallsign: string,
  aircraftCallsign: string,
  acarsAddress: string,
  ndaCallsign: string
): OpenLinkMessage {
  return buildCpdlcMessage(aircraftCallsign, acarsAddress, atcCallsign, aircraftCallsign, {
    type: "Application",
    data: {
      min: 0,
      mrn: null,
      elements: [
        {
          id: "UM160",
          args: [{ type: "FacilityDesignation", value: ndaCallsign }],
        },
      ],
      timestamp: new Date().toISOString(),
    },
  });
}

export function buildContactRequest(
  atcCallsign: string,
  aircraftCallsign: string,
  acarsAddress: string,
  nextStation: string,
  frequency = "UNKNOWN"
): OpenLinkMessage {
  return buildCpdlcMessage(aircraftCallsign, acarsAddress, atcCallsign, aircraftCallsign, {
    type: "Application",
    data: {
      min: 0,
      mrn: null,
      elements: [
        {
          id: "UM117",
          args: [
            { type: "UnitName", value: nextStation },
            { type: "Frequency", value: frequency },
          ],
        },
      ],
      timestamp: new Date().toISOString(),
    },
  });
}

export function buildEndService(
  atcCallsign: string,
  aircraftCallsign: string,
  acarsAddress: string
): OpenLinkMessage {
  return buildCpdlcMessage(aircraftCallsign, acarsAddress, atcCallsign, aircraftCallsign, {
    type: "Application",
    data: {
      min: 0,
      mrn: null,
      elements: [{ id: "UM161", args: [] }],
      timestamp: new Date().toISOString(),
    },
  });
}

export function buildLogonForward(
  atcCallsign: string,
  aircraftCallsign: string,
  acarsAddress: string,
  newStation: string,
  origin = "ZZZZ",
  destination = "ZZZZ"
): OpenLinkMessage {
  return buildCpdlcMessage(aircraftCallsign, acarsAddress, atcCallsign, newStation, {
    type: "Meta",
    data: {
      type: "LogonForward",
      data: {
        flight: aircraftCallsign,
        flight_plan_origin: origin,
        flight_plan_destination: destination,
        new_station: newStation,
      },
    },
  });
}

export function buildStationApplication(
  stationCallsign: string,
  aircraftCallsign: string,
  acarsAddress: string,
  elements: { id: string; args: { type: string; value: string | number }[] }[],
  mrn: number | null = null
): OpenLinkMessage {
  return buildCpdlcMessage(aircraftCallsign, acarsAddress, stationCallsign, aircraftCallsign, {
    type: "Application",
    data: {
      min: 0,
      mrn,
      elements,
      timestamp: new Date().toISOString(),
    },
  });
}

export function buildAircraftApplication(
  aircraftCallsign: string,
  acarsAddress: string,
  stationCallsign: string,
  elements: { id: string; args: { type: string; value: string | number }[] }[],
  mrn: number | null = null
): OpenLinkMessage {
  return buildCpdlcMessage(aircraftCallsign, acarsAddress, aircraftCallsign, stationCallsign, {
    type: "Application",
    data: {
      min: 0,
      mrn,
      elements,
      timestamp: new Date().toISOString(),
    },
  });
}

export function buildConnectionResponse(
  aircraftCallsign: string,
  acarsAddress: string,
  atcCallsign: string,
  accepted: boolean
): OpenLinkMessage {
  return buildCpdlcMessage(aircraftCallsign, acarsAddress, aircraftCallsign, atcCallsign, {
    type: "Meta",
    data: {
      type: "ConnectionResponse",
      data: { accepted },
    },
  });
}
