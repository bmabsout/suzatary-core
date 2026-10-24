// A fulfillment in [0, 1]: its state and its colour. The pie that draws it is `view/pie.ts`.

import type { Fulfillment } from "../wire/generated/wire"
import tokens from "../view/tokens"

export namespace fulfillment {
  export type State = { type: "Problem" } | { type: "Watch" } | { type: "Fine" }

  export const problem: State = { type: "Problem" }
  export const watch: State = { type: "Watch" }
  export const fine: State = { type: "Fine" }

  // The number, or null where there is none: `∅`, or a value that does not link.
  export const number = (v: Fulfillment | null): number | null => (typeof v === "number" ? v : null)

  // A value as the screen shows it, the whole percent: the core's `Observation::PERCENT`, `⌊100·v + ½⌋` over [0, 1],
  // so a view of it moves exactly where the core says the next change is.
  export const whole = (v: number): number => Math.floor(Math.min(1, Math.max(0, v)) * 100 + 0.5) / 100

  export const observed = (v: Fulfillment | null): Fulfillment | null => (typeof v === "number" ? whole(v) : v)

  export const WATCH = 0.5
  export const FINE = 0.7

  export const stateOf = (v: number): State => (v < WATCH ? problem : v < FINE ? watch : fine)

  export const fold = <R>(s: State, k: { Problem: R; Watch: R; Fine: R }): R => k[s.type]

  export const rank = (s: State): number => fold(s, { Problem: 0, Watch: 1, Fine: 2 })

  // The worse of two: a monoid with `fine` as its identity.
  export const worst = (...xs: State[]): State => xs.reduce((a, b) => (rank(b) < rank(a) ? b : a), fine)

  // The owner's scale as Typst samples it: linear in each OKLCH channel between the two stops either side of `v`,
  // and the end stop's colour at or past either end.
  export const lch = (v: number): tokens.Oklch => {
    const stops = tokens.fulfillment
    const past = stops.findIndex(s => s.at > v)
    if (past === -1) return (stops[stops.length - 1] ?? stops[0]).colour
    const [a, b] = [stops[past - 1], stops[past]]
    if (a === undefined || b === undefined) return stops[0].colour
    const t = (v - a.at) / (b.at - a.at)
    const at = (k: keyof tokens.Oklch) => a.colour[k] + (b.colour[k] - a.colour[k]) * t
    return { l: at("l"), c: at("c"), h: at("h") }
  }

  export const colour = (v: number): string => {
    const { l, c, h } = lch(v)
    return `oklch(${l.toFixed(3)}% ${c.toFixed(4)} ${h.toFixed(3)})`
  }
}
export default fulfillment
