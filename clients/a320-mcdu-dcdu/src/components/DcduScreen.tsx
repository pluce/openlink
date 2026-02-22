/**
 * DcduScreen.tsx — Datalink Control and Display Unit (DCDU) display.
 *
 * The DCDU is the cockpit unit that displays incoming and outgoing
 * CPDLC messages. In a real A320, it's mounted above the center pedestal
 * and the pilot interacts with it to:
 *
 * - View incoming ATC clearances and instructions
 * - Send responses (WILCO, UNABLE, STANDBY, etc.)
 * - Browse message history (MSG+/MSG-)
 *
 * Layout:
 * ┌──────┬────────────────────────────────────────┬──────┐
 * │ BRT  │  1410Z FROM KUSA           OPEN        │PRINT │
 * │ DIM  │                                        │      │
 * │      │  CLEARED DABOY TO LNK //               │ PGE- │
 * │ MSG- │  (white text, params in blue)           │ PGE+ │
 * │ MSG+ │                                        │      │
 * │      │  ══════════════════════════════════     │      │
 * │ ──   │  *UNABLE     │  SENT      │   STBY*    │  ──  │
 * │ ──   │  <OTHER      │  PG 1 2    │  WILCO*    │  ──  │
 * └──────┴────────────────────────────────────────┴──────┘
 *
 * The screen is split into two zones:
 * - UPPER: message content (header + message text) — display only
 * - LOWER: fixed 2-line height — response actions on left/right,
 *          transient status (SENT/SENDING) and pagination in center.
 *
 * Response buttons are INSIDE the screen, in the lower zone.
 * They reflect the available responses from the CPDLC message catalog.
 *
 * Color rules:
 * - Message text: white (static) + blue (parameters)
 * - Responded message: entirely green
 * - Response button: normal = green text; pressed/responding = green bg
 * - Status badge top-right: "OPEN" when unresponded, response label in
 *   green highlight when responded
 */

import { useRef, useState } from "react";
import type {
  DcduMessage,
  CpdlcSessionView,
  TextPart,
  ResponseIntent,
} from "../lib/types";

// ──────────────────────────────────────────────────────────────────────
// Props
// ──────────────────────────────────────────────────────────────────────

interface DcduScreenProps {
  /** List of DCDU messages (received and sent). */
  messages: DcduMessage[];
  /** Current CPDLC session state. */
  session: CpdlcSessionView | null;
  /** Callback when the pilot presses a response button. */
  onSendResponse: (
    messageId: string,
    responseLabel: string,
    downlinkId: string
  ) => Promise<void>;
  /** Callback when the pilot presses SEND on a draft message. */
  onSendDraft: (messageId: string) => Promise<void>;
}

// ──────────────────────────────────────────────────────────────────────
// Response layout helpers
// ──────────────────────────────────────────────────────────────────────

/**
 * Arrange response intents into a 2×2 grid matching real DCDU layout.
 *
 * Standard WU layout:
 *   top-left:  *UNABLE     top-right: STBY*
 *   bot-left:  (empty)     bot-right: WILCO*
 *
 * The intents come from the catalog. We match by well-known labels
 * and position them accordingly.
 */
interface ResponseGrid {
  topLeft: ResponseIntent | null;
  topRight: ResponseIntent | null;
  botLeft: ResponseIntent | null;
  botRight: ResponseIntent | null;
}

function arrangeResponses(intents: ResponseIntent[]): ResponseGrid {
  const grid: ResponseGrid = {
    topLeft: null,
    topRight: null,
    botLeft: null,
    botRight: null,
  };
  if (intents.length === 0) return grid;

  const find = (labels: string[]): ResponseIntent | undefined =>
    intents.find((i) => labels.includes(i.label.toUpperCase()));

  const wilco = find(["WILCO"]);
  const unable = find(["UNABLE"]);
  const standby = find(["STANDBY"]);
  const roger = find(["ROGER"]);
  const affirm = find(["AFFIRM"]);
  const negative = find(["NEGATIVE"]);

  // WU pattern
  if (wilco) {
    grid.topLeft = unable || null;
    grid.topRight = standby || null;
    grid.botRight = wilco;
    return grid;
  }
  // AN pattern
  if (affirm) {
    grid.topLeft = negative || null;
    grid.topRight = standby || null;
    grid.botRight = affirm;
    return grid;
  }
  // R pattern
  if (roger) {
    grid.topRight = standby || null;
    grid.botRight = roger;
    return grid;
  }
  // Generic fallback
  const slots: Array<keyof ResponseGrid> = [
    "botRight",
    "topRight",
    "topLeft",
    "botLeft",
  ];
  intents.forEach((intent, i) => {
    if (i < slots.length) grid[slots[i]] = intent;
  });
  return grid;
}

