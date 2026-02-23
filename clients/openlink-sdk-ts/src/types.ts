/** OpenLink/ACARS/CPDLC wire types for TypeScript clients. */

export interface OpenLinkEnvelope {
  id: string;
  timestamp: string;
  correlation_id?: string | null;
  routing: {
    source: OpenLinkRoutingEndpoint;
    destination: OpenLinkRoutingEndpoint;
  };
  payload: OpenLinkMessage;
  token: string;
}

export type OpenLinkRoutingEndpoint =
  | { Server: string }
  | { Address: [string, string] };

export type OpenLinkMessage =
  | { type: "Acars"; data: AcarsEnvelope }
  | { type: "Meta"; data: MetaPayload };

export interface MetaPayload {
  StationStatus: [string, "Online" | "Offline", AcarsRoutingEndpoint];
}

export interface AcarsRoutingEndpoint {
  callsign: string;
  address: string;
}

export interface AcarsEnvelope {
  routing: {
    aircraft: AcarsRoutingEndpoint;
  };
  message: AcarsMessage;
}

export type AcarsMessage = { type: "CPDLC"; data: CpdlcEnvelope };

export interface CpdlcEnvelope {
  source: string;
  destination: string;
  message: CpdlcMessageType;
}

export type CpdlcMessageType =
  | { type: "Application"; data: CpdlcApplicationMessage }
  | { type: "Meta"; data: CpdlcMetaMessage };

export interface CpdlcApplicationMessage {
  /** Message Identification Number (1—63), assigned by sender/server. */
  min: number;
  /** Message Reference Number (MIN referenced by this response). */
  mrn: number | null;
  elements: MessageElement[];
  timestamp: string;
}

export interface MessageElement {
  id: string;
  args: CpdlcArgument[];
}

export interface CpdlcArgument {
  type: string;
  value: string | number;
}

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
  | {
      type: "LogonForward";
      data: {
        flight: string;
        flight_plan_origin: string;
        flight_plan_destination: string;
        new_station: string;
      };
    }
  | { type: "SessionUpdate"; data: { session: CpdlcSessionView } };

export interface CpdlcSessionView {
  aircraft: string | null;
  aircraft_address: string | null;
  active_connection: CpdlcConnectionInfo | null;
  inactive_connection: CpdlcConnectionInfo | null;
  next_data_authority: string | null;
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

export interface ConnectionSettings {
  networkId: string;
  natsUrl: string;
  authUrl: string;
  oidcCode: string;
  callsign: string;
  acarsAddress: string;
}

export interface TextPart {
  text: string;
  isParam: boolean;
}

export interface ResponseIntent {
  label: string;
  downlinkId: string;
}

export type DcduMessageStatus =
  | "new"
  | "open"
  | "responding"
  | "responded"
  | "draft"
  | "sending"
  | "sent";

export interface DcduMessage {
  id: string;
  timestamp: Date;
  from: string;
  text: string;
  textParts: TextPart[];
  isOutgoing: boolean;
  status: DcduMessageStatus;
  responseIntents: ResponseIntent[];
  respondedWith?: string;
  mrn?: number | null;
  min?: number | null;
  elements?: MessageElement[];
}
