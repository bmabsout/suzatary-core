// The list as one tree of terms, grouped by classifiers: app/core/src/tree.rs, in the browser.

import type { Block, Entry, Fulfillment, SpecNode } from "../wire/generated/wire"
import fulfillment from "./fulfillment"
import tree from "../algebra/tree"
import lo from "../algebra/lo"
import terms from "./term"

export namespace grouping {
  // A node's value: a number, ∅ (nothing under it has one), ∅ only because it refers into a chain this tab does not hold
  // yet, or why its term does not link.
  export type Worth =
    | { type: "Value"; v: number }
    | { type: "Absent" }
    | { type: "NotYet"; chain: string }
    | { type: "Unlinked"; why: string }

  // Where a worth falls: a number's state, or Unpriced for ∅ — not a problem and not fine, after every priced band.
  export type Band = fulfillment.State | { type: "Unpriced" }

  export namespace bands {
    export const unpriced: Band = { type: "Unpriced" }
    export const fold = <R>(b: Band, k: { Problem: R; Watch: R; Fine: R; Unpriced: R }): R => k[b.type]
    export const rank = (b: Band): number => fold(b, { Problem: 0, Watch: 1, Fine: 2, Unpriced: 3 })
  }

  export namespace worth {
    export const value = (v: number): Worth => ({ type: "Value", v })
    export const absent: Worth = { type: "Absent" }
    export const unlinked = (why: string): Worth => ({ type: "Unlinked", why })
    export const notYet = (chain: string): Worth => ({ type: "NotYet", chain })
    export const fold = <R>(
      w: Worth,
      k: { Value: (v: number) => R; Absent: () => R; NotYet: (chain: string) => R; Unlinked: (why: string) => R },
    ): R =>
      w.type === "Value" ? k.Value(w.v) : w.type === "Absent" ? k.Absent() : w.type === "NotYet" ? k.NotYet(w.chain) : k.Unlinked(w.why)
    export const toNumber = (w: Worth): number | null =>
      fold(w, { Value: v => v, Absent: () => null, NotYet: () => null, Unlinked: () => null })
    // A term that does not link is a problem to fix, so it sorts below every value; ∅ has no number to rank, so after them,
    // and one not yet priced after that, since its number is unknown here rather than none.
    export const urgency = (w: Worth): number =>
      fold(w, { Value: v => v, Absent: () => Number.MAX_VALUE, NotYet: () => Number.POSITIVE_INFINITY, Unlinked: () => -1 })
    export const state = (w: Worth): Band =>
      fold<Band>(w, { Value: fulfillment.stateOf, Absent: () => bands.unpriced, NotYet: () => bands.unpriced, Unlinked: () => fulfillment.problem })
    // The row's own reading: a number, "absent" for ∅, null where it does not link.
    export const ofEntry = (todo: string, f: Fulfillment | null): Worth =>
      f === null ? unlinked(`${todo} does not link`) : f === "absent" ? absent : value(f)
    // `waits` names the chain a ∅ is waiting on, if any.
    export const of = (price: (t: SpecNode) => number | null, t: SpecNode, waits: (t: SpecNode) => string | null = () => null): Worth => {
      try {
        const v = price(t)
        if (v !== null) return value(v)
        const chain = waits(t)
        return chain === null ? absent : notYet(chain)
      } catch (e) {
        return unlinked(lo.reason(e))
      }
    }
  }

  export type Segment =
    | { type: "Category"; name: string }
    | { type: "Band"; band: Band }
    | { type: "Todo"; todo: string }

  export namespace segment {
    export const category = (name: string): Segment => ({ type: "Category", name })
    export const band = (b: Band): Segment => ({ type: "Band", band: b })
    export const todo = (id: string): Segment => ({ type: "Todo", todo: id })
    export const fold = <R>(s: Segment, k: { Category: (n: string) => R; Band: (b: Band) => R; Todo: (t: string) => R }): R =>
      s.type === "Category" ? k.Category(s.name) : s.type === "Band" ? k.Band(s.band) : k.Todo(s.todo)
    export const key = (s: Segment): string => fold(s, { Category: n => `c:${n}`, Band: b => `b:${b.type}`, Todo: t => `t:${t}` })
    const rank = (s: Segment): number => fold(s, { Category: () => 0, Band: () => 1, Todo: () => 2 })
    const within = (s: Segment): string | number =>
      fold<string | number>(s, { Category: n => n, Band: bands.rank, Todo: t => t })
    // The order Rust derives: the variant, then what it carries.
    export const compare = (a: Segment, b: Segment): number => {
      const [x, y] = [within(a), within(b)]
      return rank(a) - rank(b) || (x < y ? -1 : x > y ? 1 : 0)
    }
  }

