// The tree of terms: app/core/src/tree.rs's laws, over the seeded generator and the real core.

import { test } from "node:test"
import assert from "node:assert/strict"
import { createRequire } from "node:module"
import { readFileSync } from "node:fs"

import type { SpecNode } from "../src/wire/generated/wire"
import type { Core } from "../src/prodrome/core"
import { entriesOf, foldObjects, linked, linker, within } from "../src/prodrome/core"
import grouping from "../src/todo/grouping"
import tree from "../src/algebra/tree"
import { forAll } from "./gen"
import type { Rng } from "./gen"

const core = (): Core => {
  const built = process.env["SUZATARY_PRODROME_WASM"]
  if (built === undefined || built === "") throw new Error("SUZATARY_PRODROME_WASM is unset — run scripts/web-test.sh")
  return createRequire(import.meta.url)(`${built}/nodejs/prodrome.js`) as Core
}
const wasm = core()
const NOW = "2026-09-02T12:00:00"

const { worth, segment, path, paths, classifier } = grouping

const genTree = (r: Rng, depth = 3): tree.Tree<number> =>
  tree.of(r.int(100), depth === 0 ? [] : r.array(r.int(4), () => genTree(r, depth - 1)))

const genSegment = (r: Rng): grouping.Segment =>
  r.pick([
    () => segment.category(r.pick(["a", "b"])),
    () => segment.band(r.pick([{ type: "Problem" as const }, { type: "Fine" as const }, { type: "Unpriced" as const }])),
    () => segment.todo(r.pick(["xxx", "yyy"])),
  ])()

const genPath = (r: Rng): grouping.Path => r.array(r.int(5), () => path.one(genSegment(r))).reduce(path.combine, path.empty)

// A random list with subtodos: a DAG whose parents come first, each priced by
// its own flat — or unpriced, its own function `absent` (∅) — and the
// references to its children.
type World = {
  members: grouping.Members
  functions: Record<string, SpecNode>
  price: (t: SpecNode) => number | null
}

const genWorld = (r: Rng): World => {
  const ids = r.array(1 + r.int(7), () => "").map((_, i) => `t${i}`)
  const kids = new Map(ids.map(p => [p, ids.filter(c => c > p && r.bool(0.25))]))
  const functions = Object.fromEntries(
    ids.map(id => {
      const own: SpecNode = r.bool(0.25) ? { kind: "absent" } : { kind: "flat", value: r.int(101) / 100 }
      const cs = kids.get(id) ?? []
      return [id, cs.length === 0 ? own : { kind: "conj", p: -4, terms: [own, ...cs.map(c => ({ kind: "ref", todo: c }))] }]
    }),
  )
  const link = linker(within(wasm, functions))
  const price = linked(wasm, link, { outcomes: {}, tended: {} }, NOW)
  const members = new Map(
    ids.map(id => [
      id,
      {
        category: r.pick(["a", "b", "c"]),
        worth: worth.of(price, { kind: "ref", todo: id }),
        parents: ids.filter(p => (kids.get(p) ?? []).includes(id)),
      },
    ]),
  )
  return { members, functions, price }
}

const groupings: grouping.Grouping[] = ["category", "urgency", "flat"]

test("law: tree's map is a functor, and scan keeps the cata at every node", () => {
  forAll(200, r => {
    const t = genTree(r)
    const f = (x: number) => x + 1
    const g = (x: number) => x * 2
    assert.deepEqual(tree.map(x => x, t), t)
    assert.deepEqual(tree.map(x => f(g(x)), t), tree.map(f, tree.map(g, t)))
    const sum = (v: number, rs: number[]) => rs.reduce((a, b) => a + b, v)
    const scanned = tree.scan(sum, t)
    assert.equal(scanned.v[1], tree.cata(sum, t))
    assert.deepEqual(tree.map(([v]) => v, scanned), t)
    assert.equal(tree.nodes(t).length, tree.size(t))
  })
})

test("law: paths form a monoid, and a segment twice is once", () => {
  forAll(300, r => {
    const [a, b, c] = [genPath(r), genPath(r), genPath(r)]
    assert.deepEqual(path.combine(a, path.empty), a)
    assert.deepEqual(path.combine(path.empty, a), a)
    assert.deepEqual(path.combine(path.combine(a, b), c), path.combine(a, path.combine(b, c)))
    const s = path.one(genSegment(r))
    const once = path.combine(a, s)
    assert.deepEqual(path.combine(once, s), once)
  })
})

