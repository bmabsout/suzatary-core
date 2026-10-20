// A drawing placed in keyed DOM: an element per key, kept whole while its name holds, refilled while its own
// content holds, made afresh otherwise; children moved by the fewest moves, so scroll stays where it was.

import { reconcile } from "../algebra/reconcile"
import type draw from "./draw"

export namespace place {
  // What placed an element, read back on the next placement; nothing else is kept beside the DOM.
  type Placed = { readonly key: string; readonly name: string; readonly own: string }
  const placed = new WeakMap<Element, Placed>()

  export type Counts = { readonly kept: number; readonly refilled: number; readonly made: number; readonly moved: number }
  const none: Counts = { kept: 0, refilled: 0, made: 0, moved: 0 }
  const plus = (a: Counts, b: Counts): Counts => ({ kept: a.kept + b.kept, refilled: a.refilled + b.refilled, made: a.made + b.made, moved: a.moved + b.moved })

  type Drawing = Extract<draw.Drawn, { readonly type: "Element" }>

  // The nodes that draw an element, a run read through to its children.
  const drawing = (ds: readonly draw.Drawn[]): Drawing[] => ds.flatMap(d => (d.type === "Run" ? drawing(d.kids) : [d]))

  const mark = (e: Element, x: Drawing): void => void placed.set(e, { key: x.key, name: x.name, own: x.own })

  const made = (x: Drawing): [Element, Counts] => {
    const e = x.el.cloneNode(true)
    if (!(e instanceof Element)) throw new Error("an element cloned into something else")
    mark(e, x)
    return [e, plus({ ...none, made: 1 }, fill(e, x.kids))]
  }

  // `parent`'s placed children made to be `ds`, by key; what it holds unplaced (its own content) stays first.
  const fill = (parent: Element, ds: readonly draw.Drawn[]): Counts => {
    const want = drawing(ds)
    const live = new Map<string, Element>()
    for (const e of parent.children) {
      const p = placed.get(e)
      if (p !== undefined) live.set(p.key, e)
    }
    const now = new Map<string, Element>()
    let counts = none
    for (const x of want) {
      const l = live.get(x.key)
      const p = l === undefined ? undefined : placed.get(l)
      if (l !== undefined && p?.name === x.name) {
        now.set(x.key, l)
        counts = plus(counts, { ...none, kept: 1 })
      } else if (l !== undefined && p?.own === x.own) {
        mark(l, x)
        now.set(x.key, l)
        counts = plus(counts, plus({ ...none, refilled: 1 }, fill(l, x.kids)))
      } else {
        const [e, c] = made(x)
        l?.replaceWith(e)
        now.set(x.key, e)
        counts = plus(counts, c)
      }
    }
    for (const op of reconcile([...live.keys()], want.map(x => x.key))) {
      if (op.op === "remove") live.get(op.key)?.remove()
      else {
        const e = now.get(op.key)
        if (e !== undefined) parent.insertBefore(e, op.before === null ? null : (now.get(op.before) ?? null))
        counts = plus(counts, { ...none, moved: 1 })
      }
    }
    return counts
  }

  // Focus is kept by what it does: a control replaced under it hands it to the control with the same intent.
  export const into = (host: Element, d: draw.Drawn): Counts => {
    const active = document.activeElement
    const intent = active !== null && host.contains(active) ? (active.closest("[data-intent]")?.getAttribute("data-intent") ?? null) : null
    const counts = fill(host, [d])
    if (intent !== null && active !== null && !active.isConnected) {
      const again = [...host.querySelectorAll("[data-intent]")].find(e => e.getAttribute("data-intent") === intent)
      if (again instanceof HTMLElement) again.focus({ preventScroll: true })
    }
    return counts
  }
}
export default place