// ──────────────────────────────────────────────────────────────────────
// Component
// ──────────────────────────────────────────────────────────────────────

export default function DcduScreen({
  messages,
  session,
  onSendResponse,
  onSendDraft,
}: DcduScreenProps) {
  /** Index of the currently viewed message (navigate with MSG+/MSG-). */
  const [viewIndex, setViewIndex] = useState(-1);

  // Build a filtered list that hides messages linked to an uplink dialog.
  // These linked responses are shown inline under the uplink they respond to.
  const visibleMessages = messages.filter((m) => {
    if (m.mrn == null) return true;
    // If this message (outgoing OR incoming) references an uplink's MIN,
    // it's part of a dialog chain → hide from main list (shown inline).
    const isLinkedToUplink = messages.some(
      (u) => u.id !== m.id && !u.isOutgoing && u.min != null && u.min === m.mrn
    );
    if (isLinkedToUplink) return false;
    return true;
  });

  // Determine which message to display
  const effectiveIndex =
    viewIndex >= 0 && viewIndex < visibleMessages.length
      ? viewIndex
      : visibleMessages.length - 1;
  const currentMessage: DcduMessage | null =
    visibleMessages.length > 0 ? visibleMessages[effectiveIndex] : null;
  const totalMessages = visibleMessages.length;

  // Navigate message history
  const goNext = () => {
    if (effectiveIndex < visibleMessages.length - 1) setViewIndex(effectiveIndex + 1);
  };
  const goPrev = () => {
    if (effectiveIndex > 0) setViewIndex(effectiveIndex - 1);
  };

  // Handle response button press (triggered by physical side button).
  // The pilot can respond again after STANDBY (status="sent") — only
  // block while a response is in-flight ("responding").
  // The server is the source of truth for dialogue state;
  // we never close a dialogue locally.
  const handleResponse = (intent: ResponseIntent) => {
    if (!currentMessage) return;
    if (currentMessage.status === "responding") return;
    onSendResponse(currentMessage.id, intent.label, intent.downlinkId);
  };

  // ── Draft handling ───────────────────────────────────────────────
  const isDraft = currentMessage?.status === "draft";
  const isDraftSending = currentMessage?.isOutgoing && currentMessage?.status === "sending";

  const handleSendDraft = () => {
    if (!currentMessage || !isDraft) return;
    onSendDraft(currentMessage.id);
  };

  // ── Build the response grid for current message ──────────────────
  // We need it BOTH for screen labels AND for physical button actions.
  // Show response buttons for any non-definitive state.
  // "sent" is included because STANDBY is not a definitive response —
  // the pilot can still send WILCO/UNABLE after STANDBY.
  // The server tells us when the dialogue truly closes.
  // Find linked draft response for current uplink (composed from MCDU, not yet sent)
  const linkedDraft =
    currentMessage &&
    !currentMessage.isOutgoing &&
    currentMessage.min != null
      ? messages.find(
          (m) =>
            m.isOutgoing &&
            m.mrn === currentMessage.min &&
            (m.status === "draft" || m.status === "sending")
        ) ?? null
      : null;

  // When STANDBY has already been sent (status="sent"), filter it out
  // so the pilot can't send STANDBY twice.
  const effectiveIntents =
    currentMessage && currentMessage.status === "sent"
      ? currentMessage.responseIntents.filter(
          (i) => i.label.toUpperCase() !== "STANDBY"
        )
      : currentMessage?.responseIntents ?? [];

  // If there's a linked draft pending, show SEND* instead of response buttons
  const showResponses =
    currentMessage &&
    !currentMessage.isOutgoing &&
    !linkedDraft &&
    effectiveIntents.length > 0 &&
    (currentMessage.status === "open" ||
      currentMessage.status === "new" ||
      currentMessage.status === "responding" ||
      currentMessage.status === "sent");

  const grid = showResponses
    ? arrangeResponses(effectiveIntents)
    : { topLeft: null, topRight: null, botLeft: null, botRight: null };

  const isResponding = currentMessage?.status === "responding";
  const respondedWith = currentMessage?.respondedWith;

  // Handle sending a linked draft response
  const handleSendLinkedDraft = () => {
    if (!linkedDraft) return;
    onSendDraft(linkedDraft.id);
  };

  // ──────────────────────────────────────────────────────────────────
  // Page scroll (PGE-/PGE+)
  // ──────────────────────────────────────────────────────────────────
  const upperRef = useRef<HTMLDivElement>(null);

  const pageUp = () => {
    const el = upperRef.current;
    if (el) el.scrollBy({ top: -el.clientHeight * 0.8, behavior: "smooth" });
  };
  const pageDown = () => {
    const el = upperRef.current;
    if (el) el.scrollBy({ top: el.clientHeight * 0.8, behavior: "smooth" });
  };

  // ──────────────────────────────────────────────────────────────────
  // Render
  // ──────────────────────────────────────────────────────────────────

  return (
    <div className="dcdu">
      {/* Decorative corner screws */}
      <span className="dcdu-corner tl">⊕</span>
      <span className="dcdu-corner tr">⊕</span>
      <span className="dcdu-corner bl">⊕</span>
      <span className="dcdu-corner br">⊕</span>

      <div className="dcdu-screen">
        {/* ── LEFT SIDE BUTTONS (physical) ───────────────────── */}
        {/* Top 4: fixed BRT/DIM/MSG-/MSG+                       */}
        {/* Bottom 2: context-sensitive — mapped to response grid */}
        <div className="dcdu-buttons-left">
          <button className="dcdu-btn">BRT</button>
          <button className="dcdu-btn">DIM</button>
          <div className="dcdu-btn-spacer" />
          <button className="dcdu-btn" onClick={goPrev}>
            MSG-
          </button>
          <button className="dcdu-btn" onClick={goNext}>
            MSG+
          </button>
          <div className="dcdu-btn-spacer" />
          {/* L5: mapped to response grid top-left (e.g. *UNABLE) */}
          <PhysicalResponseBtn
            intent={grid.topLeft}
            isResponding={!!isResponding}
            respondedWith={respondedWith}
            onPress={handleResponse}
          />
          {/* L6: mapped to response grid bottom-left (e.g. <OTHER) */}
          <PhysicalResponseBtn
            intent={grid.botLeft}
            isResponding={!!isResponding}
            respondedWith={respondedWith}
            onPress={handleResponse}
          />
        </div>

        {/* ── MAIN DISPLAY ──────────────────────────────────── */}
        <div className="dcdu-display">
          {/* === UPPER: message content (display only) === */}
          <div className="dcdu-upper" ref={upperRef}>
            {currentMessage ? (
              <MessageView
                msg={currentMessage}
                linkedResponses={
                  !currentMessage.isOutgoing && currentMessage.min != null
                    ? messages.filter(
                        (m) => m.id !== currentMessage.id && m.mrn === currentMessage.min
                      )
                    : []
                }
              />
            ) : (
              <IdleView session={session} />
            )}
          </div>

          {/* Horizontal separator */}
          <div className="dcdu-separator" />

          {/* === LOWER: response labels (display only) + status === */}
          <LowerZone
            message={currentMessage}
            totalMessages={totalMessages}
            effectiveIndex={effectiveIndex}
            grid={grid}
            showResponses={!!showResponses}
            isResponding={!!isResponding}
            respondedWith={respondedWith}
            isDraft={!!isDraft}
            isDraftSending={!!isDraftSending}
            hasLinkedDraft={!!linkedDraft}
            linkedDraftSending={linkedDraft?.status === "sending"}
          />
        </div>

        {/* ── RIGHT SIDE BUTTONS (physical) ──────────────────── */}
        {/* Top 3: fixed PRINT/PGE-/PGE+                         */}
        {/* Bottom 2: context-sensitive — mapped to response grid  */}
        <div className="dcdu-buttons-right">
          <button className="dcdu-btn">PRINT</button>
          <div className="dcdu-btn-spacer" />
          <button className="dcdu-btn" onClick={pageUp}>PGE-</button>
          <button className="dcdu-btn" onClick={pageDown}>PGE+</button>
          <div className="dcdu-btn-spacer" />
          {/* R5: mapped to response grid top-right (e.g. STBY*) */}
          {isDraft || linkedDraft ? (
            <button className="dcdu-btn" disabled>―</button>
          ) : (
            <PhysicalResponseBtn
              intent={grid.topRight}
              isResponding={!!isResponding}
              respondedWith={respondedWith}
              onPress={handleResponse}
            />
          )}
          {/* R6: SEND* for drafts or linked drafts, or mapped to response grid bottom-right */}
          {isDraft ? (
            <button className="dcdu-btn action" onClick={handleSendDraft}>―</button>
          ) : linkedDraft ? (
            <button className="dcdu-btn action" onClick={handleSendLinkedDraft}>―</button>
          ) : (
            <PhysicalResponseBtn
              intent={grid.botRight}
              isResponding={!!isResponding}
              respondedWith={respondedWith}
              onPress={handleResponse}
            />
          )}
        </div>
      </div>
    </div>
  );
}

