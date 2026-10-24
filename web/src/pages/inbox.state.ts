// The Inbox page's value: the open rows resolved to cards, today's settled ones, the open folds and the row a link opened.
// A new reading keeps what is about the screen (the open folds, the settled section, the row the address opened).

import type { Row } from "../inbox/inbox"
import waiting from "../inbox/waiting"
import asker from "../inbox/asker"
import type spoken from "../inbox/spoken"
import intents from "../prodrome/intents"
import type asks from "../prodrome/asks"

export namespace inboxState {
  // The row a link opened: not yet reached, its Setup row unfolded for it, then scrolled to, once.
  export type Stage = "Pending" | "Unfolded" | "Scrolled"
  export type Opening = { readonly key: string; readonly stage: Stage } | null
  // Whether the box has answered this session for its chains' folds; until it has, an empty list is no answer.
  export type Box = "Asking" | "Answered"
  export type T = {
    readonly rows: readonly Row[]
    readonly settled: readonly waiting.Settled[]
    readonly unreadable: readonly waiting.Waiting[]
    // Proposal lines with no card, each drawn with why.
    readonly unplaced: readonly waiting.Unplaced[]
    readonly expanded: ReadonlySet<string>
    readonly settledOpen: boolean
    readonly opening: Opening
    readonly me: string | null
    readonly writes: boolean
    readonly problem: string | null
    readonly box: Box
    // What this tab asked and the replicas do not hold yet: each card is drawn against it.
    readonly intents: intents.T<asks.Ask>
    // Each todo's title as the page read it, so a card names its sources in words.
    readonly titles: spoken.Titles
  }
  export type Event =
    | { readonly type: "Expanded"; readonly key: string; readonly open: boolean }
    | { readonly type: "SettledOpen"; readonly open: boolean }
    | { readonly type: "Scrolled" }
    | { readonly type: "Writes"; readonly me: string | null; readonly writes: boolean }
    | { readonly type: "Asked"; readonly intents: intents.T<asks.Ask> }

  export const step = (v: T, e: Event): T => {
    switch (e.type) {
      case "Expanded":
        return v.expanded.has(e.key) === e.open ? v
        : { ...v, expanded: e.open ? new Set([...v.expanded, e.key]) : new Set([...v.expanded].filter(k => k !== e.key)) }
      case "SettledOpen":
        return v.settledOpen === e.open ? v : { ...v, settledOpen: e.open }
      case "Scrolled":
        return v.opening === null || v.opening.stage === "Scrolled" ? v : { ...v, opening: { ...v.opening, stage: "Scrolled" } }
      case "Writes":
        return v.me === e.me && v.writes === e.writes ? v : { ...v, me: e.me, writes: e.writes }
      case "Asked":
        return v.intents === e.intents ? v : { ...v, intents: intents.join(v.intents, e.intents) }
    }
  }

  // The Setup row's key among the open folds.
  export const SETUP = "setup"

  // A link that names a setup card unfolds the Setup row once, the first time a reading holds that card; from then on
  // whether the row is open is the reader's alone.
  export const opened = (v: T): T =>
    v.opening === null || v.opening.stage !== "Pending"
    || !v.rows.some(r => r.kind === "proposal" && asker.isSetup(r.proposal.card.by) && waiting.key(r.w) === v.opening?.key)
      ? v
      : { ...v, expanded: new Set([...v.expanded, SETUP]), opening: { ...v.opening, stage: "Unfolded" } }

  // A new reading keeps what is about the screen.
  export const keeps = (was: T, now: T): T => opened({
      ...now,
      expanded: was.expanded,
      settledOpen: was.settledOpen,
      // The address names the row it opened, so it stays marked while the page reads; it is scrolled to once.
      opening: was.opening,
      intents: intents.join(was.intents, now.intents),
    })
}
export default inboxState