test("law: sets of paths form a monoid under the pairwise combine", () => {
  forAll(200, r => {
    const x = paths.combine([genPath(r), genPath(r)], paths.empty)
    const y = [genPath(r)]
    const z = paths.combine([genPath(r), path.empty], paths.empty)
    assert.deepEqual(paths.combine(x, paths.empty), x)
    assert.deepEqual(paths.combine(paths.empty, x), x)
    assert.deepEqual(paths.combine(paths.combine(x, y), z), paths.combine(x, paths.combine(y, z)))
  })
})

test("law: a group of one is its member's term, and so its value", () => {
  forAll(60, r => {
    const w = genWorld(r)
    for (const by of groupings) {
      const check = (t: tree.Tree<grouping.Node>): void => {
        const only = t.kids[0]
        if (grouping.place.todo(t.v.place) === null && t.kids.length === 1 && only !== undefined) {
          assert.deepEqual(t.v.term, only.v.term, by)
          assert.deepEqual(t.v.worth, only.v.worth, by)
        }
        t.kids.forEach(check)
      }
      check(grouping.grow(w.members, classifier.of(by), w.price))
    }
  })
})

test("law: grouping twice by the same key is grouping once", () => {
  forAll(60, r => {
    const w = genWorld(r)
    for (const by of ["Category", "Urgency"] as const) {
      const once = grouping.grow(w.members, [by, "Ancestry"], w.price)
      assert.deepEqual(grouping.grow(w.members, [by, by, "Ancestry"], w.price), once, by)
    }
  })
})

test("law: every node is worth the fulfillment of its linked term, and a todo its own function", () => {
  forAll(40, r => {
    const w = genWorld(r)
    const specs = JSON.stringify(w.functions)
    for (const by of groupings) {
      for (const n of tree.nodes(grouping.grow(w.members, classifier.of(by), w.price))) {
        const linkedTerm = wasm.link(JSON.stringify(n.term), specs)
        assert.equal(worth.toNumber(n.worth), wasm.fulfillment(linkedTerm, NOW, "{}") ?? null)
        const todo = grouping.place.todo(n.place)
        if (todo !== null) {
          const own = wasm.link(JSON.stringify(w.functions[todo]), specs)
          assert.equal(worth.toNumber(n.worth), wasm.fulfillment(own, NOW, "{}") ?? null, todo)
        }
      }
    }
  })
})

test("law: children ascend in value at every level, a term that does not link first", () => {
  forAll(60, r => {
    const w = genWorld(r)
    for (const by of groupings) {
      const check = (t: tree.Tree<grouping.Node>): void => {
        const us = t.kids.map(k => worth.urgency(k.v.worth))
        assert.deepEqual(us, [...us].sort((a, b) => a - b), by)
        t.kids.forEach(check)
      }
      check(grouping.grow(w.members, classifier.of(by), w.price))
    }
  })
})

test("law: a todo whose function is absent reads ∅, bands Unpriced, and sorts after every priced sibling", () => {
  forAll(60, r => {
    const w = genWorld(r)
    for (const [id, m] of w.members) {
      const f = w.functions[id]
      if (f?.kind !== "absent") continue
      assert.deepEqual(m.worth, worth.absent, id)
      assert.equal(worth.state(m.worth).type, "Unpriced", id)
    }
    const t = grouping.grow(w.members, classifier.of("flat"), w.price)
    const kinds = t.kids.map(k => k.v.worth.type)
    const firstAbsent = kinds.indexOf("Absent")
    if (firstAbsent >= 0) assert.ok(kinds.slice(firstAbsent).every(k => k === "Absent"), kinds.join(","))
  })
})

test("law: every member is placed, under each of its parents in every grouping", () => {
  forAll(60, r => {
    const w = genWorld(r)
    for (const by of groupings) {
      const t = grouping.grow(w.members, classifier.of(by), w.price)
      const placed = new Set(tree.nodes(t).flatMap(n => grouping.place.todo(n.place) ?? []))
      assert.deepEqual([...placed].sort(), [...w.members.keys()].sort(), by)
      const under = (t: tree.Tree<grouping.Node>, parent: string | null): [string, string | null][] => {
        const me = grouping.place.todo(t.v.place)
        return [...(me === null ? [] : [[me, parent] as [string, string | null]]), ...t.kids.flatMap(k => under(k, me ?? parent))]
      }
      const seen = under(t, null)
      for (const [id, m] of w.members) {
        const parentsSeen = seen.filter(([c]) => c === id).map(([, p]) => p)
        if (m.parents.length === 0) assert.ok(parentsSeen.includes(null), `${id} is a root in ${by}`)
        for (const p of m.parents) assert.ok(parentsSeen.includes(p), `${id} sits under ${p} in ${by}`)
      }
    }
  })
})

