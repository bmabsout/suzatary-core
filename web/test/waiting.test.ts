// The laws of `waiting`: its order is total, open and settled are disjoint and cover what is resolved or open.

import { test } from "node:test"
import assert from "node:assert/strict"

import type { Block, Entry } from "../src/wire/generated/wire"
import type { InboxProposal } from "../src/wire/generated/wire"
import time from "../src/app/time"
import waiting from "../src/inbox/waiting"
import { authoredAs, forAll, type Rng } from "./gen"

const proposal = (r: Rng): InboxProposal => ({
  id: `p${r.string("0123456789abcdef", 6)}`,
  asked: `proposal:${r.string("abc", 3)}/${r.int(4)}`,
  serves: r.pick([null, "todo:life/reply", "todo:suzatary/gate"]),
  headline: r.string("abc ", 6),
  by: { kind: "secretary" },
  why: r.string("abc ", 6),
  state: r.pick<InboxProposal["state"]>([
    { kind: "open" },
    { kind: "completed", at: r.pick(["2026-09-28T10:00:00", "2026-09-27T10:00:00"]) },
    { kind: "cancelled", at: r.pick(["2026-09-28T10:00:00", "2026-09-27T10:00:00"]) },
    { kind: "unreadable", reason: "it checks a send that nothing says may have gone" },
  ]),
  value: r.bool() ? r.int(100) / 100 : null,
  at: "2026-09-28T09:00:00",
})

const entry = (r: Rng): Entry => ({
  todo: r.string("abcdef", 6),
  state: r.pick(["open", "completed", "cancelled"]),
  states: [],
  at: r.pick(["", "2026-09-28 10:00:00", "2026-09-27 10:00:00"]),
  claimed: "",
  unconfirmed: false,
  recurring: false,
  value: r.bool() ? r.int(100) / 100 : null,
  conflicts: {},
  authored: authoredAs(r.pick<Block>([{ kind: "decision" }, { kind: "todo", name: "todo" }])),
  candidates: [],
  stream: [],
})

const some = (r: Rng): waiting.Waiting[] =>
  waiting.all(
    r.array(r.int(5), () => proposal(r)),
    [{ chain: "life", entries: r.array(r.int(5), () => entry(r)) }],
    r.pick([null, ["life"], ["life", "suzatary"]]),
  )

test("law: compare is a total order — antisymmetric and transitive", () =>
  forAll(200, (r, seed) => {
    const [a, b, c] = [some(r), some(r), some(r)].map((ws) => ws[0]).filter((w) => w !== undefined)
    if (a === undefined || b === undefined) return
    assert.equal(Math.sign(waiting.compare(a, b)), -Math.sign(waiting.compare(b, a)), `seed ${seed}`)
    if (c !== undefined && waiting.compare(a, b) <= 0 && waiting.compare(b, c) <= 0)
      assert.ok(waiting.compare(a, c) <= 0, `seed ${seed}`)
  }))

test("law: open is sorted by compare, and nothing open is settled", () =>
  forAll(200, (r, seed) => {
    const ws = some(r)
    const open = waiting.open(ws)
    open.forEach((w, i) => {
      const next = open[i + 1]
      if (next !== undefined) assert.ok(waiting.compare(w, next) <= 0, `seed ${seed}`)
    })
    const settled = new Set(waiting.settledOn(ws, "2026-09-28", time.UTC).map(waiting.key))
    for (const w of open) assert.ok(!settled.has(waiting.key(w)), `seed ${seed}`)
  }))

test("law: an unreadable proposal is neither open nor settled, and says why", () =>
  forAll(200, (r, seed) => {
    const ws = some(r)
    const out = new Set([...waiting.open(ws), ...waiting.settledOn(ws, "2026-09-28", time.UTC)].map(waiting.key))
    for (const w of ws.filter(w => w.type === "Proposal" && w.p.state.kind === "unreadable")) {
      assert.ok(!out.has(waiting.key(w)), `seed ${seed}`)
      assert.ok((waiting.unreadable(w) ?? "") !== "", `seed ${seed}`)
    }
  }))

test("law: a todo is never waiting on him", () =>
  forAll(100, (r, seed) => {
    const entries = r.array(4, () => ({ ...entry(r), authored: authoredAs({ kind: "todo", name: "todo" }) }))
    assert.deepEqual(waiting.all([], [{ chain: "life", entries }]), [], `seed ${seed}`)
  }))

test("law: a proposal on a chain the reading did not hold is not yet priced, and comes after every priced one and ∅", () =>
  forAll(200, (r, seed) => {
    const ps = r.array(1 + r.int(5), () => proposal(r))
    const ws = waiting.open(waiting.all(ps, [], ["life"]))
    for (const w of ws) {
      const p = w.type === "Proposal" ? w.p : null
      assert.equal(waiting.notYetPriced(w), p?.serves === "todo:suzatary/gate" ? "suzatary" : null, `seed ${seed}`)
    }
    const firstUnheld = ws.findIndex((w) => waiting.notYetPriced(w) !== null)
    if (firstUnheld >= 0) assert.ok(ws.slice(firstUnheld).every((w) => waiting.notYetPriced(w) !== null), `seed ${seed}`)
    // The box holds every chain, so its answer prices them all.
    assert.ok(waiting.all(ps, []).every((w) => waiting.notYetPriced(w) === null), `seed ${seed}`)
  }))
