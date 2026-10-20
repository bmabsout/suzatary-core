// The first device's way in: the code `suzatary device pair` prints at the box, typed here, trusts this device's key.

import { el } from "../view/dom"
import { toast } from "../view/toast"
import cardui from "../inbox/cardui"

export namespace pairing {
  // `pair` sends the code as typed; the box reads it, so a wrong one is refused in the box's words.
  export type Acts = { readonly pair: (code: string) => Promise<void> }

  export const LABEL = "Enter the code from the box"

  // The register the field keeps its words in: one per device, so Settings and this device's own card share it.
  export const KEY = cardui.key("pairing", "here")

  // Sends through `send`; a trust closes the field and says so, a refusal is the box's words, and the field stays.
  export const acts = (send: (code: string) => Promise<unknown>, done: () => void): Acts => ({
    pair: code =>
      send(code).then(
        () => {
          cardui.of(KEY).go({ type: "Closed" })
          toast("This device is trusted.")
          done()
        },
        (e: unknown) => toast(e instanceof Error ? e.message : String(e)),
      ),
  })

  // Closed, the button that opens it; opened, a field for the code, the button that sends it and Cancel. The words typed
  // live in the card's register, so a redraw of the page finds them.
  export const view = (ui: cardui.Ui, acts: Acts): Node => {
    const go = (e: cardui.Event): void => {
      ui.go(e)
      node.replaceChildren(...drawn())
    }
    const opened = (): Node[] => {
      const input = cardui.field(ui, () =>
        el("input", { type: "text", class: "paircode", autocomplete: "off", autocapitalize: "characters", spellcheck: "false", placeholder: "BCDF-GHJK", "aria-label": "The code from the box" }),
      )
      const send = el("button", { type: "button", class: "act act-paircode", disabled: ui.get().text.trim() === "" }, "Trust this device")
      input.addEventListener("input", () => void (send.disabled = input.value.trim() === ""))
      send.addEventListener("click", () => {
        send.disabled = true
        void acts.pair(input.value).finally(() => void (send.disabled = input.value.trim() === ""))
      })
      return [
        el("p", { class: "settingsmeta" }, "At the box, run suzatary device pair, then type the code it prints. It works once, for 10 minutes."),
        input,
        send,
        el("button", { type: "button", class: "act act-cancelpair", onclick: () => go({ type: "Closed" }) }, "Cancel"),
      ]
    }
    const drawn = (): Node[] =>
      cardui.is(ui.get(), "Confirming", "pair")
        ? opened()
        : [el("button", { type: "button", class: "act act-pair", onclick: () => go({ type: "Opened", step: { type: "Confirming", verb: "pair" } }) }, LABEL)]
    const node = el("div", { class: "pairing" }, drawn())
    return node
  }
}
export default pairing
