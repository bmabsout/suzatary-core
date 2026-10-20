// d's laws: a decoder returns a value it accepts unchanged, drops extra fields, refuses a missing one by name.

import { test } from "node:test"
import assert from "node:assert/strict"

import d from "../src/wire/decoder"
import lo from "../src/algebra/lo"
import { forAll, SLUG } from "./gen"
import type { Rng } from "./gen"

type Row = { name: string; n: number; ok: boolean; tags: string[]; note: string | null; by: Record<string, number> }
const Row = d.obj<Row>({
  name: d.str,
  n: d.num,
  ok: d.bool,
  tags: d.arr(d.str),
  note: d.nullable(d.str),
  by: d.rec(d.num),
})
type Shape = { kind: "dot" } | { kind: "box"; w: number; rows: Row[] }
const Shape = d.sum<Shape, "kind">("kind", { dot: {}, box: { w: d.num, rows: d.arr(Row) } })

const word = (r: Rng) => r.string(SLUG, 1 + r.int(8))
const row = (r: Rng): Row => ({
  name: word(r),
  n: r.int(1000) - 500,
  ok: r.bool(),
  tags: r.array(r.int(4), () => word(r)),
  note: r.bool() ? null : word(r),
  by: Object.fromEntries(r.array(r.int(3), () => [word(r), r.int(9)])),
})
const shape = (r: Rng): Shape => (r.bool() ? { kind: "dot" } : { kind: "box", w: r.int(50), rows: r.array(r.int(3), () => row(r)) })

test("law: a value of the type decodes to itself", () =>
  forAll(300, r => {
    const s = shape(r)
    assert.deepEqual(Shape(s), lo.val(s))
    assert.deepEqual(Shape(JSON.parse(JSON.stringify(s))), lo.val(s))
  }))

test("law: an extra field is dropped, never a refusal", () =>
  forAll(200, r => {
    const x = row(r)
    assert.deepEqual(Row({ ...x, [`extra${word(r)}`]: word(r) }), lo.val(x))
  }))

test("law: a missing field is a refusal naming it", () =>
  forAll(200, r => {
    const x = row(r)
    const k = r.pick(Object.keys(x))
    assert.deepEqual(Row(Object.fromEntries(Object.entries(x).filter(([j]) => j !== k))), lo.error(`missing field "${k}"`))
  }))

test("a refusal names the path to the field that is wrong", () => {
  const x = { kind: "box", w: 1, rows: [{ name: "a", n: 1, ok: true, tags: ["t", 3], note: null, by: {} }] }
  assert.deepEqual(Shape(x), lo.error(".rows[0].tags[1]: expected a string, got number"))
  assert.deepEqual(Shape({ kind: "star" }), lo.error('.kind: expected one of dot | box, got "star"'))
  assert.deepEqual(Row(null), lo.error("expected an object, got null"))
  assert.deepEqual(d.num(Number.NaN), lo.error("expected a number, got number"))
})

test("oneOf admits its members and nothing else", () => {
  const State = d.oneOf<"open" | "done">({ open: null, done: null })
  assert.deepEqual(State("open"), lo.val("open"))
  assert.equal(State("toString").type, "Error")
  assert.equal(State("").type, "Error")
})

test("open keeps every field and checks the named ones", () => {
  const Node = d.open<{ kind: string; [k: string]: unknown }>({ kind: d.str })
  assert.deepEqual(Node({ kind: "flat", value: 0.5 }), lo.val({ kind: "flat", value: 0.5 }))
  assert.equal(Node({ value: 0.5 }).type, "Error")
})

test("alt refused by both arms names why each refused", () => {
  const Worth = d.alt(d.obj<{ value: number }>({ value: d.num }), d.lit("absent"))
  assert.deepEqual(Worth({}), lo.error('missing field "value"; or expected "absent", got object'))
  assert.deepEqual(Worth("absent"), lo.val("absent"))
})
