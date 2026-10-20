// About: what the app claims about itself, the archive, and the running version.

import type { AboutPayload } from "../wire/generated/wire"
import { about as ask, getReleases } from "../wire/api"
import releases from "./releases"
import type { Child } from "../view/dom"
import { el } from "../view/dom"
import frame from "../view/frame"
import asked from "../app/asked"
import reading from "../algebra/reading"
import { hrefOf } from "../shell/route"

export namespace about {
  const prose = (...cs: Child[]) => el("p", { class: "aboutprose" }, ...cs)

  // The claim is the system's argument about itself; it has a page, not a footer.
  export const claims = (): Child => [
    el("h2", null, "One evaluator, on two machines"),
    prose(
      "Everything on these pages is folded from the event chain by the server — and this browser runs the same core (prodrome, compiled to WebAssembly), so it can check that for itself: it verifies every chain object, it samples each todo's thirty days itself, and when the network is gone it folds the objects it is holding rather than showing you yesterday's numbers. One evaluator, on two machines.",
    ),
    prose(
      "The browser computes no fulfillment of its own: every number on screen is one the core produced, on the box or in this tab. Low means urgent, everywhere a number appears.",
    ),
    prose(
      "Every number can be explained — a todo's page has \"Why this number\" — and every claim about the chain can be checked from the ",
      el("a", { href: hrefOf({ type: "chain", focus: "" }) }, "chain page"),
      ", which rehashes every object in this tab.",
    ),
    el("h2", null, "Archive"),
    prose(
      "The morning brief is a secretary conversation since 11 September 2026. The briefings saved before that are still here: ",
      el("a", { href: hrefOf({ type: "briefings" }) }, "saved briefings"),
      ".",
    ),
    el("h2", null, "Version"),
  ]

  // What is running on the box, which only the box can say — never this bundle's own idea.
  export const version = (p: AboutPayload): Child =>
    el(
      "p",
      { class: "aboutprose aboutversion" },
      el("strong", null, `Suzatary ${p.version}`),
      p.build === "dev" ? ", a development build outside the flake." : `, built from revision ${p.build}.`,
    )

  // The version, then the releases the project chain reads, which say where the next one stands.
  type T = { readonly running: AboutPayload; readonly releases: releases.Read }

  // What the box is running, asked once and again when the box moves.
  const running = asked.question(ask, (a, b) => a.version === b.version && a.build === b.build)

  // A box that serves no project chain draws no releases; one that cannot read it says so, and loses nothing else.
  const planned = asked.question(() => getReleases().then(releases.read, releases.failed))

  export const page: frame.Page<T> = {
    className: "aboutpage",
    title: "About",
    now: claims,
    reads: () => reading.map(reading.both(running, planned), ([r, p]): T => ({ running: r, releases: p })),
    view: v => [version(v.running), releases.view(v.releases)],
  }

  export const mount = (root: HTMLElement) => frame.mount(root, page)
}
export default about
