/**
 * catalog.ts — CPDLC catalog helpers for the A320 DCDU.
 *
 * Provides functions to:
 * - Parse a CPDLC message template + arguments into TextParts (white text
 *   with blue parameter values).
 * - Derive available DCDU response intents from a catalog entry's
 *   `short_response_intents` field.
 *
 * The A320 DCDU displays controller messages with parameters highlighted
 * in a different color (blue/cyan) from the static text (white).
 */

import type {
  TextPart,
  ResponseIntent,
  MessageElement,
  CpdlcArgument,
} from "./types";

// ──────────────────────────────────────────────────────────────────────
// Catalog entry shape (minimal subset of catalog.v1.json)
// ──────────────────────────────────────────────────────────────────────

interface CatalogResponseIntent {
  intent: string;
  label: string;
  uplink_id: string;
  downlink_id: string;
}

interface CatalogEntry {
  id: string;
  direction: "Uplink" | "Downlink";
  template: string;
  args: string[];
  response_attr: string;
  short_response_intents: CatalogResponseIntent[];
}

// ──────────────────────────────────────────────────────────────────────
// Embedded catalog — loaded at build time from catalog.v1.json
// In a real app you'd fetch this or bundle it; here we import it.
// ──────────────────────────────────────────────────────────────────────

// We'll load the catalog lazily. A real implementation would use
// `import catalogData from '../../spec/cpdlc/catalog.v1.json'`.
// For now, we use a map populated on first use.
let catalogMap: Map<string, CatalogEntry> | null = null;

/**
 * Registers the full catalog (call once at startup with the JSON data).
 */
export function loadCatalog(entries: CatalogEntry[]): void {
  catalogMap = new Map();
  for (const entry of entries) {
    catalogMap.set(entry.id, entry);
  }
}

/**
 * Returns the catalog entry for a message element id (e.g. "UM19").
 */
export function getCatalogEntry(id: string): CatalogEntry | undefined {
  return catalogMap?.get(id);
}

// ──────────────────────────────────────────────────────────────────────
// Template → TextParts conversion
// ──────────────────────────────────────────────────────────────────────

/**
 * Converts a CPDLC message template string and its arguments into an
 * array of TextParts for colored rendering on the DCDU.
 *
 * Example:
 *   template = "CLIMB TO [level]"
 *   args     = [{ type: "Level", value: "FL340" }]
 *   → [
 *       { text: "CLIMB TO ", isParam: false },
 *       { text: "FL340",     isParam: true  },
 *     ]
 */
export function templateToTextParts(
  template: string,
  args: CpdlcArgument[]
): TextPart[] {
  const parts: TextPart[] = [];
  // Match placeholders like [level], [position], [free text], etc.
  const regex = /\[([^\]]+)\]/g;
  let lastIndex = 0;
  let argIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = regex.exec(template)) !== null) {
    // Static text before the placeholder
    if (match.index > lastIndex) {
      parts.push({ text: template.slice(lastIndex, match.index), isParam: false });
    }
    // Substitute the argument value (or keep placeholder if missing)
    const argValue =
      argIndex < args.length
        ? formatArgValue(args[argIndex])
        : match[1].toUpperCase();
    parts.push({ text: argValue, isParam: true });
    argIndex++;
    lastIndex = regex.lastIndex;
  }

  // Remaining static text after last placeholder
  if (lastIndex < template.length) {
    parts.push({ text: template.slice(lastIndex), isParam: false });
  }

  // If no placeholders, the whole template is static text
  if (parts.length === 0) {
    parts.push({ text: template, isParam: false });
  }

  return parts;
}

/**
 * Formats a CPDLC argument value for display.
 * Levels ≤999 → "FL350", altitudes >999 → "4000", others → string as-is.
 */
function formatArgValue(arg: CpdlcArgument): string {
  if (arg.type === "Level" && typeof arg.value === "number") {
    return arg.value <= 999 ? "FL" + arg.value : String(arg.value);
  }
  return String(arg.value);
}

