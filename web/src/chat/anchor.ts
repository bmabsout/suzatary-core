/** Where the transcript's box is held: at the newest turn, at a question asked and not yet known, at the question
 * just sent, or wherever the reader scrolled. */

import { staysAtBottom } from "./scrollpos"

export namespace anchor {
  export type T =
    | { type: "Newest" }
    /** `waiting` sends are in flight, one or more: a question can be asked again before the last one is known. */
    | { type: "Asking"; waiting: number }
    | { type: "Question"; turn: number }
    | { type: "Drifted" }

  /** Where the reader left the box. */
  export type Place = "bottom" | "above"
  /** Who moved it: the reader's own scrolling, or the page (its own placing, content shrinking under the box, the
   * browser anchoring it against content drawn above). */
  export type Mover = "reader" | "page"

  export type Event =
    | { type: "window"; fresh: boolean }
    | { type: "asked" }
    | { type: "sent"; turn: number }
    | { type: "scrolled"; by: Mover; at: Place }
    | { type: "grew" }
    | { type: "pill" }

  /** What the box does about it. */
  export type Move =
    | { type: "None" }
    | { type: "ToBottom" }
    | { type: "ToTurn"; turn: number }
    | { type: "Pill" }

  export const newest: T = { type: "Newest" }
  const none: Move = { type: "None" }

  // A growing answer never moves the box; only a send, a first draw, the pill, or a new window at the newest turn does.
  // The reader's scroll settles where the box is held, whatever held it; the page moving the box settles nothing. A question is asked at the tap and its turn known when the box answers: the reader may scroll between
  // the two, and what they did since the tap stands, so a send holds its question only while it is still being asked.
  // Asked again before the first is known, only the last send to be known holds a question: an earlier one settling
  // after the next was asked would hold the question before it, and the last would find it held and hold nothing.
  export const step = (t: T, e: Event): [T, Move] => {
    switch (e.type) {
      case "window":
        return e.fresh ? [newest, { type: "ToBottom" }]
          : t.type === "Newest" || t.type === "Asking" ? [t, { type: "ToBottom" }]
          : t.type === "Drifted" ? [t, { type: "Pill" }]
          : [t, none]
      case "asked": return [{ type: "Asking", waiting: t.type === "Asking" ? t.waiting + 1 : 1 }, none]
      case "sent":
        return t.type !== "Asking" ? [t, none]
          : t.waiting > 1 ? [{ type: "Asking", waiting: t.waiting - 1 }, none]
          : [{ type: "Question", turn: e.turn }, { type: "ToTurn", turn: e.turn }]
      case "scrolled": return e.by === "page" ? [t, none] : [e.at === "bottom" ? newest : { type: "Drifted" }, none]
      case "grew": return [t, none]
      case "pill": return [newest, { type: "ToBottom" }]
    }
  }

  /** What a window read at `total` turns is, for a box last held against `was` (null before the first): a new
   * window only when it is the first or its turns moved; a refresh that brings none is no event at all. */
  export const windowed = (was: number | null, total: number): Event | null =>
    was === total ? null : { type: "window", fresh: was === null }

  /** A move the page asked for at `at`, while the box was held at `held`. The box is moved at the next frame, after
   * the reader's input waiting for it has reached the page: the compositor may be scrolling for a wheel the page has
   * not heard yet, and moving the box would stop it. */
  export type Asked = { readonly at: number; readonly held: T }

  /** Whether the move still stands at that frame: no reader input since it was asked, and the box held as it was. */
  export const stands = (asked: Asked, readerAt: number, held: T): boolean => readerAt < asked.at && held === asked.held

  /** Whether the box keeps room beneath the question for its answer to grow into. */
  export const reserves = (t: T): boolean => t.type === "Question"

  /** Where the reader sees to and where the answer's place ends (the room starts there), in the box's own
   * coordinates, and its height. */
  export type View = { readonly bottom: number; readonly end: number; readonly height: number }

  /** The room under the answer's place, `was` before: a window's height while the question is held; otherwise only as
   * much of what it was as the reader still sees, so letting the room go never leaves the box shorter than the view.
   * Dropped at once, the browser would pull the reader down to the new end, a scroll that reads as theirs. */
  export const room = (t: T, was: number, view: View): number =>
    reserves(t) ? view.height : Math.min(was, Math.max(0, view.bottom - view.end))

  /** A scroll box's numbers: how far it is scrolled, how tall its content is, the room in that content, and its height. */
  export type Box = { readonly top: number; readonly scrollHeight: number; readonly room: number; readonly height: number }

  /** Where the reader is, measured against the end of what the box shows and never against the room: the room is the
   * box's own reserve, and it goes as soon as the hold does, so a place read against it would no longer be true once
   * it went. Read this way, letting the room go never moves the reader from above to the bottom. */
  export const placed = (box: Box): Place =>
    staysAtBottom(box.top, box.scrollHeight - box.room, box.height) ? "bottom" : "above"
}
export default anchor
