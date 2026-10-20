// A card's own state: a register outside the drawing, stepped by its events, so a redraw reads what was open and typed.

import { test } from "node:test"
import assert from "node:assert/strict"

import cardui from "../src/inbox/cardui"
import ui from "../src/app/ui"
import kv from "../src/app/kv"
import { forAll, memoryStore, type Rng } from "./gen"

const stepOf = (r: Rng): cardui.Step =>
  r.pick<cardui.Step>([{ type: "Idle" }, { type: "Confirming", verb: r.pick(["send", "always"]) }, { type: "Saying", verb: r.pick(["dismiss", "drop"]) }])
const details = (r: Rng): cardui.Details => r.pick<cardui.Details>(["Hidden", "Shown"])
const event = (r: Rng): cardui.Event =>
  r.pick<cardui.Event>([
    { type: "Opened", step: stepOf(r) },
    { type: "Typed", text: r.string("ab ", r.int(6)) },
    { type: "Closed" },
    { type: "Details", details: details(r) },
  ])

test("law: typing keeps the step and only the words move; opening starts afresh; closing is idle; none hides the details", () =>
  forAll(300, r => {
    const t = r.array(r.int(8), () => event(r)).reduce(cardui.step, cardui.IDLE)
    const text = r.string("xy", 1 + r.int(5))
    assert.deepEqual(cardui.step(t, { type: "Typed", text }), { ...t, text })
    const s = stepOf(r)
    assert.deepEqual(cardui.step(t, { type: "Opened", step: s }), { step: s, text: "", details: s.type === "Confirming" ? "Shown" : t.details })
    assert.deepEqual(cardui.step(t, { type: "Closed" }), { ...cardui.IDLE, details: t.details })
  }))

test("a card is drawn with its details hidden, and asking any yes shows them, so what is confirmed is on screen", () =>
  forAll(200, r => {
    assert.equal(cardui.IDLE.details, "Hidden")
    const t = r.array(r.int(8), () => event(r)).reduce(cardui.step, cardui.IDLE)
    const asked = cardui.step(t, { type: "Opened", step: { type: "Confirming", verb: r.pick(["send", "run", "store"]) } })
    assert.equal(asked.details, "Shown")
    const d = details(r)
    assert.deepEqual(cardui.step(t, { type: "Details", details: d }), { ...t, details: d })
  }))

test("law: a register reads back what was last written to its key, and nothing written to another", () =>
  forAll(200, r => {
    const kind = ui.kind<number>({ name: "n", tier: "Tab", initial: 0 })
    const store = memoryStore()
    const tab = ui.over(kv.over(() => store), "tab")
    const writes = r.array(r.int(10), () => [r.pick(["a", "b", "c"]), r.int(4)] as const)
    for (const [k, v] of writes) tab.cell(kind, k).set(v)
    for (const k of ["a", "b", "c", "d"]) {
      const last = writes.filter(([w]) => w === k).at(-1)
      assert.equal(tab.cell(kind, k).get(), last === undefined ? 0 : last[1])
    }
  }))

test("a card's register outlives the drawing that wrote it: a fresh handle on the same key reads the same", () =>
  forAll(100, r => {
    const k = cardui.key("proposal", `c${r.int(1000)}/${r.int(5)}`)
    const events = r.array(1 + r.int(6), () => event(r))
    for (const e of events) cardui.of(k).go(e)
    assert.deepEqual(cardui.of(k).get(), events.reduce(cardui.step, cardui.IDLE))
    cardui.of(k).go({ type: "Closed" })
    cardui.of(k).go({ type: "Details", details: "Hidden" })
  }))
