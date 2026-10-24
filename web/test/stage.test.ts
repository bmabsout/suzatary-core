import { test } from "node:test"
import assert from "node:assert/strict"

import type { ProposalRow, ProposalState } from "../src/wire/generated/wire"
import stage from "../src/inbox/stage"
import { aCard } from "./cards"

const states: readonly ProposalState[] = [
  "standing", "sent", "applied", "started", "acted", "failed", "unknown", "checked", "dismissed", "superseded",
  "withdrawn", "unreadable",
]

const row = (state: ProposalState, tries: number): ProposalRow => ({
  index: 0,
  act: { kind: "email", account: "a@b.test", to: [], cc: [], subject: "", body: "", attachments: [], held: { type: "Read", values: [] } },
  card: aCard(),
  why: "",
  sources: [],
  at: "",
  state,
  outcome: "auth: 535",
  tries,
  turn: 0,
  coding: null,
  announced: null,
})

test("law: whatever may be answered is open, a send that may have gone waits for his check, and settled states are neither", () => {
  for (const s of states) {
    assert.ok(!stage.answerable(s) || stage.open(s), s)
    const settled =
      s === "sent" || s === "applied" || s === "acted" || s === "checked" || s === "dismissed" || s === "superseded" ||
      s === "withdrawn" || s === "unreadable"
    assert.equal(stage.open(s), !settled, s)
  }
  assert.ok(stage.open("unknown") && !stage.answerable("unknown"), "open, and never sent or dismissed again")
})

test("an unreadable card says why and offers nothing to answer", () => {
  assert.equal(stage.said(row("unreadable", 0)), "Cannot be read: auth: 535")
  assert.ok(!stage.answerable("unreadable") && !stage.open("unreadable"))
})

test("a failed card says it sent nothing and how many times it was tried", () => {
  assert.equal(stage.said(row("failed", 1)), "Not sent, tried once: auth: 535")
  assert.equal(stage.said(row("failed", 3)), "Not sent, tried 3 times: auth: 535")
  assert.equal(stage.said(row("sent", 2)), "Sent to auth: 535 (tried 2 times)")
  assert.equal(stage.said(row("sent", 1)), "Sent to auth: 535")
})

test("a card that may have gone says what to check, and that he checked", () => {
  assert.equal(
    stage.said(row("unknown", 1)),
    "Failed, tried once: auth: 535. It may have been sent. Check Sent; it is never sent again.",
  )
  assert.equal(
    stage.said(row("checked", 1)),
    "Failed, tried once: auth: 535. You checked Sent; it is never sent again.",
  )
})

test("a withdrawn card says why it was taken back, and a replaced one never quotes its words", () => {
  assert.equal(stage.said(row("withdrawn", 0)), "Withdrawn: auth: 535")
  assert.equal(stage.said(row("superseded", 0)), "Replaced by a later draft.")
  assert.ok(!stage.answerable("withdrawn") && !stage.open("withdrawn"))
})
