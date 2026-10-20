// The Todos page's state: what it knows, and the one step every event moves it by.

import type overview from "./overview"
import type { SpecNode } from "../wire/generated/wire"
import grouping from "../todo/grouping"
import fulfillment from "../todo/fulfillment"
import questions from "../prodrome/questions"
import tree from "../algebra/tree"
import onscreen from "../app/store"
import type { Filter } from "../todo/todos"
import { matches } from "../todo/todos"
import type row from "../todo/row"
import type { Unsent } from "../prodrome/replica"

export namespace todosState {
  export type Briefing = { type: "Unknown" } | { type: "None" } | { type: "Today"; date: string }
  /** Whether setup is finished, as the box's overview reads: unknown until it answers. */
  export type Setup = { type: "Unknown" } | overview.Progress

  // What prices the tree. The box's rows at once, each todo's `Ref` its row's value, so the page draws from the fold it
  // has; then the functions and environment the core in its worker folded at `at` from the objects the tab holds,
  // named by the reading the worker keeps.
  export type Basis =
    | { type: "Rows" }
    | {
        type: "Folded"
        reading: questions.Name
        at: string
        specs: Readonly<Record<string, readonly SpecNode[]>>
        waits: (t: SpecNode) => string | null
      }

  // What names a basis: the box's rows at their instant, or a reading the worker keeps.
  export type Of = { readonly type: "Rows"; readonly at: string } | { readonly type: "Fold"; readonly reading: questions.Name }
  export const sameOf = (a: Of, b: Of): boolean =>
    a.type === "Rows" ? b.type === "Rows" && a.at === b.at : b.type === "Fold" && questions.same(a.reading, b.reading)

  // The prices the worker answered under one basis, by each term's print.
  export type Prices = { readonly of: Of; readonly by: ReadonlyMap<string, questions.Priced> }

  // What the tree asks the worker for: nothing; the terms its basis has no price of, not yet asked; or those terms,
  // asked and not yet answered, so the page asks once for them.
  export type Wants =
    | { readonly type: "None" }
    | { readonly type: "Wanting"; readonly of: Of; readonly basis: questions.Basis; readonly terms: readonly SpecNode[] }
    | { readonly type: "Asking"; readonly of: Of; readonly basis: questions.Basis; readonly terms: readonly SpecNode[] }

  // What went wrong, and whether a reading failed, so the next reading's basis clears it; a problem about no reading
  // stands until the store moves.
  export type Problem = { readonly reason: string; readonly ofReading: boolean }

  // Whether the search, the filters and the grouping stand open under the title: shut until asked for.
  export type Find = { type: "Shut" } | { type: "Open" }
  export const shut: Find = { type: "Shut" }
  export const opened: Find = { type: "Open" }

  // A share waiting to go into the add sheet: `n` counts the shares, so the same text shared twice is two.
  export type Share = { readonly body: string; readonly n: number }

  export type T = {
    readonly store: onscreen.Fold
    readonly filter: Filter
    readonly by: grouping.Grouping
    readonly find: Find
    readonly folds: row.Folds
    readonly basis: Basis
    readonly prices: Prices
    // The last tree every term of which was priced, kept while the worker prices the next.
    readonly grown: tree.Tree<grouping.Node> | null
    readonly wants: Wants
    readonly selected: string
    readonly opened: string
    readonly cursor: string
    readonly share: Share
    readonly briefing: Briefing
    readonly setup: Setup
    readonly problem: Problem | null
    readonly waiting: number
    // Every todo whose last act this tab holds and the box has not taken, and where that act stands.
    readonly pending: ReadonlyMap<string, Unsent>
    readonly poke: number
  }

  export type Event =
    | { type: "Store"; state: onscreen.Fold }
    | { type: "Filter"; filter: Filter }
    | { type: "By"; by: grouping.Grouping }
    | { type: "Find"; find: Find }
    | { type: "ToggleFind" }
    | { type: "Fold"; key: string; open: boolean }
    | { type: "Basis"; basis: Basis }
    | { type: "Asked"; of: Of; terms: readonly SpecNode[] }
    | { type: "Priced"; of: Of; terms: readonly SpecNode[]; prices: readonly questions.Priced[] }
    | { type: "Select"; id: string }
    | { type: "Open"; id: string }
    | { type: "Cursor"; key: string }
    | { type: "Shared"; body: string }
    | { type: "Briefing"; briefing: Briefing }
    | { type: "Setup"; setup: Setup }
    | { type: "Problem"; reason: string | null; ofReading: boolean }
    | { type: "Waiting"; count: number }
    | { type: "Pending"; todos: ReadonlyMap<string, Unsent> }
    | { type: "Poke" }