  // Words over segments with no segment twice in a row: s·s = s, so grouping twice is grouping once.
  export type Path = readonly Segment[]

  export namespace path {
    export const empty: Path = []
    export const one = (s: Segment): Path => [s]
    // Both sides are reduced, so only the seam can repeat.
    export const combine = (a: Path, b: Path): Path => {
      const [last, first] = [a[a.length - 1], b[0]]
      return last !== undefined && first !== undefined && segment.key(last) === segment.key(first) ? [...a, ...b.slice(1)] : [...a, ...b]
    }
    export const key = (p: Path): string => p.map(segment.key).join("/")
  }

  // Every place a todo sits, a set: the unit is {[]} and the combine is pairwise.
  export type Paths = readonly Path[]

  export namespace paths {
    export const empty: Paths = [path.empty]
    const unique = (ps: Path[]): Paths => [...new Map(ps.map(p => [path.key(p), p])).values()]
    export const combine = (a: Paths, b: Paths): Paths => unique(a.flatMap(x => b.map(y => path.combine(x, y))))
  }

  export type By = "Category" | "Urgency" | "Ancestry"
  // The free monoid on `By`; its unit is the flat list.
  export type Classifier = readonly By[]
  export type Grouping = "category" | "urgency" | "flat"

  export namespace classifier {
    export const empty: Classifier = []
    export const combine = (a: Classifier, b: Classifier): Classifier => [...a, ...b]
    export const of = (g: Grouping): Classifier =>
      combine(g === "category" ? ["Category"] : g === "urgency" ? ["Urgency"] : empty, ["Ancestry"])
  }

  export type Member = { category: string; worth: Worth; parents: readonly string[] }
  export type Members = ReadonlyMap<string, Member>

  // `tree::refs`.
  export const refs = (t: SpecNode): string[] =>
    terms.nodes(t).flatMap(n => (n.kind === "ref" && typeof n["todo"] === "string" ? [n["todo"]] : []))

  // `decoration::kind_category`: a record with no category groups by its kind's, which a
  // category kind (`<category>Todo`, `decoration::CATEGORY_KINDS`) names.
  const CATEGORY_KINDS: readonly string[] = ["ekaTodo"]
  export const kindCategory = (b: Block): string =>
    b.kind === "todo" && CATEGORY_KINDS.includes(b.name) ? b.name.slice(0, -"Todo".length) : "other"

  // The todos a tree places, from rows with content that are not decisions; a
  // parent is a member any of whose candidate specs references the child.
  export const members = (entries: readonly Entry[], specs: Readonly<Record<string, readonly SpecNode[]>>): Members => {
    const listed = entries.filter(e => e.authored !== null && e.authored.block.kind !== "decision")
    const ids = new Set(listed.map(e => e.todo))
    const parents = new Map<string, Set<string>>()
    for (const e of listed) {
      for (const c of (specs[e.todo] ?? []).flatMap(refs)) {
        if (ids.has(c)) parents.set(c, (parents.get(c) ?? new Set<string>()).add(e.todo))
      }
    }
    return new Map(
      [...listed]
        .sort((a, b) => (a.todo < b.todo ? -1 : a.todo > b.todo ? 1 : 0))
        .map(e => [
          e.todo,
          {
            category: e.authored?.category || (e.authored === null ? "other" : kindCategory(e.authored.block)),
            worth: worth.ofEntry(e.todo, e.value),
            parents: [...(parents.get(e.todo) ?? [])].sort(),
          },
        ]),
    )
  }

  type Step = readonly [string, Member]

  // Every route from a root down to `m`, root first, excluding it; a parent
  // already on the walk is cut, so a cycle is placed rather than unrolled.
  const routes = (ms: Members, m: Member, walk: readonly string[]): Step[][] => {
    const ps = m.parents.flatMap((p): Step[] => {
      const pm = ms.get(p)
      return pm === undefined || walk.includes(p) ? [] : [[p, pm]]
    })
    return ps.length === 0 ? [[]] : ps.flatMap(([p, pm]) => routes(ms, pm, [...walk, p]).map((r): Step[] => [...r, [p, pm]]))
  }

  // Where a member sits: one path per route to it; keys read the route's root.
  export const placed = (c: Classifier, id: string, m: Member, ms: Members): Paths =>
    paths.combine(
      (c.includes("Ancestry") ? routes(ms, m, [id]) : [[]]).map(route => {
        const root = route[0]?.[1] ?? m
        return c
          .map((by): Path =>
            by === "Category"
              ? path.one(segment.category(root.category))
              : by === "Urgency"
                ? path.one(segment.band(worth.state(root.worth)))
                : route.map(([p]) => segment.todo(p)),
          )
          .reduce(path.combine, path.empty)
      }),
      paths.empty,
    )

