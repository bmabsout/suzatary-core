// A todo's reading part: the box's Typst page when this tab has one, else the entry's plain text.

import type { Entry, TodoPayload } from "../wire/generated/wire"
import { el } from "../view/dom"
import { chip, worded } from "../view/format"
import { chevronIcon } from "../view/icons"
import { elements } from "../rich"

export namespace reading {
  export type Reading =
    | { type: "Page"; html: string }
    // No page: never fetched, or typst did not render it; drawn from the entry.
    | { type: "Plain"; entry: Entry }

  export const page = (html: string): Reading => ({ type: "Page", html })
  export const plain = (entry: Entry): Reading => ({ type: "Plain", entry })
  export const of = (p: TodoPayload): Reading => (p.reading === null ? plain(p.entry) : page(p.reading))
  export const fold = <R>(r: Reading, k: { Page: (html: string) => R; Plain: (e: Entry) => R }): R =>
    r.type === "Page" ? k.Page(r.html) : k.Plain(r.entry)

  // Where the page came from: the box now, this tab's last fetch, or only the fold.
  export type Got =
    | { type: "Fetched"; payload: TodoPayload }
    | { type: "Held"; payload: TodoPayload; why: string }
    | { type: "Unfetched"; entry: Entry; why: string }

  export const foldGot = <R>(
    g: Got,
    k: { Fetched: (p: TodoPayload) => R; Held: (p: TodoPayload, why: string) => R; Unfetched: (e: Entry, why: string) => R },
  ): R => (g.type === "Fetched" ? k.Fetched(g.payload) : g.type === "Held" ? k.Held(g.payload, g.why) : k.Unfetched(g.entry, g.why))

  export const entryOf = (g: Got): Entry => foldGot(g, { Fetched: p => p.entry, Held: p => p.entry, Unfetched: e => e })
  export const readingOf = (g: Got): Reading => foldGot(g, { Fetched: of, Held: of, Unfetched: e => plain(e) })
  export const payloadOf = (g: Got): TodoPayload | null => foldGot(g, { Fetched: p => p, Held: p => p, Unfetched: () => null })

  // The tab's cache: the last payload per todo, newest last, at most `MAX`. A map is insertion-ordered, so re-keeping moves one to the end.
  export type Held = ReadonlyMap<string, TodoPayload>
  export const MAX = 64
  export const none: Held = new Map()
  export const keep = (h: Held, p: TodoPayload): Held => {
    const next = new Map([...h].filter(([k]) => k !== p.todo))
    next.set(p.todo, p)
    return next.size > MAX ? new Map([...next].slice(next.size - MAX)) : next
  }

  // A failed fetch, answered from what the tab holds; null when it holds nothing about this todo.
  export const fallback = (h: Held, entries: readonly Entry[], id: string, why: string): Got | null => {
    const p = h.get(id)
    if (p !== undefined) return { type: "Held", payload: p, why }
    const e = entries.find(x => x.todo === id)
    return e === undefined ? null : { type: "Unfetched", entry: e, why }
  }

  // What a plain header says of its body, the same words the box's header says: an unconfirmed one is marked.
  export const marks = (e: Entry): { chips: readonly string[]; note: string } =>
    e.unconfirmed
      ? { chips: ["unconfirmed"], note: "Shown the way it was written until you confirm it." }
      : { chips: [], note: "Plain text: this tab has no rendered page for this todo yet." }

  const plainHead = (e: Entry): HTMLElement => {
    const m = marks(e)
    return el(
      "div",
      { class: "todohead" },
      el("div", { class: "todofulfil" }, el("span", { class: "todopct" }, worded(e.value))),
      e.authored === null ? null : el("div", { class: "todolead" }, el("div", { class: "todobody assource" }, e.authored.body)),
      el("code", { class: "todoid" }, e.todo),
      m.chips.length === 0 ? null : el("div", { class: "cardmeta" }, m.chips.map(c => chip(c, "prov"))),
      el("p", { class: "legend" }, m.note),
    )
  }

  const plainRest = (e: Entry): HTMLElement[] =>
    e.authored === null || e.authored.detail === ""
      ? []
      : [el("details", { class: "why", open: true }, el("summary", null, "Detail", chevronIcon()), el("div", { class: "todobody assource" }, e.authored.detail))]

  // The header, and everything under it; the page puts its acts between the two.
  export const view = (r: Reading): { head: Node[]; rest: Node[] } =>
    fold(r, {
      Page: html => {
        const [head, ...rest] = elements(html)
        return { head: head === undefined ? [] : [head], rest }
      },
      Plain: e => ({ head: [plainHead(e)], rest: plainRest(e) }),
    })
}
export default reading