  const key = (t: SpecNode): string => JSON.stringify(t)

  // What a basis prices and how: its name, the question that prices a term, and the terms it prices with no question.
  const pricing = (s: Omit<T, "grown" | "wants">): { of: Of; basis: questions.Basis; specs: Readonly<Record<string, readonly SpecNode[]>>; local: (t: SpecNode) => questions.Priced | undefined; waits: (t: SpecNode) => string | null } => {
    if (s.basis.type === "Folded") return { of: { type: "Fold", reading: s.basis.reading }, basis: { type: "Fold", reading: s.basis.reading }, specs: s.basis.specs, local: () => undefined, waits: s.basis.waits }
    const entries = onscreen.entries(s.store)
    const at = onscreen.at(s.store) ?? ""
    const values = new Map(entries.flatMap((e): [string, number | null][] => (e.value === null ? [] : [[e.todo, e.value === "absent" ? null : e.value]])))
    const functions = Object.fromEntries([...values].map(([id, v]): [string, SpecNode] => [id, v === null ? { kind: "absent" } : { kind: "flat", value: v }]))
    return {
      of: { type: "Rows", at },
      basis: { type: "Rows", at, functions },
      specs: grouping.known(entries).specs,
      // A todo is its row's value; a group composes its rows' values, which the core prices.
      local: t => {
        if (t.kind !== "ref" || typeof t["todo"] !== "string") return undefined
        const v = values.get(t["todo"])
        return v === undefined ? { type: "Refused", why: "this browser holds none of the functions yet" } : { type: "Value", v }
      },
      waits: () => null,
    }
  }

  const NONE: Prices = { of: { type: "Rows", at: "" }, by: new Map() }
  const NOTHING: Wants = { type: "None" }
  const termsKey = (ts: readonly SpecNode[]): string => ts.map(key).join(";")

  // The tree the page draws: the kept entries, placed and priced; or, while a term is not priced yet, the terms to ask
  // for, with the tree drawn before kept. A todo this tab acted on and the box has not taken stays where it was,
  // whatever the filter, so its row can say where the act stands.
  export const grow = (s: Omit<T, "grown" | "wants">, was: tree.Tree<grouping.Node> | null): { grown: tree.Tree<grouping.Node> | null; wants: Wants } => {
    if (!onscreen.loaded(s.store)) return { grown: was, wants: NOTHING }
    const p = pricing(s)
    const ms = grouping.members(onscreen.entries(s.store).filter(e => matches(e, s.filter, onscreen.at(s.store) ?? "") || s.pending.has(e.todo)), p.specs)
    const c = grouping.classifier.of(s.by)
    const by = sameOf(s.prices.of, p.of) ? s.prices.by : NONE.by
    const priced = (t: SpecNode): questions.Priced | undefined => p.local(t) ?? by.get(key(t))
    const terms = tree.nodes(tree.scan(grouping.term, grouping.trie(ms, c))).map(([, t]) => t)
    const lacking = [...new Map(terms.filter(t => priced(t) === undefined).map(t => [key(t), t])).values()]
    if (lacking.length > 0) return { grown: was, wants: { type: "Wanting", of: p.of, basis: p.basis, terms: lacking } }
    const price = (t: SpecNode): number | null => {
      const q = priced(t)
      if (q === undefined || q.type === "Refused") throw new Error(q?.why ?? "not priced")
      return q.v === null ? null : fulfillment.whole(q.v)
    }
    return { grown: grouping.grow(ms, c, price, p.waits), wants: NOTHING }
  }

  // The same terms under the same basis, asked already, stay asked.
  const sameWant = (a: Wants, b: Wants): boolean =>
    a.type !== "None" && b.type !== "None" && sameOf(a.of, b.of) && termsKey(a.terms) === termsKey(b.terms)

  // The tree grown again from the state; a want already asked stays asked.
  export const regrown = (s: T): T => {
    const g = grow(s, s.grown)
    return { ...s, grown: g.grown, wants: s.wants.type === "Asking" && sameWant(s.wants, g.wants) ? s.wants : g.wants }
  }

  // The folds the list draws: a card holding a line whose act is on its way stays open, so the line can say so, even
  // when the act made its value Fine; a fold the viewer set still wins.
  export const shownFolds = (s: T): row.Folds => {
    if (s.grown === null || s.pending.size === 0) return s.folds
    const sending = (t: tree.Tree<grouping.Node>): boolean => tree.nodes(t).some(n => s.pending.has(grouping.place.todo(n.place) ?? ""))
    const held = [s.grown, ...s.grown.kids].filter(sending).map((t): [string, boolean] => [grouping.place.key(t.v.place), true])
    return { ...Object.fromEntries(held), ...s.folds }
  }

