/** Where the answer lands: what the round is doing, and what it has said so far. */

import type { ConversationState, Frame } from "../wire/generated/wire"
import { el } from "../view/dom"
import { verbPhrase } from "./verbphrase"
import clock from "../algebra/clock"

export namespace answer {
  export type Block = { n: number; sha: string; source: string }

  export type T =
    | { type: "Thinking"; since: number }
    | { type: "Working"; verb: string; checked: number; since: number }
    | { type: "Streaming"; blocks: Block[]; tail: string }
    | { type: "Done" }

  export const done: T = { type: "Done" }

  export const fold = <R>(
    a: T,
    on: {
      thinking: (since: number) => R
      working: (verb: string, checked: number, since: number) => R
      streaming: (blocks: Block[], tail: string) => R
      done: () => R
    },
  ): R => {
    switch (a.type) {
      case "Thinking": return on.thinking(a.since)
      case "Working": return on.working(a.verb, a.checked, a.since)
      case "Streaming": return on.streaming(a.blocks, a.tail)
      case "Done": return on.done()
    }
  }

  const since = (a: T, at: number): number => (a.type === "Thinking" || a.type === "Working" ? a.since : at)
  const checked = (a: T): number => (a.type === "Working" ? a.checked : 0)

  // What the box says the conversation is doing. A question the box holds is thinking from the moment it is seen,
  // queued or running, and a round already under way keeps its clock and its words.
  export const settle = (a: T, s: ConversationState, at: number): T =>
    s === "idle" ? (a.type === "Done" ? a : done)
    : a.type === "Done" ? { type: "Thinking", since: at }
    : a

  // A block closes only the prefix of the tail it names, in order; anything else is a frame from another reading and changes nothing.
  export const step = (a: T, f: Frame, at: number): T => {
    switch (f.kind) {
      case "state": return settle(a, f.state, at)
      case "working": return { type: "Working", verb: f.verb, checked: checked(a) + 1, since: since(a, at) }
      case "delta": return a.type === "Streaming" ? { ...a, tail: a.tail + f.text } : { type: "Streaming", blocks: [], tail: f.text }
      case "block":
        return a.type === "Streaming" && f.n === a.blocks.length && a.tail.startsWith(f.source)
          ? { type: "Streaming", blocks: [...a.blocks, { n: f.n, sha: f.sha, source: f.source }], tail: a.tail.slice(f.source.length) }
          : a
      case "turn": return a.type === "Done" ? a : done
    }
  }

  /** What the place says while nothing is written yet. */
  export const label = (a: T, now: number): string =>
    fold(a, {
      thinking: s => `Thinking · ${Math.max(0, Math.floor((now - s) / 1000))} s`,
      working: (v, n, s) => `${capital(verbPhrase(v))} · ${n} checked · ${Math.max(0, Math.floor((now - s) / 1000))} s`,
      streaming: () => "",
      done: () => "",
    })

  /** When `label` next reads otherwise: the elapsed count's next whole second, and never once nothing counts. */
  export const next = (a: T, now: number): clock.Next =>
    fold(a, {
      thinking: s => clock.nextSecond(s, now),
      working: (_, __, s) => clock.nextSecond(s, now),
      streaming: () => clock.never,
      done: () => clock.never,
    })

  /** An answer's words, block by block: what Copy copies. */
  export const plain = (blocks: readonly { plain: string }[]): string => blocks.map(b => b.plain).join("\n\n")

  const capital = (s: string): string => s.charAt(0).toUpperCase() + s.slice(1)

  /** The answer as it lands: each closed block as the box renders it (`render`, an effect that names the block by its sha), the open tail as text. */
  export const view = (a: T, now: number, { render }: { render: (b: Block) => Node }): Node =>
    fold<Node>(a, {
      thinking: () => el("p", { class: "answerstate" }, label(a, now)),
      working: () => el("p", { class: "answerstate" }, label(a, now)),
      streaming: (blocks, tail) =>
        el("div", { class: "turnanswer streaming" }, blocks.map(render), tail === "" ? null : el("div", { class: "answertail" }, tail)),
      done: () => el("div", { class: "answerdone" }),
    })
}
export default answer
