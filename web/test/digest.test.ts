// A diff's one line: the cases the owner reads, and the monoid it is folded with.

import { test } from "node:test"
import assert from "node:assert/strict"

import type { ProposedObject } from "../src/wire/generated/wire"
import digest from "../src/inbox/digest"
import { forAll, type Rng } from "./gen"

const row = (act: string, todo: string, more: Partial<ProposedObject> = {}): ProposedObject => ({
  hash: `${act}-${todo}`,
  act,
  todo,
  body: "",
  at: "",
  actor: "triage",
  note: "",
  changes: [],
  needs: [],
  pending_todo: false,
  ...more,
})
const priced = (act: string, todo: string, after: string): ProposedObject =>
  row(act, todo, { changes: [{ field: "priority", before: "", after }] })
const line = (rows: ProposedObject[]): string => digest.line(digest.of(rows))

test("one act names its todo in the row's own words", () => {
  assert.equal(line([row("edited", "file-permit")]), "reword file-permit")
  assert.equal(line([row("tended", "plants")]), "record a pass at plants")
  assert.equal(line([row("completed", "renew-card")]), digest.sentence(row("completed", "renew-card")))
})

test("several of one act are counted, one todo acted on twice is named once", () => {
  assert.equal(line([row("repriced", "a"), row("repriced", "b")]), "reprice 2 todos")
  assert.equal(line([row("edited", "a"), row("edited", "a")]), "reword a")
})

test("mixed acts are grouped by act, in the order they first appear", () => {
  assert.equal(
    line([row("edited", "file-permit"), row("repriced", "a"), row("completed", "renew-card"), row("repriced", "b")]),
    "reword file-permit · reprice 2 todos · complete renew-card",
  )
})

test("a merge names no todo and is counted by its objects", () => {
  assert.equal(line([row("merge", "")]), "join two histories")
  assert.equal(line([row("added", "x"), row("merge", ""), row("merge", "")]), "add x · join histories 2 times")
})

test("an object with no todo is about the chain", () => {
  assert.equal(line([row("created", "")]), "register the chain")
  assert.equal(line([]), "")
})

test("a price at or under the hold is said, a higher one or none is not", () => {
  const t = digest.of([priced("added", "renew-card", "40%"), priced("repriced", "plants", "80%; every 60 days"), row("completed", "x")])
  assert.equal(digest.urgentLine(t), "At or under the 45% the review holds: renew-card at 40%")
  assert.equal(digest.urgentLine(digest.of([priced("added", "a", "45%"), priced("added", "b", "unpriced")])), "At or under the 45% the review holds: a at 45%")
  assert.equal(digest.urgentLine(digest.of([priced("added", "a", "46%")])), "")
})

test("a todo waiting on another batch is said", () => {
  const t = digest.of([row("completed", "x", { pending_todo: true }), row("merge", "", { pending_todo: true })])
  assert.equal(digest.pendingLine(t), "Not on the chain yet, waiting on another batch: x")
})

test("the box's printed price reads as its number, and anything else as none", () => {
  assert.equal(digest.percent("40%"), 40)
  assert.equal(digest.percent("100%; every 60 days after a pass"), 100)
  assert.equal(digest.percent("unpriced"), null)
  assert.equal(digest.percent(""), null)
})

const ACTS = ["added", "edited", "repriced", "completed", "cancelled", "reopened", "tended", "created", "merge"]
const anyRow = (r: Rng): ProposedObject => {
  const act = r.pick(ACTS)
  const todo = act === "merge" ? "" : r.pick(["", "a", "b", "c"])
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
      const said = g.act === "merge" ? digest.phrase("merge", "") : digest.phrase(g.act, g.todos.length === 1 ? (g.todos[0] ?? "") : `${g.todos.length} todos`)
      assert.ok(g.act === "merge" && g.objects > 1 ? line.includes(`join histories ${g.objects} times`) : line.includes(said), `${said} in ${line}, seed ${seed}`)
    }
  })
})
