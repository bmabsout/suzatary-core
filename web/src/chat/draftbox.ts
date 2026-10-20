// A composer's box over its draft register: the words in the box, and the draft as the box last saw it. Every
// keystroke is a write resting on that, so words written elsewhere meanwhile stand beside these as a conflict; a
// reading of one value is the draft now, so the box takes it and has seen it, whether or not its words moved.

import register from "../algebra/register"
import type drafts from "./drafts"

export namespace draftbox {
  export type T = { readonly text: string; readonly basis: register.T<string> }

  export const open = (d: drafts.Draft): T => ({ text: register.one(d.read(), ""), basis: d.seen() })

  export const typed = (d: drafts.Draft, b: T, text: string): T => ({ text, basis: d.write(b.basis, text) })

  // Heard the register move: one value (none written reads as the empty draft) is the box's, words and all; a
  // conflict leaves the box as it was.
  export const heard = (d: drafts.Draft, b: T): T => {
    const g = d.read()
    if (g.type === "Conflict") return b
    const v = register.one(g, "")
    // Blank words are written as none, so a box of spaces reading back as the empty draft keeps its spaces.
    return { text: v === (b.text.trim() === "" ? "" : b.text) ? b.text : v, basis: d.seen() }
  }

  // Keeping the box or taking other words is a write that saw every side.
  export const settled = (d: drafts.Draft, text: string): T => ({ text, basis: d.write(d.seen(), text) })
}
export default draftbox
