// A card's own state: which step its controls are on, the words being typed there, and whether its details show, one register per card,
// keyed by the card's identity, so a redraw of the page, or a page drawn again from nothing, finds it where it was.
// A secret's value is never here: it stays in its field and leaves in the tap.

import ui from "../app/ui"
import registers from "../app/registers"
import disclosure from "../view/disclosure"

export namespace cardui {
  // `Confirming` names the answer whose question is asked; `Saying` the answer that needs a reason first.
  export type Step = { readonly type: "Idle" } | { readonly type: "Confirming"; readonly verb: string } | { readonly type: "Saying"; readonly verb: string }
  // A card is drawn with its details hidden; asking a yes shows them, so what is confirmed is on screen.
  export type Details = "Hidden" | "Shown"
  export type T = { readonly step: Step; readonly text: string; readonly details: Details }
  export type Event =
    | { readonly type: "Opened"; readonly step: Step }
    | { readonly type: "Typed"; readonly text: string }
    | { readonly type: "Closed" }
    | { readonly type: "Details"; readonly details: Details }

  export const IDLE: T = registers.card.initial

  // Opening a step starts its words afresh, and a question shows the details; typing keeps the step; closing is idle
  // with the details as they were.
  export const step = (t: T, e: Event): T =>
    e.type === "Opened" ? { step: e.step, text: "", details: e.step.type === "Confirming" ? "Shown" : t.details }
    : e.type === "Typed" ? (t.text === e.text ? t : { ...t, text: e.text })
    : e.type === "Details" ? (t.details === e.details ? t : { ...t, details: e.details })
    : { ...IDLE, details: t.details }


  export const key = (kind: string, id: string): string => `${kind}/${id}`

  export type Ui = { readonly get: () => T; readonly go: (e: Event) => void }

  // A card's register stepped by its events: what a redraw reads and a control writes.
  export const of = (k: string): Ui => {
    const r = ui.cell(registers.card, k)
    return { get: r.get, go: e => r.set(step(r.get(), e)) }
  }

  // Whether the card's details show, as a disclosure reads it.
  export const details = (u: Ui): disclosure.Open => ({ get: u.get().details === "Shown", set: open => u.go({ type: "Details", details: open ? "Shown" : "Hidden" }) })

  // A field bound to a card's words: it starts from them, writes each keystroke back, and, drawn again while the
  // keyboard had nowhere to go (its predecessor taken out of the page), takes it back at the end of the words.
  export const field = (ui: Ui, make: () => HTMLInputElement): HTMLInputElement => {
    const f = make()
    f.value = ui.get().text
    f.addEventListener("input", () => ui.go({ type: "Typed", text: f.value }))
    queueMicrotask(() => {
      if (!f.isConnected || (document.activeElement !== null && document.activeElement !== document.body)) return
      f.focus()
      f.setSelectionRange(f.value.length, f.value.length)
    })
    return f
  }

  // One line of a register: its step, its verb, and how many characters are typed (never the words).
  export const print = (t: T): string => `${t.step.type}${t.step.type === "Idle" ? "" : `:${t.step.verb}`}:${t.text.length}:${t.details}`

  export const is = (t: T, type: Step["type"], verb: string): boolean => t.step.type === type && t.step.type !== "Idle" && t.step.verb === verb
}
export default cardui
