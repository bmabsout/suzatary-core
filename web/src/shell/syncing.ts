// The bar under the header while a store syncs: for each store, the bytes come of the bytes its sync needs.

import { el } from "../view/dom"
import progress from "../prodrome/progress"
import stores from "../prodrome/stores"

export namespace syncing {
  export type Line = { readonly name: stores.Name; readonly done: number; readonly needed: number }

  export const lines = (p: progress.T): readonly Line[] =>
    stores.NAMES.flatMap(name => {
      const b = progress.bar(p[name])
      return b.type === "Hidden" ? [] : [{ name, done: b.done, needed: b.needed }]
    })

  const line = (l: Line): Node =>
    el("div", { class: "syncline" }, el("span", null, progress.said(l.name, l.done, l.needed)), el("progress", { max: String(l.needed), value: String(l.done) }))

  export const install = (bar: HTMLElement): ((p: progress.T) => void) => p => {
    const shown = lines(p)
    bar.hidden = shown.length === 0
    bar.replaceChildren(...shown.map(line))
  }
}
export default syncing
