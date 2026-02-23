import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  isLogicalAckElementId,
  messageContainsLogicalAck,
  shouldAutoSendLogicalAck,
  responseAttrToIntents,
  chooseShortResponseIntents,
  closesDialogueResponseElements,
} from "../src/cpdlc-runtime";
import type { MessageElement } from "../src/types";

interface BoolCase {
  id: string;
  operation: string;
  input: Record<string, unknown>;
  expected: boolean;
}

interface DownlinkCase {
  id: string;
  operation: string;
  input: { attr: string };
  expected_downlink_ids: string[];
}

interface ShortSelectionCase {
  id: string;
  operation: string;
  input: {
    elements: MessageElement[];
    catalog_entries: Record<
      string,
      {
        id: string;
        response_attr: string;
        short_response_intents: Array<{
          intent: string;
          label: string;
          uplink_id: string;
          downlink_id: string;
        }>;
      }
    >;
  };
  expected_downlink_ids: string[];
}

interface RuntimeVectors {
  runtime: {
    logical_ack: BoolCase[];
    response_attr: DownlinkCase[];
    short_response_selection: ShortSelectionCase[];
    dialogue_close: BoolCase[];
  };
}

function loadRuntimeVectors(): RuntimeVectors {
  const here = path.dirname(fileURLToPath(import.meta.url));
  const vectorsPath = path.resolve(here, "../../../spec/sdk-conformance/runtime-vectors.v1.json");
  return JSON.parse(fs.readFileSync(vectorsPath, "utf8")) as RuntimeVectors;
}

const vectors = loadRuntimeVectors();

test("runtime vectors: logical_ack", () => {
  for (const c of vectors.runtime.logical_ack) {
    let got: boolean;

    if (c.operation === "is_logical_ack_element_id") {
      got = isLogicalAckElementId(String(c.input.id));
    } else if (c.operation === "message_contains_logical_ack") {
      got = messageContainsLogicalAck(c.input.elements as MessageElement[]);
    } else if (c.operation === "should_auto_send_logical_ack") {
      got = shouldAutoSendLogicalAck(c.input.elements as MessageElement[], Number(c.input.min));
    } else {
      throw new Error(`Unsupported operation for ${c.id}: ${c.operation}`);
    }

    assert.equal(got, c.expected, `Vector failed: ${c.id}`);
  }
});

test("runtime vectors: response_attr", () => {
  for (const c of vectors.runtime.response_attr) {
    const got = responseAttrToIntents(c.input.attr).map((i) => i.downlinkId);
    assert.deepEqual(got, c.expected_downlink_ids, `Vector failed: ${c.id}`);
  }
});

test("runtime vectors: short_response_selection", () => {
  for (const c of vectors.runtime.short_response_selection) {
    const got = chooseShortResponseIntents(c.input.elements, (id) => c.input.catalog_entries[id]).map(
      (i) => i.downlinkId
    );
    assert.deepEqual(got, c.expected_downlink_ids, `Vector failed: ${c.id}`);
  }
});

test("runtime vectors: dialogue_close", () => {
  for (const c of vectors.runtime.dialogue_close) {
    const got = closesDialogueResponseElements(c.input.elements as MessageElement[]);
    assert.equal(got, c.expected, `Vector failed: ${c.id}`);
  }
});
