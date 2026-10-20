// One thing waiting on the owner: a proposal in the inbox prodrome, or an open decision on a chain (a held batch).

import type { ConversationRow, Entry, Fulfillment, InboxProposal, Unheld } from "../wire/generated/wire"
import fulfillment from "../todo/fulfillment"
import { parse } from "../chat/refs"
import type { Reference } from "../chat/refs"
import { isDecision } from "../todo/todos"
import time from "../app/time"
import asker from "./asker"

export namespace waiting {
  // A proposal serving a todo on a chain the reading did not hold has no price yet, which is not `∅`.
  export type Waiting =
    | { readonly type: "Proposal"; readonly p: InboxProposal; readonly unheld: string | null }
    | { readonly type: "Decision"; readonly chain: string; readonly e: Entry }

  // Where its card is: a proposal's conversation, a batch on a chain, or nowhere this app can name.
  export type Where =
    | { readonly type: "Proposal"; readonly conversation: string; readonly index: number }
    | { readonly type: "Batch"; readonly chain: string; readonly head: string }
    | { readonly type: "Unresolved"; readonly problem: string }

  export type ChainFold = { readonly chain: string; readonly entries: readonly Entry[] }

  // A conversation's proposal line the inbox's reading cannot place: it has no card, and says why.
  export type Unplaced = { readonly conversation: string; readonly index: number; readonly why: Unheld }

  export const unplacedOf = (rows: readonly ConversationRow[]): Unplaced[] =>
    rows.flatMap(r => r.unplaced.map(u => ({ conversation: r.id, index: u.index, why: u.why })))

  // Why a proposal line has no card, in the words the box's `Unheld` gives.
  export const unheld = (why: Unheld): string => {
    switch (why) {
      case "notHeld":
        return "the inbox holds no record of it"
      case "unasked":
        return "it is answered and was never asked"
    }
  }

  export const proposal = (p: InboxProposal, unheld: string | null = null): Waiting => ({ type: "Proposal", p, unheld })
  export const decision = (chain: string, e: Entry): Waiting => ({ type: "Decision", chain, e })

  export const fold = <R>(w: Waiting, onProposal: (p: InboxProposal) => R, onDecision: (chain: string, e: Entry) => R): R =>
    w.type === "Proposal" ? onProposal(w.p) : onDecision(w.chain, w.e)

  export const key = (w: Waiting): string => fold(w, (p) => p.id, (_, e) => e.todo)
  export const value = (w: Waiting): Fulfillment | null => fold(w, (p) => p.value, (_, e) => e.value)

  // The chain a proposal's price waits on, or null: priced, whatever its value.
  export const notYetPriced = (w: Waiting): string | null => (w.type === "Proposal" ? w.unheld : null)

  // The chain a `todo:<chain>/<id>` reference names, when `held` does not hold it.
  const unheldIn = (held: readonly string[] | null, serves: string | null): string | null => {
    if (held === null || serves === null) return null
    const r = parse(serves)
    return r !== null && r.kind === "todo" && !held.includes(r.chain) ? r.chain : null
  }
  export const isOpen = (w: Waiting): boolean => fold(w, (p) => p.state.kind === "open", (_, e) => e.state === "open")
  export const state = (w: Waiting): string => fold(w, (p) => p.state.kind, (_, e) => e.state)
  export const settledAt = (w: Waiting): string =>
    fold(w, (p) => (p.state.kind === "completed" || p.state.kind === "cancelled" ? p.state.at : ""), (_, e) => e.at)
  // Why a proposal's state cannot be read, or null: it never waits, since nothing can act on it.
  export const unreadable = (w: Waiting): string | null =>
    w.type === "Proposal" && w.p.state.kind === "unreadable" ? w.p.state.reason : null
  export const source = (w: Waiting): string => fold(w, (p) => asker.named(p.by), (chain) => chain)

  // What it asks, in one plain line: a proposal's headline, a decision's first line of body, markup marks dropped.
  export const asked = (w: Waiting): string =>
    fold(
      w,
      (p) => p.headline,
      (_, e) => ((e.authored?.body ?? "").split("\n")[0] ?? "").replace(/[#*_\\[\]]/g, "").trim() || e.todo,
    )

  const of = (r: Reference | null, named: string): Where => {
    if (r === null) return { type: "Unresolved", problem: "this decision names nothing this app recognises" }
    switch (r.kind) {
      case "proposal":
        return { type: "Proposal", conversation: r.conversation, index: r.index }
      case "batch":
        return { type: "Batch", chain: r.chain, head: r.head }
      case "todo":
      case "mail":
      case "thread":
      case "briefing":
        return { type: "Unresolved", problem: `a decision about ${r.kind}:${named} has no card here` }
    }
  }

  // A decision names its card on `detail`'s first line, a string read back and trusted no further than refs' grammar.
  export const where = (w: Waiting): Where =>
    fold(
      w,
      (p) => of(parse(p.asked), p.id),
      (_, e) => {
        const first = (e.authored?.detail ?? "").split("\n", 1)[0] ?? ""
        return of(first === "" ? null : parse(first), e.todo)
      },
    )

  // Ascending in urgency: low value first, then `∅` and what does not link, then what is not yet priced, ties on the
  // key. A total order.
  const rank = (w: Waiting): readonly [number, number] => {
    if (notYetPriced(w) !== null) return [2, 0]
    const v = fulfillment.number(value(w))
    return v === null ? [1, 0] : [0, v]
  }
  export const compare = (a: Waiting, b: Waiting): number => {
    const [[ga, va], [gb, vb]] = [rank(a), rank(b)]
    return ga !== gb ? ga - gb : va !== vb ? va - vb : key(a).localeCompare(key(b))
  }

  // Everything there is: the inbox's proposals and every chain's decisions. `held` names the chains the proposals were
  // priced through (null: the box's answer, which holds them all).
  export const all = (proposals: readonly InboxProposal[], folds: readonly ChainFold[], held: readonly string[] | null = null): Waiting[] => [
    ...proposals.map((p) => proposal(p, unheldIn(held, p.serves))),
    ...folds.flatMap((f) => f.entries.filter(isDecision).map((e) => decision(f.chain, e))),
  ]

  export const open = (ws: readonly Waiting[]): Waiting[] => ws.filter(isOpen).sort(compare)

  // Settled on the owner's `today` (`YYYY-MM-DD`), most recently settled first.
  const instant = (w: Waiting): number => time.ms(settledAt(w)) ?? -Infinity
  export const settledOn = (ws: readonly Waiting[], today: string, zone: time.Zone): Waiting[] =>
    ws
      .filter((w) => !isOpen(w) && instant(w) !== -Infinity && time.date(zone, instant(w)) === today)
      .sort((a, b) => instant(b) - instant(a))
}

export default waiting
