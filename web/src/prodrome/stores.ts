// The secretary's two stores as replicas this tab holds: the box alone writes them, so a replica is a down-set
// of the box's objects, held beside the keys that open their words, and two of one store join as `locked` joins.

import heads from "./heads"
import locked from "./locked"
import type { Box, Replica } from "./replica"
import { said } from "./replica"
import type { ObjectsPayload } from "../wire/generated/wire"

export namespace stores {
  export type Name = "inbox" | "lines"
  export const NAMES: readonly Name[] = ["inbox", "lines"]

  export type T = { readonly [k in Name]: locked.Row }
  // A row taken in, joined; or keys whose blocks are forgotten, dropped.
  export type Event =
    | { readonly type: "Held"; readonly name: Name; readonly row: locked.Row }
    | { readonly type: "Forgot"; readonly name: Name; readonly keys: readonly string[] }

  export const NONE: T = { inbox: locked.NONE, lines: locked.NONE }

  // What a reading or a pull can see: the generation, the objects, the box's heads, the policy they are read under and
  // the keys, never the instant they were heard. A reset of an empty row holds what it held, and is still a step.
  export const same = (a: Replica, b: Replica): boolean =>
    a.generation === b.generation &&
    a.objects.length === b.objects.length &&
    sameBox(a.box, b.box)

  // Two words are one when both are unheard, or both name the same heads under the same policy.
  const sameBox = (a: Box, b: Box): boolean =>
    a.type === "Heard" && b.type === "Heard"
      ? heads.same(heads.of(a.word.heads), heads.of(b.word.heads)) && sameActors(a.word.untrusted, b.word.untrusted)
      : a.type === b.type

  // Two policies are one when they name the same actors, in any order and however often.
  export const sameActors = (a: readonly string[], b: readonly string[]): boolean => {
    const x = new Set(a), y = new Set(b)
    return x.size === y.size && [...x].every(n => y.has(n))
  }

  export const step = (t: T, e: Event): T => {
    const was = t[e.name]
    const next = e.type === "Held" ? locked.join(was, e.row) : locked.without(was, e.keys)
    return next === was || (same(next.replica, was.replica) && locked.sameKeys(next.keys, was.keys)) ? t : { ...t, [e.name]: next }
  }

  // What the box is known to hold, for `since=`: every object here came from it.
  export const asked = (r: Replica): heads.Heads => heads.of(r.heads)

  // Whether the box answered in a later epoch than the word this replica heard: its store no longer extends what it
  // served (a migration rewrote objects, or it went back to an earlier checkpoint), so what is held may name objects
  // the box no longer holds, and the replica, a copy of the box's alone, starts over and is taken again whole.
  export const overtaken = (r: Replica, answer: ObjectsPayload): boolean =>
    said(r.box, false, w => w.epoch < answer.epoch)

  // Whether the box's heads name an object the tab does not hold, landed or not: a move of the store, or a pull that was
  // cut before it took everything. A frame that says so pulls; one that does not pulls nothing.
  export const behind = (box: heads.Heads, held: Replica): boolean => {
    const names = new Set(held.objects.map(o => o.hash))
    return box.some(h => !names.has(h))
  }
}
export default stores
