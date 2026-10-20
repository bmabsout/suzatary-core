// lo's laws: a functor, a monad, a fold that rebuilds, a monoidal combine.

import { test } from "node:test"
import assert from "node:assert/strict"

import lo from "../src/algebra/lo"
import { forAll, loState } from "./gen"
import type { Rng } from "./gen"

const gen = (r: Rng) => loState(r, () => r.int(100))
const f = (n: number) => n * 3
const g = (n: number) => n - 7
const k = (n: number): lo.T<number> => (n % 2 === 0 ? lo.val(n / 2) : lo.error(`odd ${n}`))
const h = (n: number): lo.T<number> => (n > 50 ? lo.loading : lo.val(n + 1))
const self = <A>(l: lo.T<A>) => lo.fold<A, lo.T<A>>(l, { loading: () => lo.loading, error: lo.error, val: lo.val })

test("law: map preserves identity and composition", () =>
  forAll(300, r => {
    const l = gen(r)
    assert.deepEqual(lo.map(l, x => x), l)
    assert.deepEqual(lo.map(l, x => f(g(x))), lo.map(lo.map(l, g), f))
  }))

test("law: bind has val as unit on both sides and is associative", () =>
  forAll(300, r => {
    const n = r.int(100)
    const l = gen(r)
    assert.deepEqual(lo.bind(lo.val(n), k), k(n))
    assert.deepEqual(lo.bind(l, lo.val), l)
    assert.deepEqual(lo.bind(lo.bind(l, k), h), lo.bind(l, x => lo.bind(k(x), h)))
  }))

test("law: flatten undoes map(val), and bind is flatten after map", () =>
  forAll(300, r => {
    const l = gen(r)
    assert.deepEqual(lo.flatten(lo.map(l, lo.val)), l)
    assert.deepEqual(lo.flatten(lo.val(l)), l)
    assert.deepEqual(lo.bind(l, k), lo.flatten(lo.map(l, k)))
  }))

test("law: an arm without a value passes through map and bind as the same object", () =>
  forAll(200, r => {
    const l = gen(r)
    if (!lo.isVal(l)) {
      assert.ok(Object.is(lo.map(l, f), l))
      assert.ok(Object.is(lo.bind(l, k), l))
    }
  }))

test("law: folding with the constructors rebuilds the value", () =>
  forAll(300, r => {
    const l = gen(r)
    assert.deepEqual(self(l), l)
  }))

test("law: withDefault takes back what val put in, and is natural in map", () =>
  forAll(300, r => {
    const n = r.int(100)
    const l = gen(r)
    const v = lo.withDefault(l, null)
    assert.equal(lo.withDefault(lo.val(n), null), n)
    assert.deepEqual(lo.withDefault(lo.map(l, f), null), v === null ? null : f(v))
  }))

// Errors renamed by position, so "the first one wins" is visible in the reason.
const list = (r: Rng): lo.T<number>[] =>
  Array.from({ length: r.int(5) }, (_, i) =>
    lo.fold<number, lo.T<number>>(gen(r), { loading: () => lo.loading, val: lo.val, error: () => lo.error(`#${i}`) }),
  )

test("law: combine lifts one value, is a homomorphism from concatenation, and combine() = val([])", () => {
  const flat = (l: lo.T<number[][]>) => lo.map(l, xss => xss.flat())
  assert.deepEqual(lo.combine(), lo.val([]))
  forAll(300, r => {
    const [xs, ys] = [list(r), list(r)]
    const a = gen(r)
    assert.deepEqual(lo.combine(a), lo.map(a, x => [x]))
    assert.deepEqual(flat(lo.combine(lo.combine(...xs), lo.combine(...ys))), lo.combine(...xs, ...ys))
    assert.deepEqual(flat(lo.combine(lo.combine(), lo.combine(...xs))), lo.combine(...xs))
  })
})

test("law: combine is the first error, else loading if any is, else every value in order", () =>
  forAll(300, r => {
    const xs = list(r)
    const errors = xs.flatMap(l => lo.fold<number, string[]>(l, { loading: () => [], val: () => [], error: e => [e] }))
    const values = xs.flatMap(l => lo.fold<number, number[]>(l, { loading: () => [], val: v => [v], error: () => [] }))
    assert.deepEqual(
      lo.combine(...xs),
      errors.length > 0 ? lo.error(errors[0] ?? "")
      : values.length === xs.length ? lo.val(values)
      : lo.loading,
    )
  }))

test("of turns a promise into a value, never a rejection", async () => {
  assert.deepEqual(await lo.of(Promise.resolve(1)), lo.val(1))
  assert.deepEqual(await lo.of(Promise.reject(new Error("refused"))), lo.error("refused"))
})
