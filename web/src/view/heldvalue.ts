// A held value's placeholder as the box writes it into mail text (`suzatary_core::held_value`), and the chip a tap reveals.

import { el } from "./dom"

export namespace heldvalue {
  export type Token =
    | { type: "Text"; text: string }
    | { type: "Held"; kind: string; hint: string; handle: string | null; spelled: string }

  // The value, `null` when the box no longer holds it or answered a replay.
  export type Reveal = (handle: string) => Promise<string | null>

  const PATTERN = /\[(SSN|ITIN|TIN|EIN) ending (\d{0,4}) · (?:tap to show · (held:[A-Za-z0-9_-]+\.\d+)|withheld)\]/g

  export const text = (t: string): Token => ({ type: "Text", text: t })

  export const fold = <R>(t: Token, onText: (text: string) => R, onHeld: (kind: string, hint: string, handle: string | null) => R): R =>
    t.type === "Text" ? onText(t.text) : onHeld(t.kind, t.hint, t.handle)

  export const print = (t: Token): string => (t.type === "Text" ? t.text : t.spelled)

  // Total: the tokens print back to the input.
  export const tokens = (s: string): Token[] => {
    const out: Token[] = []
    let at = 0
    for (const m of s.matchAll(PATTERN)) {
      const start = m.index ?? 0
      if (start > at) out.push(text(s.slice(at, start)))
      out.push({ type: "Held", kind: m[1] ?? "", hint: m[2] ?? "", handle: m[3] ?? null, spelled: m[0] })
      at = start + m[0].length
    }
    return at < s.length ? [...out, text(s.slice(at))] : out
  }

  export const holds = (s: string): boolean => tokens(s).some((t) => t.type === "Held")

  // Dots and the last digits; on a tap the value replaces them in place, kept nowhere but this node.
  export const chip = (kind: string, hint: string, handle: string | null, reveal: Reveal): HTMLElement => {
    const label = el("span", { class: "heldlabel" }, `${kind} ••••${hint}`)
    const box = el("span", { class: "heldvalue", title: handle === null ? `${kind} ending ${hint}, withheld` : `${kind} ending ${hint}` }, label)
    if (handle === null) return box
    const button = el("button", { type: "button", class: "heldshow" }, "Show")
    button.addEventListener("click", () => {
      button.disabled = true
      reveal(handle).then(
        (v) => {
          label.textContent = v ?? `${kind} ••••${hint} (no longer held; open the message again)`
          button.remove()
        },
        () => {
          button.disabled = false
        },
      )
    })
    box.appendChild(button)
    return box
  }

  export const nodes = (s: string, reveal: Reveal): Node[] =>
    tokens(s).map((t) =>
      fold<Node>(t, (x) => document.createTextNode(x), (kind, hint, handle) => chip(kind, hint, handle, reveal)),
    )

  // Every placeholder in `root`'s text, a model's answer or a mail body, drawn as its chip.
  export const within = <N extends Element>(root: N, reveal: Reveal): N => {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT)
    const found: Text[] = []
    for (let n = walker.nextNode(); n !== null; n = walker.nextNode()) if (n instanceof Text && holds(n.data)) found.push(n)
    for (const t of found) t.replaceWith(...nodes(t.data, reveal))
    return root
  }
}
export default heldvalue