// ──────────────────────────────────────────────────────────────────────
// Idle view (no messages)
// ──────────────────────────────────────────────────────────────────────

function IdleView({ session }: { session: CpdlcSessionView | null }) {
  const activeAtc = session?.active_connection?.peer;
  const phase = session?.active_connection?.phase;

  return (
    <div className="dcdu-idle">
      {activeAtc ? (
        <>
          <div className="dcdu-status-line">
            ATC: {activeAtc} [{phase}]
          </div>
          <div className="dcdu-status-sub">NO MESSAGES</div>
        </>
      ) : (
        <>
          <div className="dcdu-status-line">DATALINK</div>
          <div className="dcdu-status-line">NOT CONNECTED</div>
          <div className="dcdu-status-sub">USE MCDU TO LOGON</div>
        </>
      )}
    </div>
  );
}

// ──────────────────────────────────────────────────────────────────────
// Message view (upper zone)
// ──────────────────────────────────────────────────────────────────────

function MessageView({ msg, linkedResponses = [] }: { msg: DcduMessage; linkedResponses?: DcduMessage[] }) {
  const timestamp = msg.timestamp
    .toISOString()
    .slice(11, 16)
    .replace(":", "");
  const directionLabel = msg.isOutgoing ? "TO" : "FROM";

  // Sort linked responses by timestamp
  const sortedResponses = [...linkedResponses].sort(
    (a, b) => a.timestamp.getTime() - b.timestamp.getTime()
  );

  return (
    <>
      {/* Header: "0806Z FROM EDYY CTL   [OPEN]" */}
      <div className="dcdu-header">
        <span className="dcdu-header-left">
          <span className="dcdu-timestamp">{timestamp}Z</span>
          <span className="dcdu-direction">{directionLabel}</span>
          <span className="dcdu-station">{msg.from}</span>
        </span>
        <StatusBadge msg={msg} />
      </div>

      {/* Message body — always green/cyan, no dimming */}
      <div className="dcdu-body">
        <MessageText msg={msg} />
      </div>

      {/* All linked responses in the dialog chain (pilot responses + ATC closures) */}
      {sortedResponses.map((resp, i) => {
        // Only outgoing (pilot) responses get highlighted background.
        // Incoming ATC messages stay in normal green text.
        const isOutgoing = resp.isOutgoing;
        const blockClass = !isOutgoing
          ? "dcdu-body"
          : resp.status === "draft"
          ? "dcdu-response-block draft"
          : resp.status === "sending"
          ? "dcdu-response-block sending"
          : "dcdu-response-block";
        return (
          <div key={resp.id || i}>
            <div className="dcdu-dialogue-sep">{"------------------------"}</div>
            <div className={blockClass}>
              <MessageText msg={resp} />
            </div>
          </div>
        );
      })}
    </>
  );
}

