// One day's briefing, rendered by the box: the secretary's brief block by block, or the saved typst file.

import type { BlockRow } from "../wire/generated/wire"
import type { BriefingPayload } from "../wire/generated/wire"
import { briefing as ask } from "../wire/api"
import http from "../wire/http"
import { answerBlocks, answerCopies, more } from "../chat/transcript"
import type { Child } from "../view/dom"
import { el } from "../view/dom"
import { chevronIcon } from "../view/icons"
import frame from "../view/frame"
import asked from "../app/asked"
import { askLink } from "../chat/refs"
import { richOrText } from "../rich"
import { hrefOf } from "../shell/route"

export namespace briefing {
  export type Day = { readonly type: "written"; readonly payload: BriefingPayload } | { readonly type: "none" }

  export const written = (payload: BriefingPayload): Day => ({ type: "written", payload })
  export const none: Day = { type: "none" }
  export const fold = <R>(alg: { written: (p: BriefingPayload) => R; none: () => R }, d: Day): R =>
    d.type === "written" ? alg.written(d.payload) : alg.none()

  // Which kind the day is, the box decides (a brief wins over a saved file). A 404 is an answer, not a failure: no Retry for it.
  export const day = (date: string, fetch: (date: string) => Promise<BriefingPayload>): Promise<Day> =>
    fetch(date).then(written, (e: unknown) => {
      if (e instanceof http.ApiError && e.status === 404) return none
      throw e
    })

  // The source stays reachable even when the rendering came through.
  const source = (s: string): HTMLElement =>
    el("details", { class: "why" }, el("summary", null, "Source", chevronIcon()), el("pre", { class: "briefingsource" }, s))

  const savedView = (date: string, s: string, rich: string | null): Child => [
    el("div", { class: "askrow" }, askLink({ kind: "briefing", date })),
    richOrText(rich, s, { class: "todobody" }),
    el("p", { class: "legend" }, rich === null ? "Shown as typst markup source. Nothing has rendered it." : "Rendered by typst."),
    source(s),
  ]

  const briefView = (conversation: string, text: string, blocks: readonly BlockRow[]): Child => [
    answerBlocks({ text, blocks }),
    more(answerCopies({ text, blocks })),
    el(
      "p",
      { class: "legend" },
      el("a", { href: hrefOf({ type: "conversation", id: conversation, refs: [] }) }, "Reply in the conversation"),
      ", where its proposals are.",
    ),
  ]

  export const view = (d: Day): Child =>
    fold<Child>(
      {
        written: p =>
          p.day.kind === "brief" ? briefView(p.day.conversation, p.day.text, p.day.blocks) : savedView(p.date, p.day.source, p.day.rich),
        none: () => el("p", { class: "legend" }, "No briefing was written for this day."),
      },
      d,
    )

  const days = asked.family((date: string) => date, (date: string) => day(date, ask))

  export const page = (date: string): frame.Page<Day> => ({
    className: "briefingpage",
    title: date,
    back: { label: "Briefings", fallback: { type: "briefings" } },
    reads: () => days(date),
    view,
  })

  export const mount = (root: HTMLElement, date: string) => frame.mount(root, page(date))
}
export default briefing
