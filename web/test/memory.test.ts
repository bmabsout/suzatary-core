/** The memory page's pure half: grouping by kind, and where a fact was said. */

import { test } from "node:test"
import assert from "node:assert/strict"

import type { FactRow, MemoryPayload } from "../src/wire/generated/wire"
import memory from "../src/chat/memory"
import { parseRoute } from "../src/shell/route"
import { forAll, type Rng } from "./gen"

const fact = (r: Rng, id: number): FactRow => ({
  id,
  at: "2026-09-25T09:00:00",
  fact: r.string("abc ", 8),
  kind: r.pick(["owner", "person", "preference", "standing"]),
  said: r.bool()
    ? { kind: "turn", conversation: "default", turn: r.int(40) }
    : { kind: "tapped", conversation: "2026-09-25-1", proposal: r.int(5) },
})

const payload = (r: Rng): MemoryPayload => ({ facts: r.array(r.int(12), () => fact(r, r.int(1000))), cap: 200, forgotten: [], cards: [] })

test("grouping keeps every fact once, in its kind, in the kinds' order and the facts' own", () =>
  forAll(200, (r) => {
    const p = payload(r)
    const groups = memory.group(p)
    assert.deepEqual(
      groups.flatMap((g) => g.facts),
      memory.KINDS.flatMap((k) => p.facts.filter((f) => f.kind === k.kind)),
    )
    for (const g of groups) {
      assert.ok(g.facts.length > 0, "an empty kind is left out")
      assert.ok(g.facts.every((f) => f.kind === g.kind))
    }
  }))

test("a fact links to the round that said it, and a tapped one to its card", () =>
  forAll(100, (r) => {
    const f = fact(r, 1)
    const route = parseRoute(memory.saidAt(f.said))
    assert.equal(route.type, "conversation")
    if (route.type !== "conversation") return
    assert.equal(route.id, f.said.conversation)
    assert.deepEqual(route.refs, f.said.kind === "tapped" ? [`proposal:${f.said.conversation}/${f.said.proposal}`] : [])
    assert.equal(route.at, f.said.kind === "turn" ? f.said.turn : undefined)
  }))

test("the memory page is its own route", () => assert.deepEqual(parseRoute("#/secretary/memory"), { type: "memory" }))
