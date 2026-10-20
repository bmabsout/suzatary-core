/** A draft per conversation: an `Owner` register, so the words follow the owner, and two writers' meet as a conflict. */

import type register from "../algebra/register"
import registers from "../app/registers"
import type ui from "../app/ui"

export namespace drafts {
  // `write` rests on what its writer saw (`seen`) and answers what it sees after; blank is nothing typed.
  export type Draft = {
    readonly read: () => register.Reading<string>
    readonly seen: () => register.T<string>
    readonly write: (seen: register.T<string>, text: string) => register.T<string>
    readonly hear: (f: () => void) => () => void
  }

  export const of = (u: ui.Ui, id: string): Draft => {
    const c = u.cell(registers.draft, id)
    return { read: c.read, seen: c.seen, write: (seen, text) => c.write(seen, text.trim() === "" ? "" : text), hear: c.hear }
  }
}
export default drafts
