// A link a card sent and the box did not store, held in this tab's memory alone (no register, no replica, no storage),
// so allowing the origin it was served from stores it with no second paste.

import type { Row } from "./inbox"
import asks from "../prodrome/asks"
import intentsync from "../prodrome/intentsync"
import { toast } from "../view/toast"

export namespace kept {
  type Held = { readonly value: string; readonly done: string }

  const held = new Map<string, Held>()

  export const key = (conversation: string, index: number): string => `${conversation}/${index}`

  export const keep = (k: string, h: Held): void => void held.set(k, h)

  export const drop = (k: string): void => void held.delete(k)

  /** Tap the link's card again with the link it was last tapped with, once; nothing when none is held. */
  export const resend = (r: Row): void => {
    if (r.kind !== "proposal") return
    const k = key(r.conversation.id, r.proposal.index)
    const h = held.get(k)
    if (h === undefined) return
    held.delete(k)
    const at: asks.Proposal = { conversation: r.conversation.id, index: r.proposal.index, from: asks.signature(r.proposal) }
    void intentsync.ask({ type: "Answer", at, answer: { type: "Tap", note: "", always: false } }, h.value).then(o => {
      if (o.type === "Answered" && o.answer.type === "Taken") toast(h.done)
    })
  }
}
export default kept