// ──────────────────────────────────────────────────────────────────────
// Status badge (top-right corner)
//
// - "OPEN"  → message awaiting pilot response
// - "WILCO" (green highlight) → pilot has responded
// - "SENDING" / "SENT" → outgoing message status
// ──────────────────────────────────────────────────────────────────────

function StatusBadge({ msg }: { msg: DcduMessage }) {
  const isResponded = msg.status === "responded";
  const isResponding = msg.status === "responding";

  // Responded: show response label (still inverted style — same as all badges)
  if (isResponded && msg.respondedWith) {
    return (
      <span className="dcdu-status-badge">{displayLabel(msg.respondedWith)}</span>
    );
  }
  // Responding: in-flight — pulsing
  if (isResponding && msg.respondedWith) {
    return (
      <span className="dcdu-status-badge responding">{displayLabel(msg.respondedWith)}</span>
    );
  }
  // Sent (non-definitive, e.g. STANDBY): show STBY badge in green highlight.
  if (msg.status === "sent" && !msg.isOutgoing && msg.respondedWith) {
    return (
      <span className="dcdu-status-badge">{displayLabel(msg.respondedWith)}</span>
    );
  }
  // Outgoing messages
  if (msg.isOutgoing) {
    const label =
      msg.status === "sent"
        ? "SENT"
        : msg.status === "sending"
        ? "SENDING"
        : "";
    if (label) return <span className="dcdu-status-badge">{label}</span>;
    return null;
  }
  // Default: OPEN
  return <span className="dcdu-status-badge open">OPEN</span>;
}

