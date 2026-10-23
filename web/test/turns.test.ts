// A turn row is tagged by who said it: each speaker's row decodes with its own parts and no other's.

import { test } from "node:test"
import assert from "node:assert/strict"

import payloads from "../src/wire/generated/payload"
import turns from "../src/chat/turns"
import type { TurnRow } from "../src/wire/generated/wire"

const shared = { index: 3, text: "t", at: "2026-10-20T09:00:00", attempt: "current", gone: null } as const
const rows: readonly TurnRow[] = [
  { ...shared, who: "person", references: ["todo:life/x"], attachments: [], queued: true },
  { ...shared, who: "model", blocks: [] },
  { ...shared, who: "verb", verb: "remember", outcome: { is: "remembered", receipt: { state: "forgotten", fact: 2 } } },
  { ...shared, who: "act", references: ["todo:life/y"] },
  { ...shared, who: "box", references: [] },
]

test("every speaker's row decodes as itself", () => {
  for (const row of rows) assert.deepEqual(payloads.TurnRow(row), { type: "Val", v: row })
})

test("a row is read as its speaker's: another speaker's part is not read, and a missing one of its own does not decode", () => {
  const person = { ...shared, who: "person", references: [], attachments: [], queued: false } as const
  assert.deepEqual(payloads.TurnRow({ ...person, verb: "x", blocks: [] }), { type: "Val", v: person })
  assert.equal(payloads.TurnRow({ ...shared, who: "verb", verb: "remember" }).type, "Error", "a verb's row says how its call ended")
  assert.equal(payloads.TurnRow({ ...shared, who: "verb", verb: "remember", outcome: { is: "remembered" } }).type, "Error", "a remembered outcome carries its receipt")
  assert.equal(payloads.TurnRow({ ...shared, who: "someone", references: [] }).type, "Error")
})

test("each part is read off its own speaker's row, and is empty for every other", () => {
  assert.deepEqual(rows.map(turns.references), [["todo:life/x"], [], [], ["todo:life/y"], []])
  assert.deepEqual(rows.map(turns.queued), [true, false, false, false, false])
  assert.deepEqual(rows.map(turns.verb), [null, null, "remember", null, null])
  assert.deepEqual(rows.map(turns.remembered), [null, null, { state: "forgotten", fact: 2 }, null, null])
  assert.deepEqual(rows.filter(turns.isQuestion).map(r => r.who), ["person"])
})
