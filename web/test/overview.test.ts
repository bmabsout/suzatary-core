// The overview's answers as the box writes them: they decode, hold every group once in order and name no secret, and
// every standing in the box's vector is drawn by its standing alone, each failure's class named.

import { test } from "node:test"
import assert from "node:assert/strict"
import { readFileSync } from "node:fs"

import payload from "../src/wire/generated/payload"
import overview from "../src/pages/overview"
import { forAll, GROUPS, overviewLine } from "./gen"

const at = (s: string) => `[${s}]`

test("the box's own overview decodes, holds every group once in order, and names no secret", () => {
  const samples: unknown = JSON.parse(readFileSync("app/core/tests/web/answers.json", "utf8"))
  const sampled = typeof samples === "object" && samples !== null && "Overview" in samples && Array.isArray(samples.Overview) ? samples.Overview : []
  assert.ok(sampled.length > 0, "the box writes an overview")
  for (const sample of sampled) {
    const read = payload.Overview(sample.value)
    if (read.type !== "Val") throw new Error(`an Overview sample does not decode: ${JSON.stringify(read)}`)
    assert.deepEqual(read.v.groups.map(g => g.group), [...GROUPS])
    assert.equal(/PLUGIN_|_API_KEY|_TOKEN/.test(JSON.stringify(read.v)), false)
  }
})

test("every standing the box writes is drawn by its standing alone, each failure's class named", () => {
  const read = payload.Overview(JSON.parse(readFileSync("app/core/tests/web/standings.json", "utf8")))
  if (read.type !== "Val") throw new Error(`the standings vector does not decode: ${JSON.stringify(read)}`)
  const lines = read.v.groups.flatMap(g => g.lines)
  const kinds = new Set(lines.map(l => l.standing.kind))
  assert.ok(kinds.size >= 10, `the vector holds every arm: ${[...kinds].join(", ")}`)
  forAll(50, r => {
    for (const l of lines) {
      const [mine, theirs] = [overview.drawn({ ...overviewLine(r), standing: l.standing }, at), overview.drawn(l, at)]
      assert.deepEqual([mine.health, mine.atTheBox], [theirs.health, theirs.atTheBox])
      if (l.standing.kind === "failing") assert.ok(overview.health(l.standing, at)?.text.endsWith(`.${overview.cause(l.standing.class)}`))
    }
  })
  const failing = lines.flatMap(l => (l.standing.kind === "failing" ? [{ kind: l.standing.class.kind, text: overview.health(l.standing, at)?.text }] : []))
  assert.equal(new Set(failing.map(f => f.text)).size, new Set(failing.map(f => f.kind)).size, "each kind of class's line says something of its own")
})
