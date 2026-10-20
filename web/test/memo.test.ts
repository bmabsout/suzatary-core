// The memoised fold's laws (the Prodrome's law 8, design §6.2), over generated lists, binary trees and rose
// trees and generated edits, named by the core's own `memo::name`: memoised equals plain whatever the cache
// dropped, an edit computes exactly its spine and hits at the top of every unchanged subtree, a name is computed
// once, and a balanced sequence is the sequence, its shape a function of its elements, its spine logarithmic.

import { test } from "node:test"
import assert from "node:assert/strict"
import { createRequire } from "node:module"

import type { Core } from "../src/prodrome/core"
import memo from "../src/algebra/memo"
import table from "../src/algebra/table"
import type { Rng } from "./gen"
import { forAll } from "./gen"

const built = process.env["SUZATARY_PRODROME_WASM"]
if (built === undefined || built === "") throw new Error("SUZATARY_PRODROME_WASM is unset — run scripts/web-test.sh")
const core = createRequire(import.meta.url)(`${built}/nodejs/prodrome.js`) as Core
const named: memo.Namer = (own, kids) => core.memo_name(own, kids)

type Node = memo.Tree<number>
type Shape = Shape[]

const list = (r: Rng): Shape => Array.from({ length: r.int(40) }).reduce<Shape>(tail => [tail], [])
const binary = (r: Rng, depth = 6): Shape => (depth === 0 || r.bool(0.3) ? [] : [binary(r, depth - 1), binary(r, depth - 1)])
const rose = (r: Rng, depth = 4): Shape => (depth === 0 ? [] : r.array(r.int(4), () => rose(r, depth - 1)))
const shape = (r: Rng): Shape => r.pick([list, binary, rose])(r)

// Every label fresh, or drawn from three so that subtrees repeat and share names.
const labels = (modulo: number): (() => number) => {
  let next = 0
  return () => (next += 1) % modulo
}

const build = (s: Shape, label: () => number): Node => {
  const v = label()
  return memo.node(named, String(v), v, s.map(k => build(k, label)))
}

const size = (n: Node): number => n.kids.reduce((t, k) => t + size(k), 1)
const names = (n: Node, into = new Set<string>()): Set<string> => (into.add(n.name), n.kids.forEach(k => names(k, into)), into)

type Edit = { readonly type: "Relabel" } | { readonly type: "Graft"; readonly shape: Shape }
const edits = (r: Rng, n: Node): Map<number, Edit> =>
  new Map(r.array(r.int(4), () => [r.int(size(n)), r.bool() ? { type: "Relabel" } : { type: "Graft", shape: rose(r) }] as const))

// The tree with the edits at their preorder positions, and the names of every node an edit changed: an edited
// node, every node of a grafted subtree, every ancestor of one.
const edited = (n: Node, at: Map<number, Edit>, label: () => number): [Node, Set<string>] => {
  const changed = new Set<string>()
  let position = 0
  const walk = (x: Node): Node => {
    const e = at.get(position)
    position += 1
    if (e?.type === "Graft") {
      position += size(x) - 1
      const g = build(e.shape, label)
      names(g, changed)
      return g
    }
    const before = changed.size
    const kids = x.kids.map(walk)
    const v = e?.type === "Relabel" ? label() : x.v
    const y = memo.node(named, String(v), v, kids)
    if (e?.type === "Relabel" || changed.size > before) changed.add(y.name)
    return y
  }
  return [walk(n), changed]
}

// Two algebras with one result type, so they share a cache and only their names keep their entries apart.
const sum = memo.pointwise<number, number>(named("sum", []), (v, rs) => rs.reduce((a, b) => a + b, v))
const weigh = memo.pointwise<number, number>(named("weigh", []), (v, rs) => rs.reduce((acc, r, i) => (acc * 31 + r * (i + 1)) % 1_000_000_007, v + 7))

// A table that drops what the draw picks between folds, beside the bounded one.
const dropping = (r: Rng): table.Table<number> & { readonly thin: () => void } => {
  const t = table.of<number>(table.keep())
  return { ...t, thin: () => t.pairs().forEach(([k]) => r.bool() && t.remove(k)) }
}

test("the memoised fold is the fold, whatever the cache dropped", () =>
  forAll(128, r => {
    const label = labels(3)
    let t = build(shape(r), label)
    const dropped = dropping(r)
    const bounded = table.of<number>(table.lru(r.int(8)))
    for (let round = 0; round < 1 + r.int(3); round += 1) {
      for (const alg of [sum, weigh]) {
        const want = memo.plain(alg, t)
        assert.equal(memo.fold(alg, t, dropped)[0], want)
        assert.equal(memo.fold(alg, t, bounded)[0], want)
      }
      dropped.thin()
      t = edited(t, edits(r, t), label)[0]
    }
  }))

