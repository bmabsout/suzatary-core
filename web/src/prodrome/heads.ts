// A replica's heads: the objects nothing it holds rests on, a set kept sorted, never one of them picked.

export namespace heads {
  export type Heads = readonly string[]

  export const NONE: Heads = []

  /** Sorted and distinct, so two spellings of one set compare equal. */
  export const of = (names: Iterable<string>): Heads => [...new Set(names)].sort()

  export const same = (a: Heads, b: Heads): boolean => a.length === b.length && a.every((h, i) => h === b[i])

  /** At most `SHOWN` names: every enrolment's signature is a head too, and a sentence of thirty names says nothing. */
  export const SHOWN = 3

  export const shown = (h: Heads): string =>
    h.length === 0
      ? "nothing"
      : [...h.slice(0, SHOWN).map(n => `${n.slice(0, 12)}…`), ...(h.length > SHOWN ? [`${h.length - SHOWN} more`] : [])].join(", ")
}

export default heads
