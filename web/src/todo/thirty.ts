// A todo's thirty days, sampled by the core from the objects this tab holds, or why there is no curve.

import type { Chains, Core, Todos } from "../prodrome/core"
import { foldOf, linkedKnots, linker, linkerOf } from "../prodrome/core"
import curve from "./curve"
import time from "../app/time"

export namespace thirty {
  export type Thirty =
    | { type: "Sampling" }
    // No object this tab holds names the todo: the replica has not taken it yet.
    | { type: "Unheld" }
    // Held, but nothing believed by now gives it a spec or a checklist, so it has no function.
    | { type: "Unpriced" }
    | { type: "Drawn"; span: curve.Span }

  export const sampling: Thirty = { type: "Sampling" }
  export const unheld: Thirty = { type: "Unheld" }
  export const unpriced: Thirty = { type: "Unpriced" }
  export const drawn = (span: curve.Span): Thirty => ({ type: "Drawn", span })

  export const fold = <R>(
    a: Thirty,
    k: { Sampling: () => R; Unheld: () => R; Unpriced: () => R; Drawn: (s: curve.Span) => R },
  ): R =>
    a.type === "Drawn" ? k.Drawn(a.span) : a.type === "Unpriced" ? k.Unpriced() : a.type === "Unheld" ? k.Unheld() : k.Sampling()

  // Folded at the moment to the millisecond: a second-truncated now would not yet believe what the box authored earlier in that second.
  // Linked through `chains`, the set the row is priced through, so a reference into another chain draws as the row reads.
  export const sample = (core: Core, chains: Chains, todos: Todos, untrusted: readonly string[], id: string, ms: number): Thirty => {
    const f = foldOf(todos, time.moment(ms), untrusted)
    if (!(id in f.flatten)) return id in f.stream ? unpriced : unheld
    const w = curve.thirtyDays(time.parse(f.at))
    const set = linkerOf(chains, f.functions, f.at)
    try {
      const link = linker(t => set.link(t))
      const knots = (from: string, to: string) => linkedKnots(core, link, f.history, from, to)({ kind: "ref", todo: id }).knots
      return drawn(curve.ofKnots(w, [...knots(time.instant(w.from), f.at).filter(([t]) => time.parse(t) < w.now), ...knots(f.at, time.instant(w.to))]))
    } finally {
      set.free()
    }
  }
}
export default thirty
