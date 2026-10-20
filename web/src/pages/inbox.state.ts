// The Inbox page's value: the open rows resolved to cards, today's settled ones, the ticks, the open diffs and the row a link opened.
// A new reading keeps what is about the screen (the ticks, the open diffs, the settled section, the row the address opened).

import type { ProposedObject } from "../wire/generated/wire"
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
  // Whether the box has answered this session for the batches held on its chains; until it has, an empty list is no answer.
  export type Box = "Asking" | "Answered"
  export type T = {
    readonly rows: readonly Row[]
    readonly settled: readonly waiting.Waiting[]
    readonly unreadable: readonly waiting.Waiting[]
    // Proposal lines with no card, each drawn with why.
    readonly unplaced: readonly waiting.Unplaced[]
    readonly picks: ReadonlyMap<string, ReadonlySet<string>>
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
    | { readonly type: "Toggled"; readonly head: string; readonly hash: string; readonly on: boolean }
    | { readonly type: "Settled"; readonly head: string }
    | { readonly type: "Expanded"; readonly key: string; readonly open: boolean }
    | { readonly type: "SettledOpen"; readonly open: boolean }
    | { readonly type: "Scrolled" }
    | { readonly type: "Writes"; readonly me: string | null; readonly writes: boolean }
    | { readonly type: "Asked"; readonly intents: intents.T<asks.Ask> }

  // Ticking takes everything the row rests on, unticking drops everything resting on it, from `needs` alone.
  export const closure = (objects: readonly ProposedObject[], picked: ReadonlySet<string>, hash: string, on: boolean): Set<string> => {
    const next = new Set(picked)
    const needs = new Map(objects.map(o => [o.hash, o.needs]))
    if (on) {
      const pending = [hash]
      for (let name = pending.pop(); name !== undefined; name = pending.pop()) {
        if (next.has(name)) continue
        next.add(name)
        pending.push(...(needs.get(name) ?? []))
      }
      return next
    }
    next.delete(hash)
    for (let moved = true; moved; ) {
      moved = false
      for (const o of objects) {
        if (next.has(o.hash) && !o.needs.every(n => next.has(n))) {
          next.delete(o.hash)
          moved = true
        }
      }
    }
    return next
  }

  // An untouched batch reads as what its decision reviewed.
  export const pickedFor = (
    picks: ReadonlyMap<string, ReadonlySet<string>>,
    row: { readonly batch: { readonly head: string }; readonly reviewed: ReadonlySet<string> },
  ): ReadonlySet<string> => picks.get(row.batch.head) ?? row.reviewed

  const batchRow = (rows: readonly Row[], head: string): (Row & { kind: "batch" }) | undefined =>
    rows.find((r): r is Row & { kind: "batch" } => r.kind === "batch" && r.batch.head === head)

  const withPicks = (v: T, head: string, next: ReadonlySet<string> | null): T => {
    const picks = new Map(v.picks)
    if (next === null) picks.delete(head)
    else picks.set(head, next)
    return { ...v, picks }
  }

  // An event naming a head no row carries changes nothing.
  export const step = (v: T, e: Event): T => {
    switch (e.type) {
      case "Toggled": {
        const r = batchRow(v.rows, e.head)
        return r === undefined ? v : withPicks(v, e.head, closure(r.batch.objects, pickedFor(v.picks, r), e.hash, e.on))
      }
      case "Settled":
        return v.picks.has(e.head) ? withPicks(v, e.head, null) : v
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
      picks: was.picks,
      expanded: was.expanded,
      settledOpen: was.settledOpen,
      // The address names the row it opened, so it stays marked while the page reads; it is scrolled to once.
      opening: was.opening,
      intents: intents.join(was.intents, now.intents),
    })
}
export default inboxState
