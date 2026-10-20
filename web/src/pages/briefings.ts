// Briefings: the secretary's morning briefs, newest first, and the files saved before them.

import type { BriefingsPayload } from "../wire/generated/wire"
import { briefings as ask } from "../wire/api"
import empty from "../view/empty"
import type { Child } from "../view/dom"
import { el } from "../view/dom"
import frame from "../view/frame"
import placeholder from "../view/placeholder"
import asked from "../app/asked"
import { hrefOf } from "../shell/route"

export namespace briefings {
  export type Row = { readonly date: string; readonly words: number; readonly title: string }

  export const row = (r: Row): HTMLElement =>
    el(
      "a",
      { class: "briefingrow", href: hrefOf({ type: "briefing", date: r.date }) },
      el(
        "div",
        { class: "briefingmeta" },
        el("span", { class: "briefingdate" }, r.date),
        el("span", null, `${r.words} words`),
      ),
      el("div", { class: "briefingtitle" }, r.title),
    )

  export const view = (p: BriefingsPayload): Child => [
    el(
      "div",
      { class: "briefinglist" },
      p.briefs.length === 0 ? empty.note("No morning brief has been written yet.") : p.briefs.map(row),
    ),
    p.briefings.length === 0
      ? null
      : [el("h2", null, "Saved before 11 September 2026"), el("div", { class: "briefinglist" }, p.briefings.map(row))],
  ]

  const sameDays = (a: readonly Row[], b: readonly Row[]): boolean =>
    a.length === b.length && a.every((r, i) => r.date === b[i]?.date && r.words === b[i]?.words && r.title === b[i]?.title)

  // Read by the list too, for today's brief: one register, asked once for both.
  export const listed = asked.question(ask, (a, b) => sameDays(a.briefs, b.briefs) && sameDays(a.briefings, b.briefings))

  export const page: frame.Page<BriefingsPayload> = {
    className: "briefingspage",
    title: "Briefings",
    shape: placeholder.ROWS,
    masthead: "destination",
    legend: () => "The secretary's morning brief, every day at 07:03, newest first.",
    reads: () => listed,
    view,
  }

  export const mount = (root: HTMLElement) => frame.mount(root, page)
}
export default briefings
