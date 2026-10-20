// A composer's box over its draft: a reading of one value is seen, so two boxes on one draft never conflict with
// themselves, and words written elsewhere unseen stand beside the box's as a conflict.

import { test } from "node:test"
import assert from "node:assert/strict"

import draftbox from "../src/chat/draftbox"
import drafts from "../src/chat/drafts"
import ui from "../src/app/ui"
import kv from "../src/app/kv"
import { forAll, memoryStore } from "./gen"

const two = () => {
  const m = memoryStore()
  return [drafts.of(ui.over(kv.over(() => m), "wide"), "c"), drafts.of(ui.over(kv.over(() => m), "panel"), "c")] as const
}

test("two boxes writing the same words, each hearing the other, then a keystroke: no conflict", () =>
  forAll(100, r => {
    const [a, b] = two()
    let x = draftbox.open(a)
    let y = draftbox.open(b)
    const words = `hello ${r.int(99)}`
    x = draftbox.typed(a, x, words)
    y = draftbox.typed(b, y, words)
    x = draftbox.heard(a, x)
    y = draftbox.heard(b, y)
    assert.equal(a.read().type, "Value", "two equal writes read as one value")
    y = draftbox.typed(b, y, `${words}!`)
    assert.deepEqual(a.read(), { type: "Value", v: `${words}!` })
    assert.equal(draftbox.heard(a, x).text, `${words}!`)
  }))

test("words typed in a box that has not heard the other's stand beside them; settling sees both", () => {
  const [a, b] = two()
  let x = draftbox.open(a)
  let y = draftbox.open(b)
  x = draftbox.typed(a, x, "from the phone")
  y = draftbox.typed(b, y, "from the desk")
  assert.equal(a.read().type, "Conflict")
  assert.equal(draftbox.heard(b, y), y, "a conflict leaves the box as it was")
  y = draftbox.settled(b, y.text)
  assert.deepEqual(a.read(), { type: "Value", v: "from the desk" })
  assert.equal(draftbox.heard(a, x).text, "from the desk")
})

test("a box of spaces keeps its spaces when its blank draft reads back", () => {
  const [a] = two()
  const x = draftbox.typed(a, draftbox.open(a), "   ")
  assert.equal(draftbox.heard(a, x).text, "   ")
})

test("a box hearing its draft emptied (sent, here or elsewhere) empties, whatever it last held", () => {
  const [a, b] = two()
  let x = draftbox.typed(a, draftbox.open(a), "sent already")
  const y = draftbox.heard(b, draftbox.open(b))
  draftbox.typed(b, y, "")
  assert.equal(a.read().type, "Unwritten")
  x = draftbox.heard(a, x)
  assert.equal(x.text, "")
})