const member = (category: string, v: number, parents: string[]): grouping.Member => ({ category, worth: worth.value(v), parents })

test("a reference inside a deferral's piece is a child, as tree::refs reads every piece", () => {
  const deferred: SpecNode = {
    kind: "piecewise",
    head: { kind: "flat", value: 0.9 },
    pieces: [{ at: "2026-09-10T00:00:00", term: { kind: "conj", p: -4, terms: [{ kind: "ref", todo: "child" }] } }],
  }
  assert.deepEqual(grouping.refs(deferred), ["child"])
})

test("a cycle is placed, not unrolled", () => {
  const ms = new Map([
    ["aaa", member("home", 0.5, ["bbb"])],
    ["bbb", member("home", 0.5, ["aaa"])],
  ])
  const t = grouping.trie(ms, ["Ancestry"])
  assert.equal(t.kids.length, 2)
  assert.ok(t.kids.every(k => k.kids.length === 1))
})

// The fixture chain, folded by the core in this process: what the page does with its replica.
test("on the fixture chain, a todo's node is its entry's value, and each group is the Conj of its children", () => {
  const chain = JSON.parse(readFileSync("app/core/tests/web/chain.json", "utf8")) as {
    links: { hash: string; literal: string }[]
    untrusted: string[]
  }
  const fold = foldObjects(wasm, chain.links, NOW, chain.untrusted)
  const entries = entriesOf(wasm, chain.links, NOW, chain.untrusted, "life", new wasm.Chains()).entries.filter(e => e.state === "open")
  const ms = grouping.members(entries, fold.specs)
  const link = linker(within(wasm, fold.flatten))
  const price = linked(wasm, link, fold.env, fold.at)
  const t = grouping.grow(ms, classifier.of("category"), price)
  for (const n of tree.nodes(t)) {
    const todo = grouping.place.todo(n.place)
    if (todo !== null) assert.equal(worth.toNumber(n.worth), entries.find(e => e.todo === todo)?.value, todo)
  }
  assert.deepEqual(
    t.kids.map(k => grouping.place.key(k.v.place)).sort(),
    ["c:home", "c:other", "c:work"],
    "the fixture's three categories",
  )
  for (const g of t.kids) {
    assert.deepEqual(
      g.v.term,
      g.kids.length === 1 ? g.kids[0]?.v.term : { kind: "conj", p: -4, terms: g.kids.map(k => k.v.term) },
    )
  }
})

test("a ∅ that refers into a chain not held is not yet priced, through the chain's own references, and nothing else is", () => {
  const functions = {
    a: { kind: "refIn", store: "suzatary", entity: "x" },
    b: { kind: "conj", p: -4, terms: [{ kind: "ref", todo: "a" }, { kind: "flat", value: 0.5 }] },
    c: { kind: "ref", todo: "c" },
    d: { kind: "refIn", store: "life", entity: "y" },
  }
  const waits = grouping.unheld(functions, new Set(["life"]))
  assert.equal(waits({ kind: "ref", todo: "a" }), "suzatary")
  assert.equal(waits({ kind: "ref", todo: "b" }), "suzatary")
  assert.equal(waits({ kind: "ref", todo: "c" }), null, "a loop ends")
  assert.equal(waits({ kind: "ref", todo: "d" }), null, "a held chain reads its own answer")
  assert.equal(grouping.unheld(functions, new Set(["life", "suzatary"]))({ kind: "ref", todo: "a" }), null)
  const { worth } = grouping
  assert.deepEqual(worth.of(() => null, { kind: "ref", todo: "a" }, waits), worth.notYet("suzatary"))
  assert.deepEqual(worth.of(() => 0.4, { kind: "ref", todo: "a" }, waits), worth.value(0.4))
  assert.deepEqual(worth.of(() => null, { kind: "ref", todo: "d" }, waits), worth.absent)
  assert.ok(worth.urgency(worth.notYet("suzatary")) > worth.urgency(worth.absent), "after ∅")
})
