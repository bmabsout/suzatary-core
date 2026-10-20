// A proposal's stage as the wire says it: whether the owner may still answer it, and what became of it.

import type { ProposalRow, ProposalState } from "../wire/generated/wire"

export namespace stage {
  // Dismiss is drawn exactly while this holds; one that may have gone is completed, never answered again.
  export const answerable = (s: ProposalState): boolean => s === "standing" || s === "failed"

  // Open means it waits: answerable, an act's flow that ends itself, or a send that may have gone until he has checked Sent.
  export const open = (s: ProposalState): boolean => answerable(s) || s === "started" || s === "unknown"

  export const tried = (n: number): string => (n === 0 ? "" : n === 1 ? "tried once" : `tried ${n} times`)

  /** What became of it, in a sentence; "" while it stands. */
  export const said = (p: ProposalRow): string => {
    const again = p.tries > 1 ? ` (${tried(p.tries)})` : ""
    switch (p.state) {
      case "standing":
        return ""
      case "sent":
        return `Sent to ${p.outcome}${again}`
      case "applied":
        return `Applied: ${p.outcome}${again}`
      case "started":
        return `Running: ${p.outcome}`
      case "acted":
        return `Done: ${p.outcome}${again}`
      case "failed":
        return `Not sent, ${tried(p.tries)}: ${p.outcome}`
      case "unknown":
        return `Failed, ${tried(p.tries)}: ${p.outcome}. ${p.card.uncertain}`
      case "checked":
        return `Failed, ${tried(p.tries)}: ${p.outcome}. You checked Sent; it is never sent again.`
      case "dismissed":
        return `Dismissed: ${p.outcome}`
      // Never the owner's words: a superseded proposal is housekeeping, and `outcome` is not read.
      case "superseded":
        return "Replaced by a later draft."
      // Taken back by the box or the secretary, never the owner's no; `outcome` says why.
      case "withdrawn":
        return `Withdrawn: ${p.outcome}`
      // Its responses read as no state; `outcome` says why, and nothing acts on it.
      case "unreadable":
        return `Cannot be read: ${p.outcome}`
    }
  }
}

export default stage
