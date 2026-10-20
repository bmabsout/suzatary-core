// The tabulation's laws (the Prodrome's law 8, design §6.2), over generated pairs: a join is union, so
// commutative, associative and idempotent; a key holding two values reads as a finding with both and never as
// either, reported where it is made; an entry may go and reads as a miss; a bounded table holds its bound.

import { test } from "node:test"
import assert from "node:assert/strict"

import table from "../src/algebra/table"
import type { Rng } from "./gen"
import { forAll } from "./gen"

type Pair = readonly [table.Key, number]

// Few keys and few values, so keys collide and some hold two values.
const pairs = (r: Rng): Pair[] => r.array(r.int(24), () => [table.key(r.pick(["f", "g"]), r.string("abc", 1)), r.int(3)] as const)

const filled = (ps: readonly Pair[]): table.Table<number> => {
  const t = table.of<number>(table.keep())
  t.join(ps)
  return t
}

// A table's meaning: its set of pairs.
const meaning = (t: table.Table<number>): string[] => t.pairs().map(([k, v]) => `${k}=${v}`).sort()

test("law: a join is union, commutative, associative and idempotent", () =>
  forAll(200, r => {
    const [a, b, c] = [pairs(r), pairs(r), pairs(r)]
    assert.deepEqual(meaning(filled([...a, ...b])), meaning(filled([...b, ...a])))
    assert.deepEqual(meaning(filled([...a, ...b, ...c])), meaning(filled([...a, ...filled([...b, ...c]).pairs()])))
    assert.deepEqual(meaning(filled([...a, ...a])), meaning(filled(a)))
    const t = filled(a)
    t.join(t.pairs())
    assert.deepEqual(meaning(t), meaning(filled(a)))
  }))

test("law: a key reads as the values it holds, one as a hit and two or more as a finding, never as either", () =>
  forAll(200, r => {
    const ps = pairs(r)
    const t = filled(ps)
    for (const [k] of ps) {
      const vs = [...new Set(ps.filter(p => p[0] === k).map(p => p[1]))]
      const l = t.get(k)
      assert.deepEqual(
        table.fold<number, number[]>(l, { miss: () => [], hit: v => [v], finding: ws => [...ws] }).sort(),
        vs.sort(),
      )
      assert.equal(l.type, vs.length === 1 ? "Hit" : "Finding")
      assert.equal(table.hit(l), vs.length === 1 ? vs[0] : undefined)
    }
    assert.deepEqual(t.findings().sort(), [...new Set(ps.map(p => p[0]))].filter(k => t.get(k).type === "Finding").sort())
  }))

test("law: a finding is reported exactly where it is made, by an insert or by a join", () =>
  forAll(200, r => {
    const [a, b] = [pairs(r), pairs(r)]
    const t = filled(a)
    const before = new Set(t.findings())
    const made = t.join(b)
    assert.deepEqual(made.sort(), t.findings().filter(k => !before.has(k)).sort())
    const u = table.of<number>(table.keep())
    for (const [k, v] of [...a, ...b]) {
      const was = u.get(k).type
      const found = u.insert(k, v)
      assert.equal(found, was === "Hit" && u.get(k).type === "Finding")
    }
  }))

test("law: an entry may go, and then reads as a miss; nothing else moves", () =>
  forAll(200, r => {
    const ps = pairs(r)
    const t = filled(ps)
    const [gone] = r.array(1, () => r.pick(ps.length === 0 ? ["f x"] : ps.map(p => p[0])))
    const others = meaning(t).filter(p => !p.startsWith(`${gone}=`))
    t.remove(gone ?? "")
    assert.equal(t.get(gone ?? "").type, "Miss")
    assert.deepEqual(meaning(t), others)
  }))

test("law: a bounded table holds at most its bound, the least recently touched going first", () =>
  forAll(200, r => {
    const bound = 1 + r.int(5)
    const t = table.of<number>(table.lru(bound))
    const touched: table.Key[] = []
    for (const [k, v] of pairs(r)) {
      t.insert(k, v)
      touched.push(k)
      assert.ok(t.size() <= bound)
      if (r.bool(0.3)) {
        const back = r.pick(touched)
        if (t.get(back).type !== "Miss") touched.push(back)
      }
    }
    const newest = [...new Set([...touched].reverse())].slice(0, Math.min(bound, t.size()))
    for (const k of newest) assert.notEqual(t.get(k).type, "Miss", `${k} was among the ${bound} most recent`)
  }))
