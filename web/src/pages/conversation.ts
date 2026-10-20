// One conversation as a page: its name and who may rename it are the value; the transcript, the composer,
// the heading's rename field and the list beside it at 62rem are islands.

import app from "../app/app"
import type { Reference } from "../chat/refs"
import { parse as parseReference } from "../chat/refs"
import intentsync from "../prodrome/intentsync"
import chat from "../chat/chat"
import chatlist from "../chat/list"
import { el } from "../view/dom"
import frame from "../view/frame"
import asked from "../app/asked"
import reading from "../algebra/reading"
import marks from "../view/marks"
import type { Mounted } from "../view/page"
import { hrefOf } from "../shell/route"
import { toast } from "../view/toast"
import secretary from "./secretary"
import lo from "../algebra/lo"

export namespace conversation {
  // What the heading knows of the conversation's name: nothing yet, nothing because its window could not be read, or
  // the name a window gave ("" for a topic the box has not named yet).
  export type Name =
    | { readonly type: "Reading" }
    | { readonly type: "Unread" }
    | { readonly type: "Named"; readonly title: string; readonly model: string }
  export type T = { readonly name: Name; readonly writes: boolean }
  export type Event =
    | { readonly type: "Named"; readonly title: string; readonly model: string }
    | { readonly type: "Unread" }
    | { readonly type: "Writes"; readonly writes: boolean }
  type Parts = "head" | "chats" | "transcript" | "composer"

  export type Options = {
    readonly refs: readonly string[]
    readonly focus: boolean
    // A search hit's turn: the first window ends at it.
    readonly at?: number
  }

  // A name once read stays: a later window that cannot be read leaves it.
  export const step = (v: T, e: Event): T => {
    switch (e.type) {
      case "Named":
        return v.name.type === "Named" && v.name.title === e.title && v.name.model === e.model ? v : { ...v, name: { type: "Named", title: e.title, model: e.model } }
      case "Unread":
        return v.name.type === "Reading" ? { ...v, name: { type: "Unread" } } : v
      case "Writes":
        return v.writes === e.writes ? v : { ...v, writes: e.writes }
    }
  }

  export const titleOf = (n: Name): string => (n.type === "Named" ? n.title : "")

  // Mounted the first time the width allows, so a phone fetches no list it cannot see; the stylesheet hides it below.
  const WIDE = "(min-width: 62rem)"

  // The heading becomes a field until Enter asks for the name or Escape restores it; the replica brings it back.
  const head = (id: string, saved: () => void): frame.Island<T> => {
    const heading = el("h1", { class: "pagetitle" })
    const writes = el("span", { class: "convwrites" })
    const button = el("button", { type: "button", class: "act act-rename" }, "Rename")
    // Opens on the name as stored, so an unnamed topic stays unnamed.
    const open = (stored: string): void => {
      const field = el("input", { type: "text", class: "renamefield", maxlength: "80", value: stored, "aria-label": "Conversation name" })
      const back = (): void => {
        field.replaceWith(heading)
        button.hidden = false
      }
      field.addEventListener("keydown", e => {
        if (e.key === "Escape") back()
        if (e.key !== "Enter") return
        void intentsync.asked({ type: "Rename", conversation: id, title: field.value })
          .then(() => (back(), saved()))
          .catch((error: unknown) => toast(lo.reason(error)))
      })
      heading.replaceWith(field)
      button.hidden = true
      field.focus()
      field.select()
    }
    return {
      node: marks.head(el("div", { class: "convhead" }, heading, writes)),
      // The model's name is only a tooltip.
      update: v => {
        heading.textContent = v.name.type === "Named" ? chatlist.named(v.name.title) : v.name.type === "Unread" ? "Conversation" : ""
        heading.classList.toggle("pagewaiting", v.name.type === "Reading")
        button.onclick = () => open(titleOf(v.name))
        heading.title = v.name.type === "Named" ? v.name.model : ""
        if (writes.contains(button) !== v.writes) writes.replaceChildren(...(v.writes ? [button] : []))
      },
    }
  }

  // A name that moved (the title job's, or a rename) moves in the column too.
  const chats = (id: string): frame.Island<T> => {
    const aside = el("aside", { class: "chatsaside", "aria-label": "Conversations" })
    const wide = window.matchMedia(WIDE)
    let list: secretary.Chats | null = null
    let named: string | null = null
    const place = (): void => {
      if (list === null && wide.matches) list = secretary.mountChats(aside, id)
    }
    wide.addEventListener("change", place)
    place()
    return {
      node: aside,
      update: v => {
        if (v.name.type !== "Named") return
        if (named !== null && v.name.title !== named) list?.again()
        named = v.name.title
      },
      destroy: () => (wide.removeEventListener("change", place), list?.destroy()),
    }
  }

  export const page = (id: string, o: Options): frame.Page<T, Event, Parts> => ({
    className: "conversation",
    title: "Conversation",
    reads: () => reading.constant<T>({ name: { type: "Reading" }, writes: app.writes(app.now()) }),
    step,
    listen: dispatch => app.hear(app.writes, writes => dispatch({ type: "Writes", writes })),
    islands: dispatch => {
      const talk = chat.mount({
        id,
        ...(o.at === undefined ? {} : { at: o.at }),
        seeded: o.refs.flatMap(text => {
          const r: Reference | null = parseReference(text)
          return r === null ? [] : [r]
        }),
        focus: o.focus,
        scroll: document.scrollingElement ?? document.documentElement,
        loaded: s => dispatch({ type: "Named", title: s.title, model: s.model }),
        unread: () => dispatch({ type: "Unread" }),
      })
      // The box moving asks the conversation for its newest window, and the name stays meanwhile.
      const unmoved = asked.hearMoves(() => void talk.source.refresh())
      return {
        head: head(id, () => void talk.source.refresh()),
        chats: chats(id),
        transcript: { node: talk.transcript.content, update: () => {}, destroy: () => (unmoved(), talk.destroy()) },
        composer: { node: talk.composer.root, update: () => {} },
      }
    },
    masthead: (_, at) => at.head,
    now: () => el("p", { class: "legend memorylink" }, el("a", { href: hrefOf({ type: "memory" }) }, "What the secretary remembers")),
    around: (column, at) => el("div", { class: "chatswide" }, at.chats, column),
    view: (_, __, ___, at) => [at.transcript, at.composer],
  })

  export const mount = (root: HTMLElement, id: string, o: Options): Promise<Mounted> => frame.mount(root, page(id, o))
}
export default conversation