  // The page's one effect: what the tree wants, asked of the worker once (`Asked` first, so the next value does not ask
  // again), and the answer an event. A failed ask prices every term it asked as refused, so the rows still draw with
  // the groups unlinked, and says why.
  export const ask = (v: T, price: (basis: questions.Basis, terms: readonly SpecNode[]) => Promise<readonly questions.Priced[]>, dispatch: (e: Event) => void): void => {
    if (v.wants.type !== "Wanting") return
    const { of, basis, terms } = v.wants
    dispatch({ type: "Asked", of, terms })
    void price(basis, terms).then(
      prices => dispatch({ type: "Priced", of, terms, prices }),
      (e: unknown) => {
        const why = e instanceof Error ? e.message : String(e)
        dispatch({ type: "Priced", of, terms, prices: terms.map(() => ({ type: "Refused", why })) })
        dispatch({ type: "Problem", reason: why, ofReading: true })
      },
    )
  }

  export const initial = (
    store: onscreen.Fold,
    filter: Filter,
    by: grouping.Grouping,
    folds: row.Folds,
    selected: string,
    prefill: string,
  ): T => {
    const s: Omit<T, "grown" | "wants"> = {
      store,
      filter,
      by,
      find: shut,
      folds,
      basis: { type: "Rows" },
      prices: NONE,
      selected,
      opened: "",
      cursor: "",
      share: { body: prefill, n: prefill === "" ? 0 : 1 },
      briefing: { type: "Unknown" },
      setup: { type: "Unknown" },
      problem: null,
      waiting: 0,
      pending: new Map(),
      poke: 0,
    }
    return { ...s, ...grow(s, null) }
  }

  export const step = (s: T, e: Event): T => {
    switch (e.type) {
      case "Store":
        return regrown({ ...s, store: e.state, problem: null })
      case "Filter":
        return regrown({ ...s, filter: e.filter })
      case "By":
        return s.by === e.by ? s : regrown({ ...s, by: e.by })
      case "Find":
        return s.find.type === e.find.type ? s : { ...s, find: e.find }
      case "ToggleFind":
        return { ...s, find: s.find.type === "Shut" ? opened : shut }
      case "Basis":
        return regrown({ ...s, basis: e.basis, problem: e.basis.type === "Folded" && s.problem?.ofReading === true ? null : s.problem })
      case "Asked":
        return s.wants.type === "Wanting" && sameWant(s.wants, { ...s.wants, of: e.of, terms: e.terms }) ? { ...s, wants: { ...s.wants, type: "Asking" } } : s
      // An answer about a basis since replaced is dropped; one about this one joins what it priced before.
      case "Priced": {
        if (s.wants.type === "None" || !sameOf(e.of, s.wants.of)) return s
        const by = new Map(sameOf(s.prices.of, e.of) ? s.prices.by : NONE.by)
        e.terms.forEach((t, i) => {
          const p = e.prices[i]
          if (p !== undefined) by.set(key(t), p)
        })
        return regrown({ ...s, prices: { of: e.of, by } })
      }
      case "Fold":
        return s.folds[e.key] === e.open ? s : { ...s, folds: { ...s.folds, [e.key]: e.open } }
      case "Select":
        return s.selected === e.id ? s : { ...s, selected: e.id }
      case "Open":
        return s.opened === e.id ? s : { ...s, opened: e.id }
      case "Cursor":
        return s.cursor === e.key ? s : { ...s, cursor: e.key }
      case "Shared":
        return e.body === "" ? s : { ...s, share: { body: e.body, n: s.share.n + 1 } }
      case "Briefing":
        return { ...s, briefing: e.briefing }
      case "Setup":
        return { ...s, setup: e.setup }
      case "Problem":
        return (e.reason === null ? s.problem === null : s.problem?.reason === e.reason && s.problem.ofReading === e.ofReading)
          ? s
          : { ...s, problem: e.reason === null ? null : { reason: e.reason, ofReading: e.ofReading } }
      case "Waiting":
        return s.waiting === e.count ? s : { ...s, waiting: e.count }
      case "Pending":
        return regrown({ ...s, pending: e.todos })
      case "Poke":
        return { ...s, poke: s.poke + 1 }
    }
  }
}
export default todosState
