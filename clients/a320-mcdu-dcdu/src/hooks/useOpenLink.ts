/**
 * useOpenLink.ts — React hook for managing the OpenLink connection lifecycle.
 *
 * This hook encapsulates the full NATS connection, station presence,
 * CPDLC session state, and message handling. It provides a clean API
 * for the UI components to interact with the OpenLink network.
 *
 * State machine for the connection:
 *   Disconnected → Connecting → Connected → Disconnected
 *
 * CPDLC session state machine (server-authoritative):
 *   Idle → LogonPending → LoggedOn → Connected
 *
 * @see docs/sdk/quickstart-raw-nats.md
 */

import { useState, useCallback, useRef, useEffect } from "react";
import { OpenLinkNatsClient } from "../lib/nats-client";
import {
  buildStationOnline,
  buildStationOffline,
  buildLogonRequest,
  buildConnectionResponse,
  buildApplicationResponse,
  buildApplicationDownlink,
  buildLogicalAck,
  buildEnvelope,
} from "../lib/envelope";
import {
  loadCatalog,
  elementsToTextParts,
  textPartsToString,
  getResponseIntents,
} from "../lib/catalog";
import { shouldAutoSendLogicalAck } from "@openlink/sdk-ts";
import type {
  ConnectionSettings,
  DcduMessage,
  OpenLinkEnvelope,
  CpdlcSessionView,
  CpdlcMetaMessage,
  CpdlcConnectionPhase,
  MessageElement,
} from "../lib/types";
import { v4 as uuidv4 } from "uuid";

// Load the CPDLC catalog at module init (bundled by Vite)
import catalogData from "../data/catalog.v1.json";
loadCatalog((catalogData as { messages: unknown[] }).messages as never[]);

// ──────────────────────────────────────────────────────────────────────
// Connection status
// ──────────────────────────────────────────────────────────────────────

export type ConnectionStatus = "disconnected" | "connecting" | "connected";

// ──────────────────────────────────────────────────────────────────────
// Hook return type
// ──────────────────────────────────────────────────────────────────────

export interface UseOpenLinkReturn {
  /** Current NATS connection status. */
  status: ConnectionStatus;
  /** Error message if connection failed. */
  error: string | null;
  /** Connect to OpenLink with the given settings. */
  connect: (settings: ConnectionSettings) => Promise<void>;
  /** Gracefully disconnect. */
  disconnect: () => Promise<void>;
  /** Send a CPDLC logon request to a station. */
  sendLogonRequest: (station: string) => Promise<void>;
  /** Current CPDLC session state (from server). */
  session: CpdlcSessionView | null;
  /** The phase of the active connection (if any). */
  activeConnectionPhase: CpdlcConnectionPhase | null;
  /** Messages displayed on the DCDU. */
  dcduMessages: DcduMessage[];
  /** Send a short response (WILCO, UNABLE, etc.) from the DCDU. */
  sendResponse: (messageId: string, responseLabel: string, downlinkId: string) => Promise<void>;
  /** Transfer composed message elements from MCDU to DCDU as a draft. */
  transferToDcdu: (elements: MessageElement[], mrn?: number | null) => void;
  /** Send a draft message from the DCDU to the network. */
  sendDraft: (messageId: string) => Promise<void>;
  /** The current connection settings (after connect). */
  settings: ConnectionSettings | null;
  /** CID obtained from the auth service */
  cid: string | null;
}

// ──────────────────────────────────────────────────────────────────────
// Hook implementation
// ──────────────────────────────────────────────────────────────────────

