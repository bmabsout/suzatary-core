// What the composer holds: the box's draft, the mode it is asked in, the files on their way and the ones held back.

import type { AttachmentRow, TurnRow } from "../wire/generated/wire"
import type { Reference } from "./refs"
import { parsed, print } from "./refs"
import again from "./again"

export namespace composing {
  // The words, the chips and the files already kept on the box.
  export type Draft = { readonly text: string; readonly references: readonly Reference[]; readonly files: readonly AttachmentRow[] }
  // A file the box held back until the owner says to keep it private in his Drive.
  export type Withheld = { readonly base64: string; readonly name: string; readonly why: string; readonly folder: string }
  export type Held =
    | { readonly type: "Asking"; readonly w: Withheld }
    | { readonly type: "Keeping"; readonly w: Withheld }
  export type T = {
    readonly draft: Draft
    readonly mode: again.Mode
    // The picks still on their way to the box, a photo redrawn first, each by the id it was picked with.
    readonly uploading: ReadonlySet<number>
    readonly held: readonly Held[]
  }
  // A draft on its way, in the mode it was asked in; a failed send hands it back.
  export type Sent = { readonly draft: Draft; readonly mode: again.Mode }

  export type Event =
    | { readonly type: "Typed"; readonly text: string }
    | { readonly type: "Quoted"; readonly text: string }
    | { readonly type: "Attached"; readonly reference: Reference }
    | { readonly type: "Detached"; readonly reference: Reference }
    | { readonly type: "Picked"; readonly id: number }
    | { readonly type: "Filed"; readonly id: number; readonly file: AttachmentRow }
    | { readonly type: "Withheld"; readonly id: number; readonly w: Withheld }
    | { readonly type: "Lost"; readonly id: number }
    | { readonly type: "Unfiled"; readonly id: string }
    | { readonly type: "Keeping"; readonly w: Withheld }
    | { readonly type: "Unkept"; readonly w: Withheld }
    | { readonly type: "Kept"; readonly w: Withheld; readonly file: AttachmentRow }
    | { readonly type: "Dropped"; readonly w: Withheld }
    | { readonly type: "Edit"; readonly question: TurnRow }
    | { readonly type: "Cancel" }
    | { readonly type: "Send" }
    | { readonly type: "Failed"; readonly sent: Sent }

  export const empty: Draft = { text: "", references: [], files: [] }
  export const isEmpty = (d: Draft): boolean => d.text === "" && d.references.length === 0 && d.files.length === 0
  export const of = (text: string, references: readonly Reference[]): T =>
    ({ draft: { ...empty, text, references: dedupe(references) }, mode: again.asking, uploading: new Set(), held: [] })

  // Whether Send sends: a blank box sends nothing, and a file on its way or held back is answered first.
  export type Sending =
    | { readonly type: "Blank" }
    | { readonly type: "Uploading" }
    | { readonly type: "Held" }
    | { readonly type: "Ready"; readonly sent: Sent }

  export const sending = (s: T): Sending =>
    s.draft.text.trim() === "" && s.draft.files.length === 0 ? { type: "Blank" }
    : s.uploading.size > 0 ? { type: "Uploading" }
    : s.held.length > 0 ? { type: "Held" }
    : { type: "Ready", sent: { draft: s.draft, mode: s.mode } }

  export const fold = <R>(x: Sending, on: { readonly [K in Sending["type"]]: (x: Extract<Sending, { type: K }>) => R }): R => {
    switch (x.type) {
      case "Blank":
        return on.Blank(x)
      case "Uploading":
        return on.Uploading(x)
      case "Held":
        return on.Held(x)
      case "Ready":
        return on.Ready(x)
    }
  }

  // References are a set by their spelling, files by their id, each in the order first attached.
  const dedupe = (rs: readonly Reference[]): readonly Reference[] =>
    rs.filter((r, i) => rs.findIndex(x => print(x) === print(r)) === i)
  const filed = (fs: readonly AttachmentRow[], f: AttachmentRow): readonly AttachmentRow[] =>
    fs.some(x => x.id === f.id) ? fs : [...fs, f]

  const put = (s: T, draft: Partial<Draft>): T => ({ ...s, draft: { ...s.draft, ...draft } })
  const landed = (s: T, id: number): T => ({ ...s, uploading: new Set([...s.uploading].filter(u => u !== id)) })
  const unheld = (s: T, w: Withheld): T => ({ ...s, held: s.held.filter(h => h.w !== w) })
  const asked = (s: T, w: Withheld, type: Held["type"]): T => ({ ...s, held: s.held.map(h => (h.w === w ? { type, w } : h)) })
  // The chips go with the question, not the box, and so do the files held back.
  const emptied = (s: T, mode: again.Mode): T => ({ ...s, draft: empty, mode, held: [] })

  const quoted = (text: string, said: string): string => {
    const cut = said.length > 200 ? `${said.slice(0, 200)}…` : said
    return `${text === "" ? "" : `${text}\n\n`}> ${cut.replace(/\n/g, "\n> ")}\n\n`
  }

  // The box empties when the draft leaves, not when the box answers: what is typed meanwhile is the next draft, and a
  // failed send comes back only into an empty box, so it never overwrites it.
  export const step = (s: T, e: Event): T => {
    switch (e.type) {
      case "Typed":
        return e.text === s.draft.text ? s : put(s, { text: e.text })
      case "Quoted":
        return put(s, { text: quoted(s.draft.text, e.text) })
      case "Attached":
        return s.draft.references.some(r => print(r) === print(e.reference)) ? s : put(s, { references: [...s.draft.references, e.reference] })
      case "Detached":
        return put(s, { references: s.draft.references.filter(r => print(r) !== print(e.reference)) })
      case "Picked":
        return { ...s, uploading: new Set([...s.uploading, e.id]) }
      case "Filed":
        return put(landed(s, e.id), { files: filed(s.draft.files, e.file) })
      case "Withheld":
        return { ...landed(s, e.id), held: [...s.held, { type: "Asking", w: e.w }] }
      case "Lost":
        return landed(s, e.id)
      case "Unfiled":
        return put(s, { files: s.draft.files.filter(f => f.id !== e.id) })
      case "Keeping":
        return asked(s, e.w, "Keeping")
      case "Unkept":
        return asked(s, e.w, "Asking")
      case "Kept":
        return put(unheld(s, e.w), { files: filed(s.draft.files, e.file) })
      case "Dropped":
        return unheld(s, e.w)
      case "Edit":
        return {
          ...s,
          mode: again.step(s.mode, { type: "edit", offer: { type: "Last", question: e.question } }),
          draft: { text: e.question.text, references: dedupe(parsed(e.question.references)), files: e.question.attachments },
        }
      case "Cancel":
        return emptied(s, again.step(s.mode, { type: "cancel" }))
      case "Send":
        return sending(s).type === "Ready" ? emptied(s, again.step(s.mode, { type: "sent" })) : s
      case "Failed":
        return isEmpty(s.draft) ? { ...s, draft: e.sent.draft, mode: e.sent.mode } : s
    }
  }
}
export default composing