// ──────────────────────────────────────────────────────────────────────
// Message text with colored parts
//
// - Static text → white
// - Parameters → blue (#00bfff)
// - Responded message → everything green
// ──────────────────────────────────────────────────────────────────────

function MessageText({ msg }: { msg: DcduMessage }) {
  // Rich text parts rendering — always green/cyan regardless of status
  if (msg.textParts.length > 0) {
    return (
      <div className="dcdu-message-text">
        {msg.textParts.map((part: TextPart, i: number) => (
          <span
            key={i}
            className={part.isParam ? "dcdu-text-param" : "dcdu-text-static"}
          >
            {part.text}
          </span>
        ))}
      </div>
    );
  }

  // Fallback: plain text
  return (
    <div className="dcdu-message-text">
      <span className="dcdu-text-static">{msg.text}</span>
    </div>
  );
}

// ──────────────────────────────────────────────────────────────────────
// Lower zone — fixed 2-line height (DISPLAY ONLY)
//
// This zone displays the response labels aligned to screen edges.
// The labels correspond to the physical side buttons — they are NOT
// clickable on screen. The pilot clicks the PHYSICAL button next to
// the label to activate a response.
//
// Layout:
//   Row 1: *UNABLE        | status |       STBY*
//   Row 2: (empty)        | PG 1 2 |      WILCO*
// ──────────────────────────────────────────────────────────────────────

function LowerZone({
  message,
  totalMessages,
  effectiveIndex,
  grid,
  showResponses,
  isResponding,
  respondedWith,
  isDraft,
  isDraftSending,
  hasLinkedDraft,
  linkedDraftSending,
}: {
  message: DcduMessage | null;
  totalMessages: number;
  effectiveIndex: number;
  grid: ResponseGrid;
  showResponses: boolean;
  isResponding: boolean;
  respondedWith?: string;
  isDraft: boolean;
  isDraftSending: boolean;
  hasLinkedDraft?: boolean;
  linkedDraftSending?: boolean;
}) {
  // Draft or linked draft: show SEND* on bottom-right
  if (isDraft || isDraftSending || hasLinkedDraft) {
    const isSending = isDraftSending || linkedDraftSending;
    return (
      <div className="dcdu-lower-zone">
        <div className="dcdu-lower-row">
          <div className="dcdu-lower-cell left" />
          <div className="dcdu-lower-center">
            <span className="dcdu-lower-status">
              {isSending ? "SENDING" : ""}
            </span>
          </div>
          <div className="dcdu-lower-cell right" />
        </div>
        <div className="dcdu-lower-row">
          <div className="dcdu-lower-cell left" />
          <div className="dcdu-lower-center">
            <Pagination total={totalMessages} current={effectiveIndex} />
          </div>
          <div className="dcdu-lower-cell right dcdu-resp-label">SEND*</div>
        </div>
      </div>
    );
  }

  if (showResponses) {
    return (
      <div className="dcdu-lower-zone">
        {/* Row 1: *UNABLE label | status | STBY* label */}
        <div className="dcdu-lower-row">
          <ResponseLabel intent={grid.topLeft} side="left" isResponding={isResponding} respondedWith={respondedWith} />
          <div className="dcdu-lower-center">
            <span className="dcdu-lower-status">
              {isResponding ? "SENDING" : ""}
            </span>
          </div>
          <ResponseLabel intent={grid.topRight} side="right" isResponding={isResponding} respondedWith={respondedWith} />
        </div>
        {/* Row 2: (empty) | pagination | WILCO* label */}
        <div className="dcdu-lower-row">
          <ResponseLabel intent={grid.botLeft} side="left" isResponding={isResponding} respondedWith={respondedWith} />
          <div className="dcdu-lower-center">
            <Pagination total={totalMessages} current={effectiveIndex} />
          </div>
          <ResponseLabel intent={grid.botRight} side="right" isResponding={isResponding} respondedWith={respondedWith} />
        </div>
      </div>
    );
  }

  // Default lower zone when no responses to show
  const lowerStatus =
    message?.status === "responded"
      ? "SENT"
      : message?.status === "sent"
      ? "SENT"
      : message?.status === "sending" || message?.status === "responding"
      ? "SENDING"
      : "";

  return (
    <div className="dcdu-lower-zone">
      <div className="dcdu-lower-row">
        <div className="dcdu-lower-cell left" />
        <div className="dcdu-lower-center">
          <span className="dcdu-lower-status">{lowerStatus}</span>
        </div>
        <div className="dcdu-lower-cell right" />
      </div>
      <div className="dcdu-lower-row">
        <div className="dcdu-lower-cell left" />
        <div className="dcdu-lower-center">
          <Pagination total={totalMessages} current={effectiveIndex} />
        </div>
        <div className="dcdu-lower-cell right dcdu-close-label">CLOSE*</div>
      </div>
    </div>
  );
}

