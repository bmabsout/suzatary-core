// A term's JSON as the tree it prints: every child term a field holds, a row's (a Piecewise's piece) included.

import type { SpecNode } from "../wire/generated/wire"
import d from "../wire/decoder"
import payload from "../wire/generated/payload"

export namespace term {
  export type Term = SpecNode

  const isTerm = d.is(payload.SpecNode)

  // Read structurally, never by constructor, so a new constructor's child is never missed: a field is a term, a list
  // of them, or a row (a kindless object, `Slot::Rows`) whose own fields hold them.
  const within = (v: unknown): Term[] =>
    isTerm(v) ? [v] : Array.isArray(v) ? v.flatMap(within) : typeof v === "object" && v !== null ? Object.values(v).flatMap(within) : []

  // `TermF::children`.
  export const children = (t: Term): Term[] => Object.entries(t).flatMap(([k, v]) => (k === "kind" ? [] : within(v)))

  export const cata = <R>(alg: (t: Term, rs: R[]) => R, t: Term): R => alg(t, children(t).map(c => cata(alg, c)))

  // Every node, the root first.
  export const nodes = (t: Term): Term[] => cata((x, rs: Term[][]) => [x, ...rs.flat()], t)

  // The first node, root first, `f` answers for.
  export const find = <R>(f: (t: Term) => R | null, t: Term): R | null => f(t) ?? children(t).reduce<R | null>((r, c) => r ?? find(f, c), null)
}

export default term