  export type Place = { type: "All" } | { type: "At"; at: Segment }

  export namespace place {
    export const all: Place = { type: "All" }
    export const at = (s: Segment): Place => ({ type: "At", at: s })
    export const key = (p: Place): string => (p.type === "All" ? "all" : segment.key(p.at))
    export const todo = (p: Place): string | null => (p.type === "At" && p.at.type === "Todo" ? p.at.todo : null)
  }

  type Trie = Map<string, { s: Segment; below: Trie }>

  const insert = (t: Trie, p: Path): Trie => {
    const [s, ...rest] = p
    if (s === undefined) return t
    const k = segment.key(s)
    const at = t.get(k) ?? { s, below: new Map() }
    t.set(k, { s, below: insert(at.below, rest) })
    return t
  }

  // Every member at its paths then itself, shared by prefix: the free structure on paths.
  export const trie = (ms: Members, c: Classifier): tree.Tree<Place> => {
    const t: Trie = new Map()
    for (const [id, m] of ms) {
      for (const p of paths.combine(placed(c, id, m, ms), [path.one(segment.todo(id))])) insert(t, p)
    }
    const under = (t: Trie): tree.Tree<Place>[] =>
      [...t.values()].sort((a, b) => segment.compare(a.s, b.s)).map(({ s, below }) => tree.of(place.at(s), under(below)))
    return tree.of(place.all, under(t))
  }

  // A todo is `Ref(todo)`, its subtodos in its own function; a group is the
  // `Conj` of its children at the list's p = −4, a group of one being that one.
  export const P = -4
  export const term = (p: Place, kids: terms.Term[]): terms.Term => {
    const t = place.todo(p)
    return t !== null ? { kind: "ref", todo: t } : kids.length === 1 && kids[0] !== undefined ? kids[0] : { kind: "conj", p: P, terms: kids }
  }

  // What a browser holding only the fold can price: a todo's `Ref` is its row's
  // value, and a term that composes several needs the functions it lacks.
  export const known = (entries: readonly Entry[]): { specs: Record<string, readonly SpecNode[]>; price: (t: terms.Term) => number | null } => {
    const values = new Map(entries.flatMap((e): [string, number | null][] => (e.value === null ? [] : [[e.todo, e.value === "absent" ? null : e.value]])))
    return {
      specs: Object.fromEntries(entries.flatMap(e => (e.authored?.spec == null ? [] : [[e.todo, [e.authored.spec]]]))),
      price: t => {
        const v = t.kind === "ref" && typeof t["todo"] === "string" ? values.get(t["todo"]) : undefined
        if (v === undefined) throw new Error("this browser holds none of the functions yet")
        return v
      },
    }
  }

  export type Node = { place: Place; term: terms.Term; worth: Worth }

  // The tree's terms, each linked and evaluated by `price`; children ascend, unlinked first.
  export const grow = (
    ms: Members,
    c: Classifier,
    price: (t: terms.Term) => number | null,
    waits: (t: terms.Term) => string | null = () => null,
  ): tree.Tree<Node> =>
    tree.sort(
      (a, b) => worth.urgency(a.worth) - worth.urgency(b.worth) || 0,
      tree.map(([p, t]): Node => ({ place: p, term: t, worth: worth.of(price, t, waits) }), tree.scan(term, trie(ms, c))),
    )

  // The first chain a term refers into that `held` does not name, following its chain's own references through
  // `functions`: what its ∅ is waiting on. A chain held reads its own answer, whatever it refers to in turn.
  export const unheld = (functions: Readonly<Record<string, terms.Term>>, held: ReadonlySet<string>): ((t: terms.Term) => string | null) => {
    // `seen` is threaded, the todos already followed: a loop ends, and a todo reached twice is walked once.
    type Walked = { found: string | null; seen: ReadonlySet<string> }
    const walk = (t: terms.Term, seen: ReadonlySet<string>): Walked => {
      const [store, todo] = [t["store"], t["todo"]]
      if (t.kind === "refIn" && typeof store === "string" && !held.has(store)) return { found: store, seen }
      if (t.kind === "ref" && typeof todo === "string") {
        const f = functions[todo]
        return seen.has(todo) || f === undefined ? { found: null, seen } : walk(f, new Set([...seen, todo]))
      }
      return terms.children(t).reduce<Walked>((w, c) => (w.found !== null ? w : walk(c, w.seen)), { found: null, seen })
    }
    return t => walk(t, new Set()).found
  }
}
export default grouping