test("a key holding two values is computed, never chosen, and reported where the fold met it", () =>
  forAll(128, r => {
    const t = build(shape(r), labels(Number.MAX_SAFE_INTEGER))
    const cache = table.of<number>(table.keep())
    memo.fold(sum, t, cache)
    const all = [...names(t)]
    const forged = new Set(all.filter(() => r.bool(0.3)))
    cache.join([...forged].map(n => [table.key(sum.name, n), -1] as const))
    const [v, report] = memo.fold(sum, t, cache)
    assert.equal(v, memo.plain(sum, t))
    const found = new Set(memo.names(report, "Finding"))
    for (const n of found) assert.ok(forged.has(n), `${n} was not forged`)
    if (forged.has(t.name)) assert.ok(found.has(t.name), "the root's finding was used")
    assert.ok(memo.byDepth(report).every(c => c.findings >= 0))
  }))

// What a fold should look at when the cache holds every node but the changed ones.
const expected = (n: Node, changed: Set<string>, depth = 0): string[] =>
  changed.has(n.name) ? [`${n.name} ${depth} Computed`, ...n.kids.flatMap(k => expected(k, changed, depth + 1))] : [`${n.name} ${depth} Hit`]

test("an edit computes exactly its spine, each node once, and hits at the top of every unchanged subtree", () =>
  forAll(128, r => {
    const label = labels(Number.MAX_SAFE_INTEGER)
    const t = build(shape(r), label)
    const cache = table.of<number>(table.keep())
    memo.fold(sum, t, cache)
    const [after, changed] = edited(t, edits(r, t), label)
    const [v, report] = memo.fold(sum, after, cache)
    assert.equal(v, memo.plain(sum, after))
    const seen = report.visits.map(x => `${x.name} ${x.depth} ${x.outcome}`)
    assert.equal(new Set(seen).size, seen.length, "a node looked at twice")
    assert.deepEqual([...seen].sort(), expected(after, changed).sort())
    const depths = memo.byDepth(report)
    assert.equal(depths.reduce((t, c) => t + c.computed, 0), changed.size)
    assert.equal((depths[0]?.hits ?? 0) + (depths[0]?.computed ?? 0), 1)
    assert.equal(report.waves.reduce((t, w) => t + w.nodes, 0), changed.size)
  }))

test("with subtrees shared, a fold computes exactly the names it has not seen, each once", () =>
  forAll(128, r => {
    const label = labels(3)
    const t = build(shape(r), label)
    const cache = table.of<number>(table.keep())
    memo.fold(weigh, t, cache)
    const after = edited(t, edits(r, t), label)[0]
    const [v, report] = memo.fold(weigh, after, cache)
    assert.equal(v, memo.plain(weigh, after))
    const computed = memo.names(report, "Computed")
    const before = names(t)
    assert.equal(new Set(computed).size, computed.length, "a name computed twice")
    assert.deepEqual(new Set(computed), new Set([...names(after)].filter(n => !before.has(n))))
  }))

test("a namer read through a cache names as the namer does, whatever the cache dropped", () =>
  forAll(64, r => {
    const cached = memo.tabulatedNamer(named, table.of(table.lru(r.int(6))))
    const t = build(shape(r), labels(3))
    const again = (n: Node): Node => memo.node(cached, String(n.v), n.v, n.kids.map(again))
    for (let k = 0; k < 3; k += 1) assert.deepEqual(names(again(t)), names(t))
  }))

test("a wave holds only nodes whose children are known, so one apply per wave is the fold", () =>
  forAll(64, r => {
    const t = build(shape(r), labels(Number.MAX_SAFE_INTEGER))
    const waves: number[] = []
    const counted: memo.Algebra<number, number> = { name: named("counted", []), apply: ls => (waves.push(ls.length), sum.apply(ls)) }
    const [v, report] = memo.fold(counted, t, table.of<number>(table.keep()))
    assert.equal(v, memo.plain(sum, t))
    assert.deepEqual(waves, report.waves.map(w => w.nodes))
  }))

// --- a balanced sequence --------------------------------------------------------------------------------------

type Seq = memo.Tree<number | null>
const item = (v: number): Seq => memo.node(named, `item ${v}`, v, [])
const chunk = (kids: Seq[]): Seq => memo.node(named, "chunk", null, kids)
const balanced = (vs: readonly number[]): Seq | undefined => memo.balance(vs.map(item), s => s.name, chunk)
const elements = memo.pointwise<number | null, number[]>(named("elements", []), (v, rs) => (v === null ? rs.flat() : [v]))
const depth = (s: Seq): number => s.kids.reduce((d, k) => Math.max(d, depth(k) + 1), 0)

