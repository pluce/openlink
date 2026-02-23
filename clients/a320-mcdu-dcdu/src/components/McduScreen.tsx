/**
 * McduScreen.tsx — Multifunction Control and Display Unit (MCDU) display.
 *
 * Faithful reproduction of the A320 MCDU screen layout for CPDLC message
 * composition. The pilot navigates sub-pages (LAT REQ, VERT REQ, TEXT, etc.),
 * fills argument fields via the scratchpad, and accumulates message elements.
 * Once composed, pressing "XFR TO DCDU" transfers the draft to the DCDU.
 *
 * ## Message composition flow
 *
 *   1. Pilot navigates to a request page (e.g. VERT REQ)
 *   2. Types a value in the scratchpad (e.g. "FL380")
 *   3. Presses an LSK → `addElement(dmId, argType)` parses the value,
 *      stores it in `fieldValues`, and pushes a `MessageElement` into
 *      `pendingElements`.
 *   4. Pilot can navigate to other pages and repeat — elements accumulate.
 *   5. On ATC MENU (or any sub-page footer), pressing XFR TO DCDU calls
 *      `doTransferToDcdu()` → all pending elements are sent to the DCDU
 *      as a single draft message.
 *
 * ## Screen grid layout
 *
 *   24 characters wide × 14 lines tall (monospace).
 *
 *   Line  1        — Title (large font, centred)
 *   Lines 2,4,6,8,10,12 — Labels  (small font 0.75×, non-interactive)
 *   Lines 3,5,7,9,11,13 — Data    (large font, aligned with LSK L1–L6 / R1–R6)
 *   Line  14       — Scratchpad (large font, independent input buffer)
 *
 * Scratchpad behaviour:
 *   - Keyboard input always fills the scratchpad
 *   - Pressing an LSK transfers scratchpad content into the field for that LSK
 *   - The scratchpad is NOT tied to any specific field
 *
 * LSK buttons are **physical** — rendered outside the screen bezel.
 *
 * @see docs/acars-ref-gold/cpdlc_message_reference.md — DM message catalog
 * @see docs/acars-ref-gold/messaging.md — CPDLC message exchange model
 * @see spec/cpdlc/catalog.v1.json — Machine-readable CPDLC catalog
 */

import { useState, useEffect, useCallback } from "react";
import type { CpdlcSessionView, CpdlcArgument, MessageElement } from "../lib/types";

// ──────────────────────────────────────────────────────────────────────
// Types
// ──────────────────────────────────────────────────────────────────────

type McduPage =
  | "ATC_MENU"
  | "CONNECTION_STATUS"
  | "NOTIFICATION"
  | "LAT_REQ"
  | "VERT_REQ"
  | "WHEN_CAN_WE"
  | "OTHER_REQ"
  | "TEXT"
  | "REPORTS";

type McduColor = "white" | "green" | "cyan" | "amber" | "red";

/** One side of a data line (left or right). */
interface CellData {
  text: string;
  color?: McduColor;
  /** If true the cell text is prefixed with ◄ (left) or suffixed with ► (right) */
  arrow?: boolean;
}

/** A label+data row pair (one of the 6 LSK rows). */
interface McduRow {
  /** Small-font label (line 2,4,6,8,10,12) */
  labelLeft?: string;
  labelRight?: string;
  labelLeftColor?: McduColor;
  labelRightColor?: McduColor;
  /** Large-font data (line 3,5,7,9,11,13) — aligned with the physical LSK */
  dataLeft?: CellData;
  dataRight?: CellData;
  /** Callbacks fired when the physical LSK button is pressed */
  onLskLeft?: () => void;
  onLskRight?: () => void;
}

interface McduScreenProps {
  callsign: string;
  session: CpdlcSessionView | null;
  onLogonRequest: (station: string) => void;
  onTransferToDcdu: (elements: MessageElement[]) => void;
}

// ──────────────────────────────────────────────────────────────────────
// Component
// ──────────────────────────────────────────────────────────────────────

