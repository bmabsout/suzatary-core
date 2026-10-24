// A diff's one line: the monoid it is folded with, and that the line names every act and todo of its rows.

import { test } from "node:test"
import assert from "node:assert/strict"

import type { ProposedChange, ProposedObject } from "../src/wire/generated/wire"
import digest from "../src/inbox/digest"
import type proposed from "../src/inbox/proposed"
import { forAll, type Rng } from "./gen"

type More = { readonly changes?: ProposedChange[]; readonly pending_todo?: boolean }
// One row of each act, about `todo`.
const row = (act: proposed.Act, todo: string, more: More = {}): ProposedObject => {
  const position = 1
  const t = { todo, body: "", at: "", actor: "triage", pending_todo: more.pending_todo ?? false }
  const changes = more.changes ?? []
  switch (act) {
    case "added": return { position, act, ...t, changes }
    case "edited": return { position, act, ...t, changes }
    case "repriced": return { position, act, ...t, changes }
    case "created": return { position, act, ...t, note: "" }
    case "completed": return { position, act, ...t, note: "" }
    case "cancelled": return { position, act, ...t, note: "" }
    case "reopened": return { position, act, ...t, note: "" }
    case "tended": return { position, act, ...t, note: "" }
    case "respecified": return { position, act, ...t, note: "" }
  }
}

const ACTS: readonly proposed.Act[] = ["added", "edited", "repriced", "completed", "cancelled", "reopened", "tended", "created", "respecified"]
const anyRow = (r: Rng): ProposedObject => {
  const act = r.pick(ACTS)
  const todo = r.pick(["a", "b", "c"])
  return row(act, todo, {
    changes: r.bool() ? [{ field: "priority", before: "", after: r.pick(["10%", "45%", "70%", "unpriced"]) }] : [],
    pending_todo: r.bool(0.2),
  })
}
const rows = (r: Rng): ProposedObject[] => Array.from({ length: r.int(6) }, () => anyRow(r))

test("law: `of` is a monoid homomorphism — empty is the identity and a split list folds alike", () => {
  forAll(300, (r, seed) => {
    const [xs, ys, zs] = [rows(r), rows(r), rows(r)]
    const { combine, empty, of } = digest
    assert.deepEqual(combine(empty, of(xs)), of(xs), `left identity, seed ${seed}`)
    assert.deepEqual(combine(of(xs), empty), of(xs), `right identity, seed ${seed}`)
    assert.deepEqual(of([...xs, ...ys]), combine(of(xs), of(ys)), `homomorphism, seed ${seed}`)
    assert.deepEqual(combine(combine(of(xs), of(ys)), of(zs)), combine(of(xs), combine(of(ys), of(zs))), `associative, seed ${seed}`)
  })
})

test("law: every act and every named todo of the rows is in the line", () => {
  forAll(300, (r, seed) => {
    const xs = rows(r)
    const t = digest.of(xs)
    assert.deepEqual(t.groups.map(g => g.act), [...new Set(xs.map(o => o.act))], `seed ${seed}`)
    assert.equal(t.groups.reduce((k, g) => k + g.objects, 0), xs.length, `every object is counted, seed ${seed}`)
    const line = digest.line(t)
    for (const g of t.groups) {
      const said = digest.phrase(g.act, g.todos.length === 1 ? (g.todos[0] ?? "") : `${g.todos.length} todos`)
      assert.ok(line.includes(said), `${said} in ${line}, seed ${seed}`)
    }
  })
})
