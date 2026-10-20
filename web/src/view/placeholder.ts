// What a page shows while what it reads has not arrived: the loading mark over grey lines in the shape of its
// content, with no words. The stylesheet shows it 300 ms after it is placed, so a shorter wait shows nothing.

import { el } from "./dom"
import loadmark from "./loadmark"

export namespace placeholder {
  export type Shape =
    | { readonly type: "Lines" }
    | { readonly type: "Rows" }
    | { readonly type: "Cards" }
    | { readonly type: "Turns" }

  export const LINES: Shape = { type: "Lines" }
  export const ROWS: Shape = { type: "Rows" }
  export const CARDS: Shape = { type: "Cards" }
  export const TURNS: Shape = { type: "Turns" }

  export const fold = <R>(s: Shape, on: { lines: () => R; rows: () => R; cards: () => R; turns: () => R }): R => {
    switch (s.type) {
      case "Lines": return on.lines()
      case "Rows": return on.rows()
      case "Cards": return on.cards()
      case "Turns": return on.turns()
    }
  }

  // The stylesheet sizes each bar by its place, so a shape is only how many there are and how they group.
  const bars = (n: number): HTMLElement[] => Array.from({ length: n }, () => el("span", { class: "phbar" }))
  const each = (n: number, cls: string, inner: number): HTMLElement[] => Array.from({ length: n }, () => el("span", { class: cls }, bars(inner)))

  const parts = (s: Shape): HTMLElement[] =>
    fold(s, {
      lines: () => bars(5),
      rows: () => each(5, "phrow", 2),
      cards: () => each(3, "phcard", 3),
      turns: () => each(4, "phturn", 2),
    })

  export const view = (s: Shape): HTMLElement =>
    el("div", { class: `placeholder ph-${s.type.toLowerCase()}`, role: "status", "aria-label": "Loading" }, loadmark.view(), parts(s))

  // The mark alone, beside a control waiting on the box, shown at once: the person pressed it, so the wait is theirs.
  export const inline = (): HTMLElement => el("span", { class: "placeholder phinline", role: "status", "aria-label": "Waiting" }, loadmark.view())
}
export default placeholder
