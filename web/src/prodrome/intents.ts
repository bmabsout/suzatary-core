// What a page asked and the history does not yet hold: a set that only grows, retracted by subsumption.

export namespace intents {
  // What the box said to one asking; a lost answer is no answer, and asking again is asking once.
  export type Answer =
    | { readonly type: "Taken" }
    | { readonly type: "Refused"; readonly e: string }
    | { readonly type: "Lost"; readonly e: string }

  export type Asked<A> = { readonly act: A; readonly asks: number; readonly answer: Answer | null }

  // Two grow-only sets, a 2P-set: what was asked, named by its print, and what the history has since subsumed.
  export type T<A> = {
    readonly asked: ReadonlyMap<string, Asked<A>>
    readonly retracted: ReadonlySet<string>
  }

  export type Event<A> =
    | { readonly type: "Asked"; readonly key: string; readonly act: A }
    | { readonly type: "Answered"; readonly key: string; readonly asks: number; readonly answer: Answer }
    | { readonly type: "Subsumed"; readonly key: string }

  // What a card draws of one intent: nothing (none, or the history holds it), waiting, taken and not yet read back, or failed.
  export type Shown =
    | { readonly type: "Absent" }
    | { readonly type: "Pending" }
    | { readonly type: "Taken" }
    | { readonly type: "Failed"; readonly e: string; readonly lost: boolean }

  export const EMPTY: T<never> = { asked: new Map(), retracted: new Set() }

  // The canonical print: keys sorted at every level, so two equal acts are one key however they were spelled.
  export const key = (a: unknown): string => JSON.stringify(a, (_, v: unknown) =>
    v !== null && typeof v === "object" && !Array.isArray(v)
      ? Object.fromEntries(Object.entries(v).sort(([x], [y]) => (x < y ? -1 : x > y ? 1 : 0)))
      : v)

  const RANK: { readonly [K in Answer["type"]]: number } = { Lost: 0, Refused: 1, Taken: 2 }

  // The order an asking climbs: a later ask, then an answer over none, then a firmer answer.
  const above = <A>(a: Asked<A>, b: Asked<A>): Asked<A> =>
    a.asks !== b.asks ? (a.asks > b.asks ? a : b)
    : a.answer === null ? b
    : b.answer === null ? a
    : RANK[a.answer.type] !== RANK[b.answer.type] ? (RANK[a.answer.type] > RANK[b.answer.type] ? a : b)
    : key(a.answer) >= key(b.answer) ? a : b

  export const isIn = <A>(t: T<A>, k: string): boolean => t.asked.has(k) && !t.retracted.has(k)

  // An intent is sent when asked anew or after a failure; one pending, taken or subsumed is already asked.
  export const wants = <A>(t: T<A>, k: string): boolean => {
    if (t.retracted.has(k)) return false
    const a = t.asked.get(k)
    return a === undefined || (a.answer !== null && a.answer.type !== "Taken")
  }

  // Inflationary: every event leaves `t` or climbs above it, so no order of events can un-ask or un-retract.
  export const step = <A>(t: T<A>, e: Event<A>): T<A> => {
    if (e.type === "Subsumed") return !t.asked.has(e.key) || t.retracted.has(e.key) ? t : { ...t, retracted: new Set([...t.retracted, e.key]) }
    const was = t.asked.get(e.key)
    if (e.type === "Asked") {
      if (!wants(t, e.key)) return t
      return { ...t, asked: new Map([...t.asked, [e.key, { act: e.act, asks: (was?.asks ?? 0) + 1, answer: null }]]) }
    }
    if (was === undefined || was.asks !== e.asks || was.answer !== null) return t
    return { ...t, asked: new Map([...t.asked, [e.key, { ...was, answer: e.answer }]]) }
  }

  // The join of two sets of intents, `EMPTY` its unit.
  export const join = <A>(a: T<A>, b: T<A>): T<A> => {
    const asked = new Map(a.asked)
    for (const [k, v] of b.asked) {
      const w = asked.get(k)
      asked.set(k, w === undefined ? v : above(w, v))
    }
    return { asked, retracted: new Set([...a.retracted, ...b.retracted]) }
  }

  // Retract every intent the history holds; a function of the set and the reading, never of when it is asked.
  export const settle = <A>(t: T<A>, holds: (a: A) => boolean): T<A> =>
    [...t.asked].reduce((u, [k, v]) => (isIn(u, k) && holds(v.act) ? step(u, { type: "Subsumed", key: k }) : u), t)

  export const pending = <A>(t: T<A>): readonly A[] => [...t.asked].filter(([k]) => !t.retracted.has(k)).map(([, v]) => v.act)

  // The history first: an intent it holds is drawn by the history alone.
  export const shown = <A>(t: T<A>, k: string, holds: boolean): Shown => {
    const a = t.asked.get(k)
    return a === undefined || holds || t.retracted.has(k) ? { type: "Absent" }
      : a.answer === null ? { type: "Pending" }
      : a.answer.type === "Taken" ? { type: "Taken" }
      : { type: "Failed", e: a.answer.e, lost: a.answer.type === "Lost" }
  }

  // What a card draws of the intents about it: the first still open, else nothing.
  export const about = <A>(t: T<A>, matches: (a: A) => boolean, holds: (a: A) => boolean): Shown =>
    [...t.asked].filter(([, v]) => matches(v.act)).map(([k, v]) => shown(t, k, holds(v.act))).find(s => s.type !== "Absent") ?? { type: "Absent" }

  export const fold = <R>(s: Shown, on: { readonly [K in Shown["type"]]: (s: Extract<Shown, { type: K }>) => R }): R =>
    s.type === "Absent" ? on.Absent(s) : s.type === "Pending" ? on.Pending(s) : s.type === "Taken" ? on.Taken(s) : on.Failed(s)
}
export default intents
