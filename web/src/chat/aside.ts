// The owner's rule for the secretary beside a wide page: it opens by itself only on a page about one thing (a todo, a
// mail, a thread, a briefing) when the conversation holds talk about that thing. A page of many things (Todos, Inbox,
// Settings, Mail, Calendar) keeps it closed until the toggle is pressed. A press, open or close, holds for that page
// alone, in this tab; nothing about the panel is kept on the device.

import app from "../app/app"
import nav from "../shell/nav"
import { referenceInView } from "./inview"
import { print, type Reference } from "./refs"
import type { SourceState } from "./source"

export namespace aside {
  // A page of many things is its destination, so a calendar's next week or a mail search is still the same page.
  export type Page =
    | { readonly type: "Many"; readonly key: string }
    | { readonly type: "One"; readonly key: string; readonly reference: Reference }

  /** Beside the page at 62rem and up, nowhere below it, and never beside the secretary's own pages. */
  export type Placement = { readonly type: "Absent" } | { readonly type: "Beside"; readonly page: Page }

  export type Talk = "Quiet" | "Talking"

  /** What a press left on a page. */
  export type Press = "open" | "closed"

  /** This tab's presses, by page. */
  export type Presses = { readonly [key: string]: Press }

  export const placement = (a: app.App): Placement => {
    const at = nav.activeFor(app.route(a), a.chain)
    if (!a.wide || at === "secretary") return { type: "Absent" }
    const r = referenceInView(app.screen(a), a.chain)
    return { type: "Beside", page: r.type === "one" ? { type: "One", key: print(r.reference), reference: r.reference } : { type: "Many", key: at } }
  }

  // The whole conversation drawn because nothing in it is about the page (`fallback`) is not talk about the page.
  export const talk = (s: SourceState): Talk => (s.kind === "loaded" && !s.fallback && s.turns.length > 0 ? "Talking" : "Quiet")

  export const shown = (page: Page, presses: Presses, talk: Talk): boolean => {
    const said: Press | "unpressed" = presses[page.key] ?? "unpressed"
    switch (said) {
      case "open":
        return true
      case "closed":
        return false
      case "unpressed":
        return page.type === "One" && talk === "Talking"
    }
  }

  export const pressed = (page: Page, presses: Presses, talk: Talk): Presses => ({
    ...presses,
    [page.key]: shown(page, presses, talk) ? "closed" : "open",
  })
}
export default aside
