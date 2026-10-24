// A node as a line and a group as a sheet: fold-by-need, the peek, the counts, and what l and h do.

import { test } from "node:test"
import assert from "node:assert/strict"

import grouping from "../src/todo/grouping"
import row from "../src/todo/row"
import tree from "../src/algebra/tree"
import { forAll } from "./gen"
import type { Rng } from "./gen"

const { worth, place, segment } = grouping

const node = (p: grouping.Place, v: number | null): grouping.Node => ({
  place: p,
  term: { kind: "flat", value: v ?? 0 },
  worth: v === null ? worth.unlinked("no value") : worth.value(v),
})
const todo = (id: string, v: number | null, kids: tree.Tree<grouping.Node>[] = []) => tree.of(node(place.at(segment.todo(id)), v), kids)
const group = (name: string, v: number, kids: tree.Tree<grouping.Node>[]) => tree.of(node(place.at(segment.category(name)), v), kids)

// home (Watch): book (0.55) with a subtodo pass (0.3); water (0.88). work (Fine): file (0.8).
const ROOT = tree.of(node(place.all, 0.6), [
  group("home", 0.6, [todo("book", 0.55, [todo("pass", 0.3)]), todo("water", 0.88)]),
  group("work", 0.8, [todo("file", 0.8)]),
])

const genWorth = (r: Rng): grouping.Worth => (r.bool(0.1) ? worth.unlinked("x") : worth.value(r.int(101) / 100))

test("law: a fold nobody touched is open exactly when its value is not Fine; a touched one is the viewer's", () => {
  forAll(300, r => {
    const w = genWorth(r)
    assert.equal(row.isOpen({}, "k", w), worth.state(w).type !== "Fine")
    const set = r.bool()
    assert.equal(row.isOpen({ k: set }, "k", w), set)
  })
})

test("cards: a Watch group opens with its rows, a Fine one folds and keeps a peek of its most urgent todo", () => {
  const [home, work] = row.cards(ROOT, "category", {})
  assert.equal(home?.open, true)
  assert.deepEqual(home?.lines.map(l => l.key), ["c:home/t:book", "c:home/t:book/t:pass", "c:home/t:water"])
  assert.equal(home?.count, 3)
  assert.equal(home?.peek === null || home?.peek === undefined ? null : place.todo(home.peek.place), "pass")
  assert.equal(work?.open, false)
  assert.deepEqual(work?.lines, [])
  assert.deepEqual(work?.peek?.worth, worth.value(0.8))
  assert.equal(row.attention(ROOT), 1, "pass is the one Problem")
})

test("the viewer's folds win: a folded parent hides its subtodos, an unfolded Fine group shows its rows", () => {
  const [home, work] = row.cards(ROOT, "category", { "t:book": false, "c:work": true })
  assert.deepEqual(home?.lines.map(l => l.key), ["c:home/t:book", "c:home/t:water"])
  assert.deepEqual(work?.lines.map(l => l.key), ["c:work/t:file"])
})

const stopAt = (key: string, folds: row.Folds = {}): row.Stop => {
  const s = row.stops(row.cards(ROOT, "category", folds)).find(p => p.key === key)
  if (s === undefined) throw new Error(`no stop ${key}`)
  return s
}

test("l unfolds, then opens in place (a pane when wide), then goes to the todo", () => {
  assert.deepEqual(row.onOpen(stopAt("c:work"), {}, "", false), { type: "Fold", key: "c:work", open: true })
  assert.deepEqual(row.onOpen(stopAt("c:home"), {}, "", false), { type: "Stay" })
  const folded = { "t:book": false }
  assert.deepEqual(row.onOpen(stopAt("c:home/t:book", folded), folded, "", false), { type: "Fold", key: "t:book", open: true })
  assert.deepEqual(row.onOpen(stopAt("c:home/t:water"), {}, "", false), { type: "Open", todo: "water" })
  assert.deepEqual(row.onOpen(stopAt("c:home/t:water"), {}, "water", false), { type: "Go", todo: "water" })
  assert.deepEqual(row.onOpen(stopAt("c:home/t:water"), {}, "", true), { type: "Go", todo: "water" })
})

test("h closes what is open in place, then folds, then climbs to the parent", () => {
  assert.deepEqual(row.onClose(stopAt("c:home/t:water"), {}, "water"), { type: "Close" })
  assert.deepEqual(row.onClose(stopAt("c:home/t:book"), {}, ""), { type: "Fold", key: "t:book", open: false })
  assert.deepEqual(row.onClose(stopAt("c:home/t:book/t:pass"), {}, ""), { type: "Cursor", key: "c:home/t:book" })
  assert.deepEqual(row.onClose(stopAt("c:home/t:water"), {}, ""), { type: "Cursor", key: "c:home" })
  assert.deepEqual(row.onClose(stopAt("c:home"), {}, ""), { type: "Fold", key: "c:home", open: false })
})

test("law: the stops are each card's head then its visible lines, every key once", () => {
  forAll(100, r => {
    const folds = { "c:home": r.bool(), "c:work": r.bool(), "t:book": r.bool() }
    const cs = row.cards(ROOT, "category", folds)
    const ks = row.stops(cs).map(s => s.key)
    assert.equal(new Set(ks).size, ks.length)
    assert.deepEqual(ks, cs.flatMap(c => [c.key, ...c.lines.map(l => l.key)]))
  })
})
