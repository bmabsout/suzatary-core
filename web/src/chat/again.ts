/** Asking the last round again: what the box offers, and the composer's mode. */

import type { TurnRow } from "../wire/generated/wire"
import type { SourceState } from "./source"

export namespace again {
  export type Offer =
    | { type: "None" }
    | { type: "Last"; question: TurnRow }

  export const none: Offer = { type: "None" }

  export const fold = <R>(o: Offer, onNone: () => R, onLast: (question: TurnRow) => R): R =>
    o.type === "None" ? onNone() : onLast(o.question)

  // The box names the question; the page only finds it in the window it holds, and offers nothing while a round runs.
  export const offer = (s: SourceState): Offer => {
    if (s.kind !== "loaded" || s.conversationState !== "idle" || s.again === null) return none
    const question = s.turns.find(t => t.index === s.again)
    return question === undefined ? none : { type: "Last", question }
  }

  export type Mode =
    | { type: "Asking" }
    | { type: "Editing"; question: TurnRow }

  export const asking: Mode = { type: "Asking" }

  export type Event =
    | { type: "edit"; offer: Offer }
    | { type: "cancel" }
    | { type: "sent" }

  export const step = (m: Mode, e: Event): Mode => {
    switch (e.type) {
      case "edit":
        return fold<Mode>(e.offer, () => m, question => ({ type: "Editing", question }))
      case "cancel":
      case "sent":
        return m.type === "Asking" ? m : asking
    }
  }
}
export default again
