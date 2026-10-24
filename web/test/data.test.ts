import assert from "node:assert/strict"
import { test } from "node:test"

import data from "../src/algebra/data"
import type { Rng } from "./gen"
import { forAll } from "./gen"

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
