// The laws of fulfillment's states, its colour and its pie, over seeded values.

import { test } from "node:test"
import assert from "node:assert/strict"

import fulfillment from "../src/todo/fulfillment"
import pie from "../src/view/pie"
import tokens from "../src/view/tokens"
import { forAll } from "./gen"
import type { Rng } from "./gen"

const value = (r: Rng): number => r.int(1_000_001) / 1_000_000

class FakeNode {}
class FakeElement extends FakeNode {
  readonly attrs = new Map<string, string>()
  readonly kids: FakeElement[] = []
  constructor(readonly tag: string) {
    super()
  }
  setAttribute(k: string, v: string) {
    this.attrs.set(k, v)
  }
  appendChild(k: FakeElement) {
    this.kids.push(k)
  }
  addEventListener() {}
}
Object.assign(globalThis, {
  Node: FakeNode,
  document: { createElementNS: (_: string, tag: string) => new FakeElement(tag) },
})

const drawn = (v: number, size?: number): FakeElement => pie.fulfilled(v, size) as unknown as FakeElement

test("law: stateOf is monotone in the value", () => {
  forAll(500, (r) => {
    const [a, b] = [value(r), value(r)].sort((x, y) => x - y) as [number, number]
    assert.ok(fulfillment.rank(fulfillment.stateOf(a)) <= fulfillment.rank(fulfillment.stateOf(b)), `${a} ${b}`)
  })
})

test("law: 0.5 is the first Watch and 0.7 the first Fine", () => {
  assert.deepEqual(fulfillment.stateOf(0), fulfillment.problem)
  assert.deepEqual(fulfillment.stateOf(0.4999), fulfillment.problem)
  assert.deepEqual(fulfillment.stateOf(0.5), fulfillment.watch)
  assert.deepEqual(fulfillment.stateOf(0.6999), fulfillment.watch)
  assert.deepEqual(fulfillment.stateOf(0.7), fulfillment.fine)
  assert.deepEqual(fulfillment.stateOf(1), fulfillment.fine)
})

test("law: worst is a monoid with fine as its identity", () => {
  const states = [fulfillment.problem, fulfillment.watch, fulfillment.fine]
  forAll(200, (r) => {
    const [a, b, c] = [r.pick(states), r.pick(states), r.pick(states)]
    assert.deepEqual(fulfillment.worst(a, fulfillment.fine), a)
    assert.deepEqual(fulfillment.worst(fulfillment.fine, a), a)
    assert.deepEqual(fulfillment.worst(fulfillment.worst(a, b), c), fulfillment.worst(a, fulfillment.worst(b, c)))
    assert.deepEqual(fulfillment.worst(a, b, c), fulfillment.worst(fulfillment.worst(a, b), c))
  })
  assert.deepEqual(fulfillment.worst(), fulfillment.fine)
})

test("law: at each of the scale's stops the colour is that stop's, the ends included", () => {
  for (const s of tokens.fulfillment) assert.deepEqual(fulfillment.lch(s.at), s.colour)
  assert.deepEqual(tokens.fulfillment.map(s => s.at), [0, 0.33, 0.58, 0.78, 1])
})

// The stops either side of a value: the segment it lies in.
const segment = (v: number): number => tokens.fulfillment.findIndex(s => s.at > v)

test("law: the colour is continuous and linear in each OKLCH channel between two stops, its hue increasing", () => {
  forAll(500, (r) => {
    const [a, b] = [value(r), value(r)].sort((x, y) => x - y) as [number, number]
    const [x, y] = [fulfillment.lch(a), fulfillment.lch(b)]
    const i = segment(a)
    const [lo, hi] = [tokens.fulfillment[i - 1], tokens.fulfillment[i]]
    if (i === segment(b) && lo !== undefined && hi !== undefined)
      for (const k of ["l", "c", "h"] as const) {
        const slope = (hi.colour[k] - lo.colour[k]) / (hi.at - lo.at)
        assert.ok(Math.abs(y[k] - x[k] - slope * (b - a)) < 1e-9, `${k} at ${a} and ${b}`)
      }
    assert.ok(a === b ? x.h === y.h : x.h < y.h, `hue at ${a} and ${b}`)
    const near = Math.min(1, a + 1e-9)
    assert.ok(Math.abs(fulfillment.lch(near).l - x.l) < 1e-5, `continuous at ${a}`)
  })
})

test("the colour prints as the CSS oklch() of its channels", () => {
  assert.equal(fulfillment.colour(0), "oklch(44.000% 0.1600 14.000)")
  assert.equal(fulfillment.colour(1), "oklch(58.000% 0.1100 185.000)")
})

test("law: the pie's filled share is the value, in the value's colour", () => {
  forAll(300, (r) => {
    const v = value(r)
    const size = 1 + r.int(64)
    const pie = drawn(v, size)
    const fill = pie.kids.find((k) => k.attrs.get("class") === "piefill")
    assert.ok(fill !== undefined)
    const [dash] = (fill.attrs.get("stroke-dasharray") ?? "").split(" ").map(Number)
    assert.equal((dash ?? NaN) / Number(fill.attrs.get("pathLength")), v)
    assert.equal(fill.attrs.get("transform"), `rotate(-90 ${size / 2} ${size / 2})`, "from twelve o'clock")
    assert.equal(pie.attrs.get("style"), `color:${fulfillment.colour(v)}`)
    assert.equal(pie.attrs.get("width"), String(size))
  })
  assert.equal(drawn(0.5).attrs.get("width"), String(pie.SIZE))
})
