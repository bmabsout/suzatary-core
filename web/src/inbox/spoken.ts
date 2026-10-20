// What a card rests on, named as the owner would say it, never by its reference's spelling.

import type { Entry } from "../wire/generated/wire"
import type { Reference } from "../chat/refs"
import { parse } from "../chat/refs"

export namespace spoken {
  // Each todo's title, as a list shows it: its body's first line, markup marks dropped.
  export type Titles = ReadonlyMap<string, string>

  export const titles = (entries: readonly Entry[]): Titles =>
    new Map(entries.flatMap((e): [string, string][] => {
      const t = ((e.authored?.body ?? "").split("\n")[0] ?? "").replace(/[#*_\\[\]]/g, "").trim()
      return t === "" ? [] : [[e.todo, t]]
    }))

  export const same = (a: Titles, b: Titles): boolean => a.size === b.size && [...a].every(([k, t]) => b.get(k) === t)

  export const of = (r: Reference, titles: Titles): string => {
    switch (r.kind) {
      case "todo":
        return titles.get(r.id) ?? "a todo"
      case "mail":
        return "a message"
      case "thread":
        return "a thread"
      case "briefing":
        return `the briefing of ${r.date}`
      case "proposal":
        return "an earlier card"
      case "batch":
        return "a held batch"
    }
  }

  // A source as the card names it: its words, or the text itself when it is no reference.
  export const said = (text: string, titles: Titles): string => {
    const r = parse(text)
    return r === null ? text : of(r, titles)
  }
}
export default spoken
