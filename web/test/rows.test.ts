// The chain page's window: every row the reader can see is drawn, a bounded number besides, and the space left out
// above and below is exactly the rows left out.

import { test } from "node:test"
import assert from "node:assert/strict"

import rows from "../src/view/rows"
import { forAll } from "./gen"

test("law: every row in sight is drawn, the drawn rows are bounded, and the whole table's height stands", () =>
  forAll(1000, r => {
    const count = r.int(20000)
    const px = 1 + r.int(60)
    const view = r.int(2000)
    const top = r.int(count * px + 500) - 100
    const margin = r.int(40)
    const w = rows.of(count, px, top, view, margin)
    assert.ok(0 <= w.from && w.from <= w.to && w.to <= count, JSON.stringify(w))
    assert.equal(w.before + (w.to - w.from) * px + w.after, count * px, "the scrollbar is the whole table's")
    for (let i = 0; i < count; i++) {
      const seen = i * px < Math.max(0, top) + view && (i + 1) * px > Math.max(0, top)
      if (seen) assert.ok(w.from <= i && i < w.to, `row ${i} is in sight and not drawn: ${JSON.stringify({ w, top, view, px })}`)
      if (i * px > top + view + (margin + 1) * px) break
    }
    assert.ok(w.to - w.from <= Math.ceil(view / px) + 2 * margin + 1, `at most a screen and the margins: ${JSON.stringify(w)}`)
  }))

test("a row scrolled to the middle is in sight, and a table shorter than its box is drawn whole", () => {
  forAll(300, r => {
    const count = 1 + r.int(5000)
    const px = 1 + r.int(40)
    const view = 1 + r.int(900)
    const i = r.int(count)
    const top = rows.centred(i, px, view)
    const w = rows.of(count, px, top, view, 0)
    assert.ok(w.from <= i && i < w.to, JSON.stringify({ i, w }))
  })
  assert.deepEqual(rows.of(10, 20, 0, 1000, 5), { from: 0, to: 10, before: 0, after: 0 })
  assert.deepEqual(rows.of(0, 20, 0, 1000, 5), { from: 0, to: 0, before: 0, after: 0 })
})

// The rows drawn after the moves, applied as the chain page applies them.
const applied = (kept: readonly number[], m: rows.Moves<number>): number[] => {
  const out = kept.filter(i => !m.gone.includes(i))
  for (const { row, before } of m.added) {
    const at = before === null ? out.length : out.indexOf(before)
    out.splice(at, 0, row)
  }
  return out
}

test("law: the moves draw exactly the rows wanted, in order, and never take out a row in both windows", () =>
  forAll(1000, r => {
    const count = r.int(200)
    const px = 1 + r.int(30)
    const view = r.int(600)
    const was = rows.of(count, px, r.int(count * px + 1), view, r.int(10))
    const now = rows.of(count, px, r.int(count * px + 1), view, r.int(10))
    const range = (w: rows.Window) => Array.from({ length: w.to - w.from }, (_, k) => w.from + k)
    const kept = range(was)
    const m = rows.moves(kept, range(now), i => i)
    assert.deepEqual(applied(kept, m), range(now))
    for (const i of m.gone) assert.ok(!range(now).includes(i), `row ${i} is in both and was taken out`)
    assert.equal(m.added.length + kept.length - m.gone.length, now.to - now.from)
  }))
