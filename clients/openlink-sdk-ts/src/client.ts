import {
  buildAircraftApplication,
  buildConnectionRequest,
  buildConnectionResponse,
  buildContactRequest,
  buildEndService,
  buildLogonForward,
  buildLogonRequest,
  buildLogonResponse,
  buildLogicalAckForSender,
  buildNextDataAuthority,
  buildStationApplication,
} from "./envelope";
import { OpenLinkNatsClient } from "./nats-client";
import type { OpenLinkEnvelope, OpenLinkMessage } from "./types";

/**
 * High-level TypeScript OpenLink client with API names aligned to Rust SDK where possible.
 */
export class OpenLinkClient {
  private readonly inner: OpenLinkNatsClient;

  private constructor(inner: OpenLinkNatsClient) {
    this.inner = inner;
  }

  static async connect_with_authorization_code(
    nats_url: string,
    auth_url: string,
    authorization_code: string,
    network_id: string
  ): Promise<OpenLinkClient> {
    const inner = await OpenLinkNatsClient.connect({
      natsUrl: nats_url,
      authUrl: auth_url,
      oidcCode: authorization_code,
      networkId: network_id,
    });
    return new OpenLinkClient(inner);
  }

  network_id(): string {
    return this.inner.networkId;
  }

  network_address(): string {
    return this.inner.networkAddress;
  }

  cid(): string {
    return this.inner.cid;
  }

  jwt(): string {
    return this.inner.jwt;
  }

  async subscribe_inbox(handler: (envelope: OpenLinkEnvelope) => void): Promise<void> {
    this.inner.onMessage(handler);
  }

  async send_to_server(envelope: OpenLinkEnvelope): Promise<void> {
    await this.inner.publish(envelope);
  }

  async disconnect(): Promise<void> {
    await this.inner.disconnect();
  }

  cpdlc_logon_request(
    aircraft_callsign: string,
    aircraft_address: string,
    target_station: string
  ): OpenLinkMessage {
    return buildLogonRequest(aircraft_callsign, aircraft_address, target_station);
  }

  cpdlc_logon_response(
    atc_callsign: string,
    aircraft_callsign: string,
    aircraft_address: string,
    accepted: boolean
  ): OpenLinkMessage {
    return buildLogonResponse(atc_callsign, aircraft_callsign, aircraft_address, accepted);
  }

  cpdlc_connection_request(
    atc_callsign: string,
    aircraft_callsign: string,
    aircraft_address: string
  ): OpenLinkMessage {
    return buildConnectionRequest(atc_callsign, aircraft_callsign, aircraft_address);
  }

  cpdlc_connection_response(
    aircraft_callsign: string,
    aircraft_address: string,
    atc_callsign: string,
    accepted: boolean
  ): OpenLinkMessage {
    return buildConnectionResponse(aircraft_callsign, aircraft_address, atc_callsign, accepted);
  }

  cpdlc_next_data_authority(
    atc_callsign: string,
    aircraft_callsign: string,
    aircraft_address: string,
    nda_callsign: string
  ): OpenLinkMessage {
    return buildNextDataAuthority(atc_callsign, aircraft_callsign, aircraft_address, nda_callsign);
  }

  cpdlc_contact_request(
    atc_callsign: string,
    aircraft_callsign: string,
    aircraft_address: string,
    next_station: string
  ): OpenLinkMessage {
    return buildContactRequest(atc_callsign, aircraft_callsign, aircraft_address, next_station);
  }

  cpdlc_end_service(
    atc_callsign: string,
    aircraft_callsign: string,
    aircraft_address: string
  ): OpenLinkMessage {
    return buildEndService(atc_callsign, aircraft_callsign, aircraft_address);
  }

  cpdlc_logon_forward(
    atc_callsign: string,
    aircraft_callsign: string,
    aircraft_address: string,
    new_station: string
  ): OpenLinkMessage {
    return buildLogonForward(atc_callsign, aircraft_callsign, aircraft_address, new_station);
  }

  cpdlc_station_application(
    station_callsign: string,
    aircraft_callsign: string,
    aircraft_address: string,
    elements: { id: string; args: { type: string; value: string | number }[] }[],
    mrn: number | null = null
  ): OpenLinkMessage {
    return buildStationApplication(
      station_callsign,
      aircraft_callsign,
      aircraft_address,
      elements,
      mrn
    );
  }

  cpdlc_aircraft_application(
    aircraft_callsign: string,
    aircraft_address: string,
    station_callsign: string,
    elements: { id: string; args: { type: string; value: string | number }[] }[],
    mrn: number | null = null
  ): OpenLinkMessage {
    return buildAircraftApplication(
      aircraft_callsign,
      aircraft_address,
      station_callsign,
      elements,
      mrn
    );
  }

  cpdlc_logical_ack(
    sender_callsign: string,
    receiver_callsign: string,
    aircraft_callsign: string,
    aircraft_address: string,
    mrn: number
  ): OpenLinkMessage {
    const is_aircraft_sender = sender_callsign === aircraft_callsign;
    return buildLogicalAckForSender(
      aircraft_callsign,
      aircraft_address,
      sender_callsign,
      receiver_callsign,
      mrn,
      is_aircraft_sender
    );
  }
}
