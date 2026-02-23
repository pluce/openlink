import type { CpdlcArgument, MessageElement, ResponseIntent, TextPart } from "./types";
import {
  chooseShortResponseIntents,
  type CatalogEntryForRuntime,
  type CatalogResponseIntent,
} from "./cpdlc-runtime";

export interface CatalogEntry extends CatalogEntryForRuntime {
  direction: "Uplink" | "Downlink";
  template: string;
  args: string[];
  response_attr: string;
  short_response_intents: CatalogResponseIntent[];
}

let catalogMap: Map<string, CatalogEntry> | null = null;

export function loadCatalog(entries: CatalogEntry[]): void {
  catalogMap = new Map();
  for (const entry of entries) {
    catalogMap.set(entry.id, entry);
  }
}

export function getCatalogEntry(id: string): CatalogEntry | undefined {
  return catalogMap?.get(id);
}

export function templateToTextParts(template: string, args: CpdlcArgument[]): TextPart[] {
  const parts: TextPart[] = [];
  const regex = /\[([^\]]+)\]/g;
  let lastIndex = 0;
  let argIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = regex.exec(template)) !== null) {
    if (match.index > lastIndex) {
      parts.push({ text: template.slice(lastIndex, match.index), isParam: false });
    }
    const argValue =
      argIndex < args.length ? formatArgValue(args[argIndex]) : match[1].toUpperCase();
    parts.push({ text: argValue, isParam: true });
    argIndex++;
    lastIndex = regex.lastIndex;
  }

  if (lastIndex < template.length) {
    parts.push({ text: template.slice(lastIndex), isParam: false });
  }

  if (parts.length === 0) {
    parts.push({ text: template, isParam: false });
  }

  return parts;
}

function formatArgValue(arg: CpdlcArgument): string {
  if (arg.type === "Level" && typeof arg.value === "number") {
    return arg.value <= 999 ? `FL${arg.value}` : String(arg.value);
  }
  return String(arg.value);
}

export function elementsToTextParts(elements: MessageElement[]): TextPart[] {
  const allParts: TextPart[] = [];

  for (let i = 0; i < elements.length; i++) {
    const el = elements[i];
    const entry = getCatalogEntry(el.id);

    if (entry) {
      allParts.push(...templateToTextParts(entry.template, el.args));
    } else {
      allParts.push({ text: el.id, isParam: false });
      for (const arg of el.args) {
        allParts.push({ text: " ", isParam: false });
        allParts.push({ text: formatArgValue(arg), isParam: true });
      }
    }

    if (i < elements.length - 1) {
      allParts.push({ text: " // ", isParam: false });
    }
  }

  return allParts;
}

export function textPartsToString(parts: TextPart[]): string {
  return parts.map((p) => p.text).join("");
}

export function getResponseIntents(elements: MessageElement[]): ResponseIntent[] {
  if (elements.length === 0) return [];
  return chooseShortResponseIntents(elements, getCatalogEntry);
}