// ──────────────────────────────────────────────────────────────────────
// Response label — display-only text on the screen edge
//
// These labels are NOT buttons. They just indicate which response
// is associated with which physical side button. The label is highlighted
// (green bg) when that response is currently being sent.
// ──────────────────────────────────────────────────────────────────────

function ResponseLabel({
  intent,
  side,
  isResponding,
  respondedWith,
}: {
  intent: ResponseIntent | null;
  side: "left" | "right";
  isResponding: boolean;
  respondedWith?: string;
}) {
  if (!intent) {
    return <div className={`dcdu-lower-cell ${side}`} />;
  }

  const isActive = isResponding && respondedWith === intent.label;

  // Left-side labels get * prefix, right-side get * suffix (DCDU convention)
  const text =
    side === "left" ? `*${displayLabel(intent.label)}` : `${displayLabel(intent.label)}*`;

  return (
    <div className={`dcdu-lower-cell ${side} dcdu-resp-label ${isActive ? "active" : ""}`}>
      {text}
    </div>
  );
}

// ──────────────────────────────────────────────────────────────────────
// Physical response button — side bezel button
//
// This is the actual clickable button on the DCDU bezel. When pressed,
// it sends the response and highlights until server confirms.
// ──────────────────────────────────────────────────────────────────────

/** Map internal labels to short display labels for the DCDU screen */
function displayLabel(label: string): string {
  const map: Record<string, string> = { STANDBY: "STBY" };
  return map[label.toUpperCase()] ?? label;
}

function PhysicalResponseBtn({
  intent,
  isResponding,
  respondedWith,
  onPress,
}: {
  intent: ResponseIntent | null;
  isResponding: boolean;
  respondedWith?: string;
  onPress: (intent: ResponseIntent) => void;
}) {
  // Empty slot — render an inactive (but always visible) physical button
  if (!intent) {
    return <button className="dcdu-btn" disabled>―</button>;
  }

  const isActive = isResponding && respondedWith === intent.label;

  return (
    <button
      className={`dcdu-btn action ${isActive ? "active" : ""}`}
      onClick={() => onPress(intent)}
      disabled={isResponding}
    >
      ―
    </button>
  );
}

// ──────────────────────────────────────────────────────────────────────
// Pagination indicator: PG 1 2 3 ...
// ──────────────────────────────────────────────────────────────────────

function Pagination({ total, current }: { total: number; current: number }) {
  if (total <= 1) return null;
  return (
    <span className="dcdu-pagination">
      PG{" "}
      {Array.from({ length: Math.min(total, 9) }, (_, i) => (
        <span key={i} className={i === current ? "pg-active" : "pg-inactive"}>
          {i + 1}
        </span>
      ))}
    </span>
  );
}