export default function McduScreen({
  callsign,
  session,
  onLogonRequest,
  onTransferToDcdu,
}: McduScreenProps) {
  const [currentPage, setCurrentPage] = useState<McduPage>("ATC_MENU");
  const [scratchpad, setScratchpad] = useState("");

  // Field values filled via scratchpad transfer
  const [atcCenter, setAtcCenter] = useState("");

  // Pending message elements for multi-element composition
  const [pendingElements, setPendingElements] = useState<MessageElement[]>([]);

  // Per-field stored values for request pages (keyed by field id like "DM22", "DM9")
  const [fieldValues, setFieldValues] = useState<Record<string, string>>({});

  // ──────────────────────────────────────────────────────────────────
  // Keyboard → scratchpad
  // ──────────────────────────────────────────────────────────────────

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (
      e.target instanceof HTMLInputElement ||
      e.target instanceof HTMLTextAreaElement
    )
      return;

    if (e.key === "Backspace") {
      e.preventDefault();
      setScratchpad((p) => p.slice(0, -1));
    } else if (e.key === "Delete" || e.key === "Escape") {
      e.preventDefault();
      setScratchpad("");
    } else if (e.key.length === 1 && /[A-Za-z0-9 /.]/.test(e.key)) {
      e.preventDefault();
      setScratchpad((p) => (p.length >= 24 ? p : p + e.key.toUpperCase()));
    }
  }, []);

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  // ──────────────────────────────────────────────────────────────────
  // LSK scratchpad transfer helper
  // ──────────────────────────────────────────────────────────────────

  /** Transfer scratchpad content to a field setter, then clear scratchpad. */
  const transferScratchpad = (setter: (v: string) => void) => {
    if (!scratchpad) return;
    setter(scratchpad);
    setScratchpad("");
  };

  /**
   * Parse scratchpad value and store it in a field + add to pending elements.
   * The message is NOT sent to DCDU yet — pilot must press XFR TO DCDU.
   *
   * @param dmId   — CPDLC Downlink Message identifier (e.g. "DM9" = REQUEST CLIMB
   *                  TO [level]). Maps to an entry in `catalog.v1.json`.
   *                  @see docs/acars-ref-gold/cpdlc_message_reference.md
   * @param argType — Argument type expected by this DM message. Determines how
   *                  the scratchpad value is parsed:
   *                  • "Level"    — Strip FL prefix, parseInt. ≤999 → FL, >999 → altitude ft
   *                  • "Speed"    — parseInt (knots or Mach)
   *                  • "Degrees"  — parseInt (heading / ground track)
   *                  • "Position" — String as-is (waypoint name)
   *                  • "FreeText" — String as-is
   *                  • undefined  — No argument (e.g. DM65 DUE TO WEATHER)
   *
   * Dedup policy:
   *   - DM67 (free text) and no-arg messages allow stacking (multiple instances)
   *   - All others replace any existing element with the same DM ID
   */
  const addElement = (dmId: string, argType?: string) => {
    const args: CpdlcArgument[] = [];
    if (argType) {
      if (!scratchpad) return; // Need a value in the scratchpad
      let value: string | number = scratchpad;
      if (argType === "Level") {
        // FL340 → 340, 4000 → 4000 (0-999 = FL, >999 = altitude ft)
        const raw = scratchpad.toUpperCase().replace(/^FL/, "");
        const num = parseInt(raw, 10);
        if (isNaN(num)) return;
        value = num;
      } else if (argType === "Speed" || argType === "Degrees") {
        const num = parseInt(scratchpad, 10);
        if (isNaN(num)) return;
        value = num;
      }
      args.push({ type: argType, value });
      // Store the display value for this field
      setFieldValues((prev) => ({ ...prev, [dmId]: scratchpad.toUpperCase() }));
      setScratchpad("");
    }
    // Add to pending elements
    // DM67 (free text) and no-arg messages (DM65, DM66, etc.) can be added multiple times
    // Others replace existing entry with same ID
    const allowDuplicates = dmId === "DM67" || args.length === 0;
    setPendingElements((prev) => {
      if (allowDuplicates) return [...prev, { id: dmId, args }];
      const filtered = prev.filter((e) => e.id !== dmId);
      return [...filtered, { id: dmId, args }];
    });
  };

  /**
   * Transfer all pending elements to the DCDU as a draft.
   *
   * Calls `onTransferToDcdu(elements)` which (in useOpenLink) creates a
   * DcduMessage with status "draft" and auto-links it to the most recent
   * OPEN uplink via MRN if one exists.
   * @see docs/acars-ref-gold/messaging.md — dialog linking via MIN/MRN
   */
  const doTransferToDcdu = () => {
    if (pendingElements.length === 0) return;
    onTransferToDcdu(pendingElements);
    setPendingElements([]);
    setFieldValues({});
    setCurrentPage("ATC_MENU");
  };

  /**
   * Erase all pending elements and field values.
   */
  const eraseInputs = () => {
    setPendingElements([]);
    setFieldValues({});
    setScratchpad("");
  };

  /** Get field display value: stored value from fieldValues or placeholder. */
  const fieldDisplay = (dmId: string, placeholder = "[     ]"): string => {
    return fieldValues[dmId] || placeholder;
  };

  /** Color for a field: green if filled, cyan if still a placeholder. */
  const fieldColor = (dmId: string): McduColor => {
    return fieldValues[dmId] ? "green" : "cyan";
  };

  /** Whether there are pending elements to transfer. */
  const hasPending = pendingElements.length > 0;

  /**
   * Standard footer rows shared by all sub-pages (ERASE + RETURN / XFR TO DCDU).
   *
   * Row 5: ERASE — clears all pending elements and field values.
   * Row 6: RETURN (L6) — navigates back to ATC MENU without losing pending elements.
   *         XFR TO DCDU (R6) — visible only when `pendingElements.length > 0`;
   *         transfers the composed multi-element message to the DCDU as a draft.
   */
  const subPageFooter = (): McduRow[] => [
    {
      labelLeft: "INPUTS",
      dataLeft: { text: "ERASE", color: "cyan" },
      onLskLeft: eraseInputs,
    },
    {
      labelLeft: "ATC MENU",
      dataLeft: { text: "RETURN", arrow: true, color: "white" },
      labelRight: hasPending ? "XFR TO" : undefined,
      labelRightColor: "cyan" as McduColor,
      dataRight: hasPending ? { text: "DCDU", arrow: true, color: "cyan" } : undefined,
      onLskLeft: () => setCurrentPage("ATC_MENU"),
      onLskRight: hasPending ? doTransferToDcdu : undefined,
    },
  ];

  // ──────────────────────────────────────────────────────────────────
  // Page data builders
  // ──────────────────────────────────────────────────────────────────

  const getPageData = (): {
    title: string;
    titleRight?: string;
    rows: McduRow[];
    annunciators?: { fm1?: boolean; ind?: boolean; rdy?: boolean; fm2?: boolean };
  } => {
    switch (currentPage) {
      // ────────────────── ATC MENU ──────────────────
      case "ATC_MENU":
        return {
          title: "ATC MENU",
          annunciators: { fm1: true, ind: true, rdy: true, fm2: true },
          rows: [
            {
              dataLeft:  { text: "LAT REQ",  arrow: true, color: "white" },
              dataRight: { text: "VERT REQ", arrow: true, color: "white" },
              onLskLeft:  () => setCurrentPage("LAT_REQ"),
              onLskRight: () => setCurrentPage("VERT_REQ"),
            },
            {},
            {
              dataLeft:  { text: "WHEN CAN WE", arrow: true, color: "white" },
              dataRight: { text: "OTHER REQ", arrow: true, color: "white" },
              onLskLeft:  () => setCurrentPage("WHEN_CAN_WE"),
              onLskRight: () => setCurrentPage("OTHER_REQ"),
            },
            {
              dataRight: { text: "TEXT", arrow: true, color: "white" },
              onLskRight: () => setCurrentPage("TEXT"),
            },
            {
              dataLeft:  { text: "MSG LOG",  arrow: true, color: "white" },
              dataRight: { text: "REPORTS",  arrow: true, color: "white" },
              onLskRight: () => setCurrentPage("REPORTS"),
            },
            {
              dataLeft:  { text: "NOTIF", arrow: true, color: "white" },
              labelRight: hasPending ? "XFR TO" : undefined,
              labelRightColor: "cyan" as McduColor,
              dataRight: hasPending
                ? { text: `DCDU (${pendingElements.length})`, arrow: true, color: "cyan" }
                : { text: "CONN STATUS", arrow: true, color: "white" },
              onLskLeft:  () => setCurrentPage("NOTIFICATION"),
              onLskRight: hasPending ? doTransferToDcdu : () => setCurrentPage("CONNECTION_STATUS"),
            },
          ],
        };

      // ────────────────── CONNECTION STATUS ──────────────────
      case "CONNECTION_STATUS": {
        const activeAtc = session?.active_connection?.peer ?? "----";
        const activePhase = session?.active_connection?.phase;
        const nextAtc = session?.inactive_connection?.peer ?? session?.next_data_authority ?? "----";

        const activeColor: McduColor =
          activePhase === "Connected"
            ? "green"
            : activePhase === "LoggedOn" || activePhase === "LogonPending"
            ? "cyan"
            : "white";

        return {
          title: "CONNECTION STATUS",
          annunciators: { fm1: true, ind: true, rdy: true, fm2: true },
          rows: [
            {
              labelLeft: "ACTIVE ATC",
              dataLeft: { text: activeAtc, color: activeColor },
            },
            {
              labelLeft: "NEXT ATC",
              dataLeft: { text: nextAtc, color: "white" },
            },
            {},
            {
              dataLeft: { text: "*SET OFF", color: "cyan" },
            },
            {},
            {
              dataLeft: { text: "RETURN", arrow: true, color: "white" },
              dataRight: { text: "NOTIF", arrow: true, color: "white" },
              onLskLeft:  () => setCurrentPage("ATC_MENU"),
              onLskRight: () => setCurrentPage("NOTIFICATION"),
            },
          ],
        };
      }

      // ────────────────── NOTIFICATION ──────────────────
      case "NOTIFICATION": {
        const hasCenter = atcCenter.length >= 3 && atcCenter.length <= 4;
        const canNotify = hasCenter;

        return {
          title: "NOTIFICATION",
          annunciators: { fm1: true, ind: true, rdy: true, fm2: true },
          rows: [
            {
              labelLeft: "ATC FLT NBR",
              dataLeft: { text: callsign, color: "green" },
            },
            {
              labelLeft: "ATC CENTER",
              dataLeft: {
                text: atcCenter || "□□□□",
                color: atcCenter ? "cyan" : "amber",
              },
              dataRight: canNotify
                ? { text: "NOTIFY", arrow: true, color: "cyan" }
                : undefined,
              onLskLeft: () => transferScratchpad(setAtcCenter),
              onLskRight: canNotify
                ? () => {
                    onLogonRequest(atcCenter.toUpperCase());
                  }
                : undefined,
            },
            {},
            {},
            {},
            {
              dataLeft:  { text: "RETURN", arrow: true, color: "white" },
              dataRight: { text: "CONN STATUS", arrow: true, color: "white" },
              onLskLeft:  () => setCurrentPage("ATC_MENU"),
              onLskRight: () => setCurrentPage("CONNECTION_STATUS"),
            },
          ],
        };
      }

      // ────────────────── LAT REQ ──────────────────
      // DM22 = REQUEST DIRECT TO [position]
      // DM27 = REQUEST WEATHER DEVIATION UP TO [distance]
      // DM70 = REQUEST HEADING [degrees]
      // DM65 = DUE TO WEATHER    (no arg, stackable)
      // DM66 = DUE TO A/C PERF   (no arg, stackable)
      case "LAT_REQ":
        return {
          title: "ATC LAT REQ",
          rows: [
            {
              labelLeft: "DIR TO",
              dataLeft: { text: fieldDisplay("DM22"), color: fieldColor("DM22") },
              labelRight: "WX DEV UP TO",
              dataRight: { text: fieldDisplay("DM27"), color: fieldColor("DM27") },
              onLskLeft: () => addElement("DM22", "Position"),
              onLskRight: () => addElement("DM27", "Distance"),
            },
            {
              labelLeft: "HEADING",
              dataLeft: { text: fieldDisplay("DM70", "[   ]") + "°", color: fieldColor("DM70") },
              labelRight: "GROUND TRK",
              dataRight: { text: fieldDisplay("DM70_GT", "[   ]") + "°", color: fieldColor("DM70_GT") },
              onLskLeft: () => addElement("DM70", "Degrees"),
              onLskRight: () => addElement("DM70", "Degrees"),
            },
            {
              labelLeft: "DUE TO",
              dataLeft: { text: "WEATHER", arrow: true, color: "white" },
              labelRight: "DUE TO",
              dataRight: { text: "A/C PERF", arrow: true, color: "white" },
              onLskLeft: () => addElement("DM65"),
              onLskRight: () => addElement("DM66"),
            },
            {
              labelRight: "WHEN CAN WE EXPECT",
              dataRight: { text: "BACK ON ROUTE", arrow: true, color: "white" },
            },
            ...subPageFooter(),
          ],
        };

      // ────────────────── VERT REQ ──────────────────
      // DM9  = REQUEST CLIMB TO [level]
      // DM10 = REQUEST DESCENT TO [level]
      // DM6  = REQUEST [level]  (generic altitude/FL request)
      // DM18 = REQUEST [speed]
      // DM65 = DUE TO WEATHER    (no arg, stackable)
      // DM66 = DUE TO A/C PERF   (no arg, stackable)
      case "VERT_REQ":
        return {
          title: "ATC VERT REQ",
          rows: [
            {
              labelLeft: "CLB TO",
              dataLeft: { text: fieldDisplay("DM9"), color: fieldColor("DM9") },
              labelRight: "SPD",
              dataRight: { text: fieldDisplay("DM18"), color: fieldColor("DM18") },
              onLskLeft: () => addElement("DM9", "Level"),
              onLskRight: () => addElement("DM18", "Speed"),
            },
            {
              labelLeft: "DES TO",
              dataLeft: { text: fieldDisplay("DM10"), color: fieldColor("DM10") },
              onLskLeft: () => addElement("DM10", "Level"),
            },
            {
              labelLeft: "ALT",
              dataLeft: { text: fieldDisplay("DM6"), color: fieldColor("DM6") },
              onLskLeft: () => addElement("DM6", "Level"),
            },
            {
              labelLeft: "DUE TO",
              dataLeft: { text: "WEATHER", arrow: true, color: "white" },
              labelRight: "DUE TO",
              dataRight: { text: "A/C PERF", arrow: true, color: "white" },
              onLskLeft: () => addElement("DM65"),
              onLskRight: () => addElement("DM66"),
            },
            ...subPageFooter(),
          ],
        };

      // ────────────────── WHEN CAN WE ──────────────────
      // DM49 = WHEN CAN WE EXPECT [speed]
      // DM50 = WHEN CAN WE EXPECT [level]
      case "WHEN_CAN_WE":
        return {
          title: "ATC WHEN CAN WE",
          rows: [
            {
              labelLeft: "EXPECT SPEED",
              dataLeft: { text: fieldDisplay("DM49"), color: fieldColor("DM49") },
              onLskLeft: () => addElement("DM49", "Speed"),
            },
            {
              labelLeft: "EXPECT LEVEL",
              dataLeft: { text: fieldDisplay("DM50"), color: fieldColor("DM50") },
              onLskLeft: () => addElement("DM50", "Level"),
            },
            {},
            {},
            ...subPageFooter(),
          ],
        };

      // ────────────────── OTHER REQ ──────────────────
      // DM18 = REQUEST [speed]
      // DM70 = REQUEST HEADING [degrees]
      // DM20 = REQUEST VOICE CONTACT
      // DM25 = REQUEST CLEARANCE
      case "OTHER_REQ":
        return {
          title: "ATC OTHER REQ",
          rows: [
            {
              labelLeft: "SPEED",
              dataLeft: { text: fieldDisplay("DM18_other"), color: fieldColor("DM18_other") },
              onLskLeft: () => addElement("DM18", "Speed"),
            },
            {
              labelLeft: "HEADING",
              dataLeft: { text: fieldDisplay("DM70_other", "[   ]") + "°", color: fieldColor("DM70_other") },
              onLskLeft: () => addElement("DM70", "Degrees"),
            },
            {
              dataLeft: { text: "REQ VOICE", arrow: true, color: "cyan" },
              onLskLeft: () => addElement("DM20"),
            },
            {
              dataLeft: { text: "REQ CLEARANCE", arrow: true, color: "cyan" },
              onLskLeft: () => addElement("DM25"),
            },
            ...subPageFooter(),
          ],
        };

      // ────────────────── TEXT ──────────────────
      // DM67 = [free text]  (stackable — multiple free text lines accumulate)
      case "TEXT":
        return {
          title: "ATC TEXT",
          rows: [
            {
              labelLeft: "FREE TEXT",
              dataLeft: { text: fieldDisplay("DM67", "[           ]"), color: fieldColor("DM67") },
              onLskLeft: () => addElement("DM67", "FreeText"),
            },
            {},
            {},
            {},
            ...subPageFooter(),
          ],
        };

      // ────────────────── REPORTS ──────────────────
      // DM32 = PRESENT LEVEL [level]
      // DM48 = PRESENT POSITION [position]
      // DM34 = PRESENT SPEED [speed]
      case "REPORTS":
        return {
          title: "ATC REPORTS",
          rows: [
            {
              labelLeft: "PRESENT LEVEL",
              dataLeft: { text: fieldDisplay("DM32"), color: fieldColor("DM32") },
              onLskLeft: () => addElement("DM32", "Level"),
            },
            {
              labelLeft: "PRESENT POSITION",
              dataLeft: { text: fieldDisplay("DM48"), color: fieldColor("DM48") },
              onLskLeft: () => addElement("DM48", "Position"),
            },
            {
              labelLeft: "PRESENT SPEED",
              dataLeft: { text: fieldDisplay("DM34"), color: fieldColor("DM34") },
              onLskLeft: () => addElement("DM34", "Speed"),
            },
            {},
            ...subPageFooter(),
          ],
        };
    }
  };

  const { title, titleRight, rows, annunciators } = getPageData();

  // Pad to exactly 6 rows
  const paddedRows: McduRow[] = [...rows];
  while (paddedRows.length < 6) paddedRows.push({});

  // ──────────────────────────────────────────────────────────────────
  // Render
  // ──────────────────────────────────────────────────────────────────

  return (
    <div className="mcdu">
      {/* Annunciator bar */}
      <div className="mcdu-annunciators">
        <span className={`ann ${annunciators?.fm1 ? "on" : ""}`}>FM1</span>
        <span className={`ann ${annunciators?.ind ? "on" : ""}`}>IND</span>
        <span className={`ann rdy ${annunciators?.rdy ? "on" : ""}`}>RDY</span>
        <span className="ann spacer" />
        <span className={`ann ${annunciators?.fm2 ? "on" : ""}`}>FM2</span>
      </div>

      {/* Bezel wraps LSKs + screen */}
      <div className="mcdu-bezel">
        {/* Left LSK column (L1–L6) */}
        <div className="mcdu-lsk-col left">
          {paddedRows.map((row, i) => (
            <button
              key={i}
              className="lsk"
              disabled={!row.onLskLeft}
              onClick={row.onLskLeft}
            />
          ))}
        </div>

        {/* Screen — 14 fixed lines */}
        <div className="mcdu-screen">
          {/* Line 1: Title */}
          <div className="mcdu-row title">
            <span className="mcdu-cell left color-white">{title}</span>
            {titleRight && (
              <span className="mcdu-cell right color-white">{titleRight}</span>
            )}
          </div>

          {/* Lines 2–13: 6 × (label + data) */}
          {paddedRows.map((row, i) => (
            <div key={i} className="mcdu-row-group">
              {/* Label line (small) */}
              <div className="mcdu-row label">
                <span className={`mcdu-cell left color-${row.labelLeftColor ?? "white"}`}>
                  {row.labelLeft ?? ""}
                </span>
                <span className={`mcdu-cell right color-${row.labelRightColor ?? "white"}`}>
                  {row.labelRight ?? ""}
                </span>
              </div>
              {/* Data line (large) */}
              <div className="mcdu-row data">
                <span className={`mcdu-cell left color-${row.dataLeft?.color ?? "white"}`}>
                  {row.dataLeft?.arrow && "◄"}
                  {row.dataLeft?.text ?? ""}
                </span>
                <span className={`mcdu-cell right color-${row.dataRight?.color ?? "white"}`}>
                  {row.dataRight?.text ?? ""}
                  {row.dataRight?.arrow && "►"}
                </span>
              </div>
            </div>
          ))}

          {/* Line 14: Scratchpad */}
          <div className="mcdu-row scratchpad">
            <span className="mcdu-cell left color-white">
              {scratchpad}
            </span>
          </div>
        </div>

        {/* Right LSK column (R1–R6) */}
        <div className="mcdu-lsk-col right">
          {paddedRows.map((row, i) => (
            <button
              key={i}
              className="lsk"
              disabled={!row.onLskRight}
              onClick={row.onLskRight}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
