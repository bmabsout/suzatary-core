import assert from "node:assert/strict"
import { test } from "node:test"

import data from "../src/algebra/data"
import type { Rng } from "./gen"
import { forAll } from "./gen"

test("data: equal data is the same, field by field and element by element", () => {
  assert.ok(data.same({ a: [1, "x", null, { b: true }] }, { a: [1, "x", null, { b: true }] }))
  assert.ok(data.same(NaN, NaN))
})

test("data: a field, an element or a length apart is not the same", () => {
  assert.ok(!data.same({ a: 1 }, { a: 2 }))
  assert.ok(!data.same({ a: 1 }, { a: 1, b: undefined }))
  assert.ok(!data.same([1, 2], [1, 2, 3]))
  assert.ok(!data.same([1], { 0: 1, length: 1 }))
  assert.ok(!data.same(null, {}))
})

test("data: sets are equal by their members and maps by their entries", () => {
  assert.ok(data.same(new Set(["a", "b"]), new Set(["b", "a"])))
  assert.ok(!data.same(new Set(["a"]), new Set(["a", "b"])))
  assert.ok(data.same(new Map([["k", { v: [1] }]]), new Map([["k", { v: [1] }]])))
  assert.ok(!data.same(new Map([["k", 1]]), new Map([["k", 2]])))
  assert.ok(!data.same(new Set(), []))
})

test("data: what is not data is itself alone", () => {
  const d = new Date(0)
  assert.ok(data.same(d, d))
  assert.ok(!data.same(new Date(0), new Date(0)))
})

const value = (r: Rng, depth: number): unknown =>
  depth === 0 || r.bool(0.4)
    ? r.pick<unknown>([null, true, false, r.int(9), r.string("ab", r.int(3))])
    : r.bool()
      ? r.array(r.int(4), () => value(r, depth - 1))
      : Object.fromEntries(r.array(r.int(4), () => [r.string("xyz", 1), value(r, depth - 1)]))

// A copy made through the wire, which is what an answer asked again is.
const again = (v: unknown): unknown => JSON.parse(JSON.stringify(v))

test("data: an answer is the same as itself asked again, and the equality is symmetric", () =>
  forAll(300, r => {
    const a = value(r, 3)
    const b = value(r, 3)
    assert.ok(data.same(a, again(a)))
    assert.equal(data.same(a, b), data.same(b, a))
    assert.equal(data.same(a, b), data.same(again(a), again(b)))
  }))
