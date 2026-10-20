// A head set's laws: `of` forgets order and repetition, and `same` is equality of sets so built.

import { test } from "node:test"
import assert from "node:assert/strict"

import heads from "../src/prodrome/heads"
import { SLUG, forAll } from "./gen"

test("law: of is idempotent and order-free, and same is its equality", () => {
  forAll(200, r => {
    const names = r.array(r.int(6), () => r.string(SLUG, 3))
    const h = heads.of(names)
    assert.deepEqual(heads.of(h), h)
    assert.ok(heads.same(h, heads.of([...names].reverse().concat(names))))
    assert.equal(heads.same(h, heads.of([...names, "zzzz"])), names.includes("zzzz"))
  })
})

test("a set is shown whole, none picked", () => {
  assert.equal(heads.shown(heads.NONE), "nothing")
  assert.equal(heads.shown(["b".repeat(64), "a".repeat(64)]), `${"b".repeat(12)}…, ${"a".repeat(12)}…`)
  assert.equal(heads.shown(["a", "b", "c", "d", "e"].map(c => c.repeat(64))), `${"a".repeat(12)}…, ${"b".repeat(12)}…, ${"c".repeat(12)}…, 2 more`)
})