// ──────────────────────────────────────────────────────────────────────
// Elements → rich display
// ──────────────────────────────────────────────────────────────────────

/**
 * Converts a list of MessageElements into an array of TextParts
 * by looking up each element in the catalog and substituting args.
 *
 * Multiple elements are joined with " // " separator (CPDLC convention).
 */
export function elementsToTextParts(elements: MessageElement[]): TextPart[] {
  const allParts: TextPart[] = [];

  for (let i = 0; i < elements.length; i++) {
    const el = elements[i];
    const entry = getCatalogEntry(el.id);

    if (entry) {
      // Use the catalog template for proper formatting
      const parts = templateToTextParts(entry.template, el.args);
      allParts.push(...parts);
    } else {
      // Fallback: show element id + argument values
      allParts.push({ text: el.id, isParam: false });
      for (const arg of el.args) {
        allParts.push({ text: " ", isParam: false });
        allParts.push({ text: formatArgValue(arg), isParam: true });
      }
    }

    // Separator between elements
    if (i < elements.length - 1) {
      allParts.push({ text: " // ", isParam: false });
    }
  }

  return allParts;
}

/**
 * Builds a flat plain-text representation from TextParts (for fallback).
 */
export function textPartsToString(parts: TextPart[]): string {
  return parts.map((p) => p.text).join("");
}

// ──────────────────────────────────────────────────────────────────────
// Response intent extraction
// ──────────────────────────────────────────────────────────────────────

/**
 * Response attribute priority (highest first).
 * WU (WILCO/UNABLE) > AN (AFFIRM/NEGATIVE) > R (ROGER) > Y (specific) > N (none)
 */
const RESPONSE_ATTR_PRIORITY: Record<string, number> = {
  WU: 5,
  AN: 4,
  R: 3,
  Y: 2,
  N: 1,
};

/**
 * Gets available response intents for a multi-element message.
 * Scans ALL elements and picks the one with the highest-priority
 * response attribute (WU > AN > R > Y > N).
 */
export function getResponseIntents(
  elements: MessageElement[]
): ResponseIntent[] {
  if (elements.length === 0) return [];

  // Find the element with the highest-priority response attribute
  let bestEntry: CatalogEntry | undefined;
  let bestPriority = 0;

  for (const el of elements) {
    const entry = getCatalogEntry(el.id);
    if (!entry) continue;
    const prio = RESPONSE_ATTR_PRIORITY[entry.response_attr] ?? 0;
    if (prio > bestPriority) {
      bestPriority = prio;
      bestEntry = entry;
    }
  }

  if (!bestEntry) return defaultWUIntents();

  // Use the winning entry's pre-computed short_response_intents
  if (bestEntry.short_response_intents.length > 0) {
    return bestEntry.short_response_intents.map((sri) => ({
      label: sri.label,
      downlinkId: sri.downlink_id,
    }));
  }

  // Fallback: derive from response_attr
  return responseAttrToIntents(bestEntry.response_attr);
}

/**
 * Maps the response_attr code to default response intents.
 */
function responseAttrToIntents(attr: string): ResponseIntent[] {
  switch (attr) {
    case "WU":
      return defaultWUIntents();
    case "AN":
      return [
        { label: "AFFIRM", downlinkId: "DM4" },
        { label: "NEGATIVE", downlinkId: "DM5" },
        { label: "STANDBY", downlinkId: "DM2" },
      ];
    case "R":
      return [
        { label: "ROGER", downlinkId: "DM3" },
        { label: "STANDBY", downlinkId: "DM2" },
      ];
    case "Y":
      // Yes: requires a specific downlink reply (handled by MCDU)
      return [{ label: "STANDBY", downlinkId: "DM2" }];
    case "N":
      // No response required
      return [];
    default:
      return [];
  }
}

function defaultWUIntents(): ResponseIntent[] {
  return [
    { label: "WILCO", downlinkId: "DM0" },
    { label: "UNABLE", downlinkId: "DM1" },
    { label: "STANDBY", downlinkId: "DM2" },
  ];
}