type SeqEdit = { readonly type: "Replace"; readonly at: number; readonly v: number } | { readonly type: "Insert"; readonly at: number; readonly v: number } | { readonly type: "Remove"; readonly at: number }
const seqEdit = (r: Rng, len: number): SeqEdit =>
  r.pick<SeqEdit>([
    { type: "Replace", at: r.int(len), v: r.int(1 << 30) },
    { type: "Insert", at: r.int(len + 1), v: r.int(1 << 30) },
    { type: "Remove", at: r.int(len) },
  ])
const applied = (vs: readonly number[], e: SeqEdit): number[] =>
  e.type === "Replace" ? vs.map((v, i) => (i === e.at ? e.v : v))
  : e.type === "Insert" ? [...vs.slice(0, e.at), e.v, ...vs.slice(e.at)]
  : vs.filter((_, i) => i !== e.at)

test("a balanced sequence folds back to the sequence", () =>
  forAll(64, r => {
    const vs = r.array(r.int(600), () => r.int(64))
    const t = balanced(vs)
    if (t === undefined) return assert.equal(vs.length, 0)
    assert.deepEqual(memo.fold(elements, t, table.of(table.keep()))[0], vs)
  }))

test("the shape is the elements: an edit and its undoing build the tree the sequence had", () =>
  forAll(64, r => {
    const vs = r.array(1 + r.int(400), () => r.int(1 << 30))
    const e = seqEdit(r, vs.length)
    const undo: SeqEdit =
      e.type === "Replace" ? { ...e, v: vs[e.at] as number } : e.type === "Insert" ? { type: "Remove", at: e.at } : { type: "Insert", at: e.at, v: vs[e.at] as number }
    const back = applied(applied(vs, e), undo)
    assert.deepEqual(back, vs)
    assert.equal(balanced(back)?.name, balanced(vs)?.name)
  }))

test("one edit to a balanced sequence computes a spine logarithmic in its length", () =>
  forAll(32, r => {
    const vs = r.array(256 + r.int(1792), () => r.int(1 << 30))
    const after = applied(vs, seqEdit(r, vs.length))
    const before = balanced(vs) as Seq
    const t = balanced(after) as Seq
    const bound = 4 * Math.ceil(Math.log2(vs.length + 1))
    assert.ok(depth(t) <= bound, `depth ${depth(t)} over ${bound}`)
    const cache = table.of<number[]>(table.keep())
    memo.fold(elements, before, cache)
    const [v, report] = memo.fold(elements, t, cache)
    assert.deepEqual(v, after)
    const computed = memo.names(report, "Computed").length
    assert.ok(computed <= bound, `${computed} computed of ${vs.length} elements`)
  }))

// An element with an identity apart from its content, cut by that identity, its chunk by its children's.
type Cut = { readonly t: memo.Tree<number | null>; readonly cut: string }
const byIdentity = (vs: readonly (readonly [string, number])[]): Seq | undefined =>
  memo.balance<Cut>(
    vs.map(([id, v]) => ({ t: item(v), cut: named(id, []) })),
    x => x.cut,
    run => ({ t: chunk(run.map(x => x.t)), cut: named("run", run.map(x => x.cut)) }),
  )?.t

test("cut by identity, an element's new content renames exactly its path to the root", () =>
  forAll(64, r => {
    const vs = Array.from({ length: 1 + r.int(400) }, (_, i) => [`e${i}`, r.int(1 << 30)] as const)
    const at = r.int(vs.length)
    const before = byIdentity(vs) as Seq
    const after = byIdentity(vs.map(([id, v], i) => [id, i === at ? v + 1 : v] as const)) as Seq
    const all = (s: Seq): Seq[] => [s, ...s.kids.flatMap(all)]
    const old = new Set(all(before).map(s => s.name))
    const path = (s: Seq, i: number): string[] => {
      let k = i
      for (const kid of s.kids) {
        const n = memo.plain(elements, kid).length
        if (k < n) return [s.name, ...path(kid, k)]
        k -= n
      }
      return [s.name]
    }
    assert.deepEqual(new Set(all(after).map(s => s.name).filter(n => !old.has(n))), new Set(path(after, at)))
  }))

// Each level is cut at WIDEST at the latest, and a level whose every node ends a run closes as one chunk.
test("equal elements still balance", () =>
  [7, 8, 9, 10].forEach(v => {
    const vs = Array.from({ length: 5000 }, () => v)
    const t = balanced(vs) as Seq
    const bound = Math.ceil(Math.log(vs.length) / Math.log(memo.WIDEST)) + 1
    assert.ok(depth(t) <= bound, `depth ${depth(t)} over ${bound}`)
    const cache = table.of<number[]>(table.keep())
    memo.fold(elements, t, cache)
    const computed = memo.names(memo.fold(elements, balanced([...vs, v]) as Seq, cache)[1], "Computed").length
    assert.ok(computed <= 2 * (bound + 1), `${computed} computed`)
  }))
