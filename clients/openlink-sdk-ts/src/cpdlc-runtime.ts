import type { MessageElement, ResponseIntent } from "./types";

export type ResponseAttribute = "WU" | "AN" | "R" | "Y" | "N" | "NE";

export const LOGICAL_ACK_DOWNLINK_ID = "DM100" as const;
export const LOGICAL_ACK_UPLINK_ID = "UM227" as const;

export interface CatalogResponseIntent {
  intent: string;
  label: string;
  uplink_id: string;
  downlink_id: string;
}

export interface CatalogEntryForRuntime {
  id: string;
  response_attr: string;
  short_response_intents: CatalogResponseIntent[];
}

const RESPONSE_ATTR_PRIORITY: Record<string, number> = {
  WU: 5,
  AN: 4,
  R: 3,
  Y: 2,
  N: 1,
  NE: 1,
};

export function isLogicalAckElementId(id: string): boolean {
  return id === "DM100" || id === "UM227";
}

export function messageContainsLogicalAck(elements: MessageElement[]): boolean {
  return elements.some((e) => isLogicalAckElementId(e.id));
}

export function shouldAutoSendLogicalAck(elements: MessageElement[], min: number): boolean {
  return min > 0 && !messageContainsLogicalAck(elements);
}

export function logicalAckDownlinkId(): string {
  return LOGICAL_ACK_DOWNLINK_ID;
}

export function logicalAckUplinkId(): string {
  return LOGICAL_ACK_UPLINK_ID;
}

export function logicalAckElementIdForSender(isAircraftSender: boolean): string {
  return isAircraftSender ? logicalAckDownlinkId() : logicalAckUplinkId();
}

export function closesDialogueResponseElements(elements: MessageElement[]): boolean {
  const hasStandby = elements.some((e) => e.id === "DM2" || e.id === "UM1" || e.id === "UM2");
  const hasClosing = elements.some((e) =>
    ["DM0", "DM1", "DM3", "DM4", "DM5", "UM0", "UM3", "UM4", "UM5"].includes(e.id)
  );
  return hasClosing && !hasStandby;
}

export function responseAttrToIntents(attr: string): ResponseIntent[] {
  switch (attr) {
    case "WU":
      return [
        { label: "WILCO", downlinkId: "DM0" },
        { label: "UNABLE", downlinkId: "DM1" },
        { label: "STANDBY", downlinkId: "DM2" },
      ];
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
      return [];
    default:
      return [];
  }
}

export function chooseShortResponseIntents(
  elements: MessageElement[],
  resolveEntry: (id: string) => CatalogEntryForRuntime | undefined
): ResponseIntent[] {
  let best: CatalogEntryForRuntime | undefined;
  let bestPrio = 0;

  for (const e of elements) {
    const entry = resolveEntry(e.id);
    if (!entry) continue;
    const prio = RESPONSE_ATTR_PRIORITY[entry.response_attr] ?? 0;
    if (prio > bestPrio) {
      bestPrio = prio;
      best = entry;
    }
  }

  if (!best) {
    return responseAttrToIntents("WU");
  }

  if (best.short_response_intents.length > 0) {
    return best.short_response_intents.map((sri) => ({
      label: sri.label,
      downlinkId: sri.downlink_id,
    }));
  }

  return responseAttrToIntents(best.response_attr);
}

// Rust-parity aliases
export const is_logical_ack_element_id = isLogicalAckElementId;
export const message_contains_logical_ack = messageContainsLogicalAck;
export const should_auto_send_logical_ack = shouldAutoSendLogicalAck;
export const closes_dialogue_response_elements = closesDialogueResponseElements;
export const response_attr_to_intents = responseAttrToIntents;
export const choose_short_response_intents = chooseShortResponseIntents;
