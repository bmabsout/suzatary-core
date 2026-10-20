// The frame's step: a value read shows, a failure shows only when nothing has, own events move only a value.

import { test } from "node:test"
import assert from "node:assert/strict"

import frame from "../src/view/frame"
import lo from "../src/algebra/lo"
import { forAll, loState } from "./gen"
import type { Rng } from "./gen"

type Ev = frame.Event<number, number>
const step = frame.step<number, number>((v, e) => v + e)
const genState = (r: Rng) => loState(r, () => r.int(100))
const genEvent = (r: Rng): Ev => r.pick<Ev>([frame.read(lo.val(r.int(100))), frame.read(lo.error(r.string("abc", 2))), frame.read(lo.loading), frame.own(r.int(10))])

test("a value read shows, and a failure or a wait before any value is the page's state", () => {
  assert.deepEqual(step(lo.loading, frame.read(lo.val(3))), lo.val(3))
  assert.deepEqual(step(lo.loading, frame.read(lo.error("down"))), lo.error("down"))
  assert.deepEqual(step(lo.error("down"), frame.read(lo.loading)), lo.loading)
})

test("a value read after a failure lands", () => {
  assert.deepEqual(step(lo.error("down"), frame.read(lo.val(1))), lo.val(1))
})

test("law: a value read replaces whatever was shown", () => {
  forAll(200, r => {
    const v = r.int(100)
    assert.deepEqual(step(genState(r), frame.read(lo.val(v))), lo.val(v))
  })
})

test("law: a failed or waiting reading never takes a value off the screen", () => {
  forAll(200, r => {
    const s = genState(r)
    for (const l of [lo.error("down"), lo.loading]) assert.deepEqual(step(s, frame.read(l)), lo.isVal(s) ? s : l)
  })
})

test("law: an own event before a value returns the same object, so nobody wakes; after one it moves the value", () => {
  forAll(200, r => {
    const s = genState(r)
    const e = r.int(10)
    const next = step(s, frame.own(e))
    assert.deepEqual(next, lo.map(s, v => v + e))
    if (!lo.isVal(s)) assert.ok(Object.is(next, s))
  })
})

test("law: mapOwn is a functor, and step is natural in it", () => {
  const f = (n: number) => n * 2
  forAll(300, r => {
    const e = genEvent(r)
    const s = genState(r)
    assert.deepEqual(frame.mapOwn(x => x, e), e)
    assert.deepEqual(frame.mapOwn(x => f(x + 1), e), frame.mapOwn(f, frame.mapOwn(x => x + 1, e)))
    assert.deepEqual(step(s, frame.mapOwn(f, e)), frame.step<number, number>((v, x) => v + f(x))(s, e))
  })
})

test("law: folding an event with the constructors rebuilds it", () => {
  forAll(200, r => {
    const e = genEvent(r)
    assert.deepEqual(frame.fold<number, number, Ev>({ read: frame.read, own: frame.own }, e), e)
  })
})

test("law: a failed reading and an own event commute", () => {
  forAll(300, r => {
    const s: lo.T<number> = genState(r)
    const [failed, own] = [frame.read(lo.error(r.string("abc", 2))), frame.own(r.int(10))]
    assert.deepEqual(step(step(s, failed), own), step(step(s, own), failed))
  })
})

test("law: a page keeps what it says it keeps of its own state; with nothing shown a value read is itself", () => {
  const keeps = (was: number, now: number) => was * 1000 + now
  const stepK = frame.step<number, number>((v, e) => v + e, keeps)
  forAll(300, r => {
    const s = genState(r)
    const v = r.int(100)
    assert.deepEqual(stepK(s, frame.read(lo.val(v))), lo.isVal(s) ? lo.val(keeps(lo.withDefault(s, 0), v)) : lo.val(v))
  })
})

test("law: an own event that leaves the value as it was returns the same object, so nobody wakes", () => {
  const same = frame.step<number, number>((v, e) => (e === 0 ? v : v + e))
  forAll(200, r => {
    const s = genState(r)
    assert.ok(Object.is(same(s, frame.own(0)), s))
  })
})
