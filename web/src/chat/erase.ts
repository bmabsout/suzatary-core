// Erasing a turn's words, the owner's tap: a second press says what it does, the ask is an intent, and the turn is
// drawn erased once the replica holds the box's erasure, never before.

import type { TurnRow } from "../wire/generated/wire"
import asks from "../prodrome/asks"
import intents from "../prodrome/intents"
import intentsync from "../prodrome/intentsync"
import cardui from "../inbox/cardui"
import { el } from "../view/dom"
import placeholder from "../view/placeholder"
import { loadCore } from "../prodrome/core"

export namespace erase {
  // The box's sentence for what an erasure does, from the module this tab runs; none where it does not load.
  export const warning = (): Promise<string> => loadCore().then(c => c.erase_warning(), () => "")

  export const ask = (conversation: string, turn: TurnRow): asks.Ask => ({ type: "Erase", conversation, turns: [turn.index] })

  const ui = (conversation: string, turn: TurnRow): cardui.Ui => cardui.of(cardui.key("erase", `${conversation}/${turn.index}`))

  export const shown = (conversation: string, turn: TurnRow, asked: intentsync.T): intents.Shown =>
    intents.about(asked, a => intents.key(a) === intents.key(ask(conversation, turn)), () => turn.gone !== null)

  // What the control draws from: its step and its intent, so a redraw of the turn is a change of either.
  export const sig = (conversation: string, turn: TurnRow, asked: intentsync.T): string =>
    `${cardui.print(ui(conversation, turn).get())}|${intents.key(shown(conversation, turn, asked))}`

  // Whether the control stands untouched: not asking to confirm, and no erasure asked, so whatever holds it may stay closed.
  export const atRest = (conversation: string, turn: TurnRow, asked: intentsync.T): boolean =>
    !cardui.is(ui(conversation, turn).get(), "Confirming", "erase") && shown(conversation, turn, asked).type === "Absent"

  // `warning` is the box's own sentence for what an erasure does, read from the core this tab runs.
  export const view = (conversation: string, turn: TurnRow, asked: intentsync.T, warning: Promise<string>): HTMLElement => {
    const u = ui(conversation, turn)
    const row = el("div", { class: "turnerase" })
    const draw = (): void => {
      const now = shown(conversation, turn, asked)
      row.replaceChildren(
        ...intents.fold<Node[]>(now, {
          Pending: () => [placeholder.inline()],
          Taken: () => [placeholder.inline()],
          Failed: f => [el("span", { class: "problem" }, f.e), ...idle()],
          Absent: () => (cardui.is(u.get(), "Confirming", "erase") ? confirming() : idle()),
        }),
      )
    }
    const go = (e: cardui.Event): void => (u.go(e), draw())
    const idle = (): Node[] => [
      el("button", { type: "button", class: "act act-erase", onclick: () => go({ type: "Opened", step: { type: "Confirming", verb: "erase" } }) }, "Erase"),
    ]
    const confirming = (): Node[] => [
      said(el("span", { class: "proposalconfirm" }, "Erase these words for good?")),
      el(
        "button",
        { type: "button", class: "act act-erase confirming", onclick: () => (u.go({ type: "Closed" }), void intentsync.ask(ask(conversation, turn))) },
        "Erase",
      ),
      el("button", { type: "button", class: "act act-keep", onclick: () => go({ type: "Closed" }) }, "Keep"),
    ]
    const said = (span: HTMLElement): HTMLElement => (void warning.then(w => span.append(` ${w}`)), span)
    draw()
    return row
  }
}
export default erase
