// A turn row by who said it: each speaker's row holds only its own parts, read through this algebra.

import type { AttachmentRow, BlockRow, Receipt, TurnRow } from "../wire/generated/wire"

export namespace turns {
  export type Who = TurnRow["who"]
  export type Of<W extends Who> = Extract<TurnRow, { readonly who: W }>
  export type Question = Of<"person">

  export const fold = <R>(
    t: TurnRow,
    on: { person: (t: Of<"person">) => R; model: (t: Of<"model">) => R; verb: (t: Of<"verb">) => R; act: (t: Of<"act">) => R; box: (t: Of<"box">) => R },
  ): R => {
    switch (t.who) {
      case "person": return on.person(t)
      case "model": return on.model(t)
      case "verb": return on.verb(t)
      case "act": return on.act(t)
      case "box": return on.box(t)
    }
  }

  const none = <A>(): readonly A[] => []
  const pointed = (t: { readonly references: readonly string[] }): readonly string[] => t.references

  export const isQuestion = (t: TurnRow): t is Question => t.who === "person"
  export const references = (t: TurnRow): readonly string[] =>
    fold(t, { person: pointed, model: none<string>, verb: none<string>, act: pointed, box: pointed })
  export const attachments = (t: TurnRow): readonly AttachmentRow[] =>
    fold(t, { person: p => p.attachments, model: none<AttachmentRow>, verb: none<AttachmentRow>, act: none<AttachmentRow>, box: none<AttachmentRow> })
  export const blocks = (t: TurnRow): readonly BlockRow[] =>
    fold(t, { person: none<BlockRow>, model: m => m.blocks, verb: none<BlockRow>, act: none<BlockRow>, box: none<BlockRow> })
  export const queued = (t: TurnRow): boolean => isQuestion(t) && t.queued
  export const refused = (t: TurnRow): boolean => t.who === "verb" && t.refused
  export const verb = (t: TurnRow): string | null => (t.who === "verb" ? t.verb : null)
  export const remembered = (t: TurnRow): Receipt | null => (t.who === "verb" ? t.remembered : null)
}
export default turns