export function useOpenLink(): UseOpenLinkReturn {
  const [status, setStatus] = useState<ConnectionStatus>("disconnected");
  const [error, setError] = useState<string | null>(null);
  const [session, setSession] = useState<CpdlcSessionView | null>(null);
  const [dcduMessages, setDcduMessages] = useState<DcduMessage[]>([]);
  const [settings, setSettings] = useState<ConnectionSettings | null>(null);
  const [cid, setCid] = useState<string | null>(null);

  // Refs to keep stable references across renders
  const clientRef = useRef<OpenLinkNatsClient | null>(null);
  const settingsRef = useRef<ConnectionSettings | null>(null);
  const heartbeatRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const dcduMessagesRef = useRef<DcduMessage[]>([]);
  const sessionRef = useRef<CpdlcSessionView | null>(null);

  // Keep refs in sync with state — avoids stale closures in callbacks
  useEffect(() => { dcduMessagesRef.current = dcduMessages; }, [dcduMessages]);
  useEffect(() => { sessionRef.current = session; }, [session]);

  // Derived state: phase of the active connection
  const activeConnectionPhase = session?.active_connection?.phase ?? null;

  // ────────────────────────────────────────────────────────────────────
  // Incoming message handler
  // ────────────────────────────────────────────────────────────────────

  /**
   * Process an incoming OpenLink envelope from the inbox.
   *
   * This handler:
   * 1. Identifies the message type (Meta or Application)
   * 2. Updates the CPDLC session state from SessionUpdate messages
   * 3. Auto-accepts ConnectionRequests (avionics behavior)
   * 4. Adds displayable messages to the DCDU queue
   */
  const handleIncomingMessage = useCallback(
    (envelope: OpenLinkEnvelope) => {
      console.log("[A320] Incoming:", JSON.stringify(envelope).slice(0, 200));

      // Only process ACARS/CPDLC messages
      if (envelope.payload.type !== "Acars") return;

      const acars = envelope.payload.data;
      const cpdlc = acars.message.data;
      const cpdlcMsg = cpdlc.message;

      if (cpdlcMsg.type === "Meta") {
        handleMetaMessage(cpdlcMsg.data, cpdlc.source, acars.routing.aircraft.address);
      } else if (cpdlcMsg.type === "Application") {
        // Parse elements into rich TextParts using the CPDLC catalog
        const elements = cpdlcMsg.data.elements;

        // Auto-send CPDLC logical acknowledgement based on shared SDK runtime rules.
        if (shouldAutoSendLogicalAck(elements, cpdlcMsg.data.min)) {
          const s = settingsRef.current;
          if (s && clientRef.current) {
            const ack = buildLogicalAck(
              s.callsign,
              s.acarsAddress,
              cpdlc.source,
              cpdlcMsg.data.min
            );
            const ackEnv = buildEnvelope(
              s.networkId,
              clientRef.current.networkAddress,
              ack,
              clientRef.current.jwt
            );
            clientRef.current.publish(ackEnv).catch((err) => {
              console.warn("[A320] Logical ACK send failed:", err);
            });
          }
        }

        // UM117 CONTACT [unit name] [frequency] now drives auto-logon handoff.
        const contactElement = elements.find((e) => e.id === "UM117");
        if (contactElement) {
          const stationArg = contactElement.args.find((a) => a.type === "UnitName");
          const station = typeof stationArg?.value === "string" ? stationArg.value : null;
          const s = settingsRef.current;
          if (station && s && clientRef.current) {
            const logon = buildLogonRequest(s.callsign, s.acarsAddress, station);
            const env = buildEnvelope(
              s.networkId,
              clientRef.current.networkAddress,
              logon,
              clientRef.current.jwt
            );
            clientRef.current.publish(env);
          }
        }

        const textParts = elementsToTextParts(elements);
        const plainText = textPartsToString(textParts);
        const responseIntents = getResponseIntents(elements);
        const incomingMrn = cpdlcMsg.data.mrn;

        const msg: DcduMessage = {
          id: uuidv4(),
          timestamp: new Date(),
          from: cpdlc.source,
          text: plainText,
          textParts,
          isOutgoing: false,
          status: "open",
          responseIntents,
          min: cpdlcMsg.data.min,
          mrn: incomingMrn,
        };

        setDcduMessages((prev) => {
          let updated = [...prev];

          // If this incoming message references a MRN (response to something),
          // try to link it into an existing dialog chain.
          if (incomingMrn != null) {
            // 1. Direct match: do we have an outgoing message whose min == incomingMrn?
            let targetOutgoing = updated.find(
              (m) => m.isOutgoing && m.min != null && m.min === incomingMrn
            );

            // 2. Heuristic: the server assigns MINs to our downlinks but never
            //    tells us. Find the most recent outgoing "sent" message that
            //    has a mrn (meaning it was a response in a dialog) and whose
            //    server-assigned MIN we don't know yet (min is undefined).
            if (!targetOutgoing) {
              targetOutgoing = [...updated]
                .reverse()
                .find(
                  (m) =>
                    m.isOutgoing &&
                    m.status === "sent" &&
                    m.mrn != null &&
                    (m.min == null || m.min === 0)
                );
              // Now we know its server-assigned MIN — store it
              if (targetOutgoing) {
                updated = updated.map((m) =>
                  m.id === targetOutgoing!.id
                    ? { ...m, min: incomingMrn }
                    : m
                );
                console.log(
                  `[A320] Resolved outgoing MIN: ${targetOutgoing.text} → MIN=${incomingMrn}`
                );
              }
            }

            // 3. Walk the chain back to the root uplink:
            //    incoming (mrn=Y) → our outgoing (min=Y, mrn=X) → original uplink (min=X)
            if (targetOutgoing && targetOutgoing.mrn != null) {
              const rootUplinkMin = targetOutgoing.mrn;
              // Re-link this incoming message to the root uplink's dialog
              msg.mrn = rootUplinkMin;
              // Mark the incoming as a dialog closure (no pilot response needed)
              msg.status = "responded";
              msg.respondedWith = plainText;
              msg.responseIntents = [];

              // Close the root uplink dialog
              updated = updated.map((m) =>
                !m.isOutgoing && m.min === rootUplinkMin
                  ? { ...m, status: "responded" as const, respondedWith: plainText }
                  : m
              );
              console.log(
                `[A320] Dialog closed: ATC ${plainText} → root uplink MIN=${rootUplinkMin}`
              );
            }
          }

          return [...updated, msg];
        });
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    []
  );

  /**
   * Handle incoming CPDLC Meta messages (session management protocol).
   *
   * The key messages for the LOGON flow are:
   * - SessionUpdate: Server-authoritative state snapshot → update local state
   * - ConnectionRequest: ATC wants to connect → auto-accept
   * - LogonResponse: ATC accepted/rejected our logon → informational
   */
  const handleMetaMessage = useCallback(
    (meta: CpdlcMetaMessage, source: string, _aircraftAddress: string) => {
      switch (meta.type) {
        case "SessionUpdate": {
          // Server sent the authoritative session state — replace our local copy
          console.log("[A320] Session update:", meta.data.session);
          setSession(meta.data.session);
          break;
        }

        case "ConnectionRequest": {
          // ATC wants to establish a CPDLC connection with us.
          // Per avionics behavior, we auto-accept.
          console.log("[A320] Auto-accepting connection request from", source);
          const s = settingsRef.current;
          if (s && clientRef.current) {
            const msg = buildConnectionResponse(
              s.callsign,
              s.acarsAddress,
              source,
              true
            );
            const env = buildEnvelope(s.networkId, clientRef.current.networkAddress, msg, clientRef.current.jwt);
            clientRef.current.publish(env);
          }
          break;
        }

        case "LogonResponse": {
          // The station responded to our logon request
          const accepted = meta.data.accepted;
          const statusText = accepted ? "ACCEPTED" : "REJECTED";
          console.log(`[A320] Logon response from ${source}: ${statusText}`);

          const msg: DcduMessage = {
            id: uuidv4(),
            timestamp: new Date(),
            from: source,
            text: `LOGON ${statusText} BY ${source}`,
            textParts: [
              { text: "LOGON ", isParam: false },
              { text: statusText, isParam: true },
              { text: " BY ", isParam: false },
              { text: source, isParam: true },
            ],
            isOutgoing: false,
            status: "responded",
            responseIntents: [],
            respondedWith: statusText,
          };
          setDcduMessages((prev) => [...prev, msg]);
          break;
        }

        default:
          console.log("[A320] Unhandled meta:", meta.type);
      }
    },
    []
  );

  // ────────────────────────────────────────────────────────────────────
  // Connect
  // ────────────────────────────────────────────────────────────────────

  const connectToOpenLink = useCallback(
    async (newSettings: ConnectionSettings) => {
      setStatus("connecting");
      setError(null);

      try {
        // Step 1: Connect to NATS
        const client = await OpenLinkNatsClient.connect({
          natsUrl: newSettings.natsUrl,
          authUrl: newSettings.authUrl,
          oidcCode: newSettings.oidcCode,
          networkId: newSettings.networkId,
        });

        clientRef.current = client;
        settingsRef.current = newSettings;
        setSettings(newSettings);
        setCid(client.cid);

        // Step 2: Subscribe to inbox (before publishing online!)
        client.onMessage(handleIncomingMessage);

        // Step 3: Publish station online
        const onlineEnv = buildStationOnline(
          newSettings.networkId,
          client.networkAddress,
          newSettings.callsign,
          newSettings.acarsAddress,
          client.jwt
        );
        await client.publish(onlineEnv);
        console.log("[A320] Station online published");

        // Step 4: Start heartbeat (refresh presence every 25 seconds)
        heartbeatRef.current = setInterval(async () => {
          if (clientRef.current && settingsRef.current) {
            const hb = buildStationOnline(
              settingsRef.current.networkId,
              clientRef.current.networkAddress,
              settingsRef.current.callsign,
              settingsRef.current.acarsAddress,
              clientRef.current.jwt
            );
            await clientRef.current.publish(hb);
          }
        }, 25_000);

        setStatus("connected");
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        console.error("[A320] Connection failed:", message);
        setError(message);
        setStatus("disconnected");
      }
    },
    [handleIncomingMessage]
  );

  // ────────────────────────────────────────────────────────────────────
  // Disconnect
  // ────────────────────────────────────────────────────────────────────

  const disconnectFromOpenLink = useCallback(async () => {
    // Stop heartbeat
    if (heartbeatRef.current) {
      clearInterval(heartbeatRef.current);
      heartbeatRef.current = null;
    }

    // Send offline status
    if (clientRef.current && settingsRef.current) {
      try {
        const offlineEnv = buildStationOffline(
          settingsRef.current.networkId,
          clientRef.current.networkAddress,
          settingsRef.current.callsign,
          settingsRef.current.acarsAddress,
          clientRef.current.jwt
        );
        await clientRef.current.publish(offlineEnv);
      } catch {
        // Best-effort
      }
      await clientRef.current.disconnect();
    }

    clientRef.current = null;
    settingsRef.current = null;
    setStatus("disconnected");
    setSession(null);
    setDcduMessages([]);
    setSettings(null);
    setCid(null);
  }, []);

  // ────────────────────────────────────────────────────────────────────
  // CPDLC Logon Request
  // ────────────────────────────────────────────────────────────────────

  /**
   * Send a CPDLC Logon Request to the specified ATC station.
   *
   * This is initiated from the MCDU NOTIFICATION page. The pilot enters
   * the ATC station code (e.g. "LFPG") and presses NOTIFY.
   *
   * @param station - 4-letter ICAO code of the ATC station
   */
  const sendLogonRequest = useCallback(async (station: string) => {
    if (!clientRef.current || !settingsRef.current) {
      throw new Error("Not connected");
    }

    const s = settingsRef.current;
    const logonMsg = buildLogonRequest(
      s.callsign,
      s.acarsAddress,
      station.toUpperCase()
    );
    const env = buildEnvelope(
      s.networkId,
      clientRef.current.networkAddress,
      logonMsg,
      clientRef.current.jwt
    );

    await clientRef.current.publish(env);
    console.log(`[A320] Logon request sent to ${station}`);

    // Add outgoing message to DCDU
    const dcduMsg: DcduMessage = {
      id: uuidv4(),
      timestamp: new Date(),
      from: s.callsign,
      text: `LOGON REQUEST → ${station.toUpperCase()}`,
      textParts: [
        { text: "LOGON REQUEST → ", isParam: false },
        { text: station.toUpperCase(), isParam: true },
      ],
      isOutgoing: true,
      status: "sent",
      responseIntents: [],
    };
    setDcduMessages((prev) => [...prev, dcduMsg]);
  }, []);

  // ────────────────────────────────────────────────────────────────────
  // DCDU Short Response
  // ────────────────────────────────────────────────────────────────────

  /**
   * Send a short response (WILCO, UNABLE, ROGER, etc.) from the DCDU.
   *
   * When the pilot presses a response button on the DCDU, we:
   * 1. Mark the message as "responding" (button highlighted)
   * 2. Build and publish a downlink Application message with the DM code
   * 3. On success, mark the message as "responded" with the chosen label
   */
  const sendResponse = useCallback(
    async (messageId: string, responseLabel: string, downlinkId: string) => {
      if (!clientRef.current || !settingsRef.current) return;

      // Mark the message as "responding" immediately (highlight the button)
      setDcduMessages((prev) =>
        prev.map((m) =>
          m.id === messageId
            ? { ...m, status: "responding" as const, respondedWith: responseLabel }
            : m
        )
      );

      const s = settingsRef.current;
      const client = clientRef.current;

      // Find the original message to get source station and MIN for MRN
      const origMsg = dcduMessagesRef.current.find((m) => m.id === messageId);
      const destination = origMsg?.from ?? "UNKNOWN";
      const mrn = origMsg?.min ?? null;

      // Build a properly-wrapped CPDLC Application response
      const payload = buildApplicationResponse(
        s.callsign,
        s.acarsAddress,
        destination,
        downlinkId,
        mrn
      );

      const env = buildEnvelope(
        s.networkId,
        client.networkAddress,
        payload,
        client.jwt
      );

      try {
        await client.publish(env);
        console.log(`[A320] Response ${responseLabel} (${downlinkId}) sent`);

        // STANDBY (DM2) keeps the dialogue open — the pilot can still
        // send a definitive response later. All other responses
        // (WILCO/DM0, UNABLE/DM1, ROGER/DM3, AFFIRM/DM4, NEGATIVE/DM5)
        // close the dialogue. The catalog already tells us which is which.
        const isStandby = downlinkId === "DM2";
        const newStatus = isStandby ? ("sent" as const) : ("responded" as const);

        setDcduMessages((prev) =>
          prev.map((m) =>
            m.id === messageId
              ? { ...m, status: newStatus, respondedWith: responseLabel }
              : m
          )
        );
      } catch (err) {
        console.error("[A320] Failed to send response:", err);
        // Revert to open status on failure
        setDcduMessages((prev) =>
          prev.map((m) =>
            m.id === messageId
              ? { ...m, status: "open" as const, respondedWith: undefined }
              : m
          )
        );
      }
    },
    [] // eslint-disable-line react-hooks/exhaustive-deps
  );

  // ────────────────────────────────────────────────────────────────────
  // Transfer message from MCDU → DCDU (draft)
  // ────────────────────────────────────────────────────────────────────

  /**
   * Transfer a composed message from the MCDU to the DCDU as a draft.
   * The pilot can then review it and press SEND on the DCDU.
   */
  const transferToDcdu = useCallback(
    (elements: MessageElement[], mrn?: number | null) => {
      const textParts = elementsToTextParts(elements);
      const plainText = textPartsToString(textParts);
      const s = settingsRef.current;

      setDcduMessages((prev) => {
        // If no explicit MRN given, auto-link to the most recent OPEN uplink
        const effectiveMrn =
          mrn ?? [...prev]
            .reverse()
            .find(
              (m) =>
                !m.isOutgoing &&
                m.min != null &&
                (m.status === "open" || m.status === "new" || m.status === "sent")
            )?.min ?? null;

        const msg: DcduMessage = {
          id: uuidv4(),
          timestamp: new Date(),
          from: s?.callsign ?? "SELF",
          text: plainText,
          textParts,
          isOutgoing: true,
          status: "draft",
          responseIntents: [],
          elements,
          mrn: effectiveMrn,
        };
        console.log(`[A320] Draft transferred to DCDU: ${elements.map(e => e.id).join(", ")} → ${plainText} (mrn=${effectiveMrn})`);
        return [...prev, msg];
      });
    },
    []
  );

  // ────────────────────────────────────────────────────────────────────
  // Send draft from DCDU
  // ────────────────────────────────────────────────────────────────────

  /**
   * Send a draft message from the DCDU to the network.
   * Called when the pilot presses SEND on a draft message.
   */
  const sendDraft = useCallback(
    async (messageId: string) => {
      if (!clientRef.current || !settingsRef.current) return;

      const s = settingsRef.current;
      const client = clientRef.current;
      const draftMsg = dcduMessagesRef.current.find((m) => m.id === messageId);
      if (!draftMsg || !draftMsg.elements) return;

      const peer = sessionRef.current?.active_connection?.peer ?? "UNKNOWN";

      // Mark as sending
      setDcduMessages((prev) =>
        prev.map((m) =>
          m.id === messageId ? { ...m, status: "sending" as const } : m
        )
      );

      const payload = buildApplicationDownlink(
        s.callsign,
        s.acarsAddress,
        peer,
        draftMsg.elements,
        draftMsg.mrn ?? null
      );
      const env = buildEnvelope(
        s.networkId,
        client.networkAddress,
        payload,
        client.jwt
      );

      try {
        await client.publish(env);
        console.log(`[A320] Draft sent: ${draftMsg.text}`);
        setDcduMessages((prev) =>
          prev.map((m) =>
            m.id === messageId ? { ...m, status: "sent" as const } : m
          )
        );
      } catch (err) {
        console.error("[A320] Failed to send draft:", err);
        setDcduMessages((prev) =>
          prev.map((m) =>
            m.id === messageId ? { ...m, status: "draft" as const } : m
          )
        );
      }
    },
    [] // eslint-disable-line react-hooks/exhaustive-deps
  );

  // ────────────────────────────────────────────────────────────────────
  // Cleanup on unmount
  // ────────────────────────────────────────────────────────────────────

  useEffect(() => {
    return () => {
      if (heartbeatRef.current) {
        clearInterval(heartbeatRef.current);
      }
    };
  }, []);

  return {
    status,
    error,
    connect: connectToOpenLink,
    disconnect: disconnectFromOpenLink,
    sendLogonRequest,
    sendResponse,
    transferToDcdu,
    sendDraft,
    session,
    activeConnectionPhase,
    dcduMessages,
    settings,
    cid,
  };
}
