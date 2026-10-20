// The secretary's conversations: the list's value and view are `chat/list.ts`; this page fetches, debounces and navigates.
// `?ref=` and `?focus=1` forward to the default conversation; `mountChats` is the same list as the column beside one.

import app from "../app/app"
import type { ConversationRow } from "../wire/generated/wire"
import { openConversation } from "../wire/api"
import storesync from "../prodrome/storesync"
import conversations from "../chat/conversations"
import list from "../chat/list"
import { el } from "../view/dom"
import frame from "../view/frame"
import placeholder from "../view/placeholder"
import lo from "../algebra/lo"
import clock from "../algebra/clock"
import time from "../app/time"
import type { Mounted } from "../view/page"
import { hrefOf } from "../shell/route"
import source from "../algebra/source"
import reading from "../algebra/reading"
import { toast } from "../view/toast"
import address from "../shell/address"
import readonly from "../view/readonly"

export namespace secretary {
  const DEBOUNCE_MS = 250

  // A new topic opens at once with no name: the box names it after its first answer, and Rename is on its page.
  const starter = (): HTMLElement => {
    const start = (): void =>
      void openConversation("")
        .then(p => { address.go({ type: "conversation", id: p.id, refs: [], focus: true }) })
        .catch((e: unknown) => toast(lo.reason(e)))
    return el("button", { type: "button", class: "act act-newtopic", onclick: start }, "New topic")
  }

  // The search field and New topic, on one line outside the redrawn list, so a redraw keeps what is typed.
  type Head = { readonly node: HTMLElement; readonly listen: () => () => void }
  const head = (dispatch: (e: list.Event) => void): Head => {
    const field = el("input", { type: "search", class: "search chatsearch", placeholder: "Search", "aria-label": "Search conversations" })
    const writes = el("div", { class: "chatwrites" })
    const offer = (): void => {
      const reach = app.reach(app.now())
      writes.replaceChildren(reach.type === "Writes" ? starter() : readonly.line(reach) ?? "")
    }
    let timer: ReturnType<typeof setTimeout> | null = null
    field.addEventListener("input", () => {
      dispatch({ type: "Typed", q: field.value })
      if (timer !== null) clearTimeout(timer)
      const q = field.value.trim()
      timer = q === "" ? null : setTimeout(() => void lo.of(storesync.reading(s => s.search(q))).then(found => dispatch({ type: "Found", q, found: lo.map(found, p => p.hits) })), DEBOUNCE_MS)
    })
    return {
      node: el("div", { class: "chathead" }, field, writes),
      listen: () => {
        offer()
        const unwatch = app.hear(app.reach, offer, app.sameReach)
        return () => (timer !== null && clearTimeout(timer), unwatch())
      },
    }
  }

  // A failure of the reading is the list's own row, so the page never loses its filter or search to it.
  const rows = conversations.rows
  const listed = (r: lo.T<readonly ConversationRow[]>): list.Event => ({ type: "Listed", rows: r })

  // The list's instant moves when the owner's day turns, so a time of today reads "yesterday" from then on. A timer
  // counts only the time the device is awake, so a page shown again reads the instant afresh and arms again.
  const ticking = (dispatch: (e: list.Event) => void): (() => void) => {
    let stop = (): void => {}
    const arm = (): void => {
      stop()
      dispatch({ type: "Ticked", now: time.now() })
      stop = clock.wake(clock.nextDay(app.now().zone, time.now()), arm)
    }
    const shown = (): void => (document.visibilityState === "visible" ? arm() : undefined)
    arm()
    document.addEventListener("visibilitychange", shown)
    window.addEventListener("pageshow", shown)
    return () => (stop(), document.removeEventListener("visibilitychange", shown), window.removeEventListener("pageshow", shown))
  }

  export const page = (): frame.Page<list.T, list.Event> => {
    let h: Head | null = null
    return {
      className: "chats",
      title: "Secretary",
      masthead: "destination",
      now: dispatch => {
        h = head(dispatch)
        return [el("p", { class: "legend chatsmemory" }, el("a", { href: hrefOf({ type: "memory" }) }, "What the secretary remembers")), h.node]
      },
      listen: dispatch => {
        const us = [h?.listen() ?? (() => {}), ticking(dispatch)]
        return () => us.forEach(u => u())
      },
      reads: () => reading.map(rows, r => list.step(list.init(null, time.now()), listed(r))),
      shape: placeholder.ROWS,
      // A search on its way keeps the frame's placeholder.
      waits: list.searching,
      step: list.step,
      keeps: (was, now) => list.step(was, { type: "Listed", rows: now.rows }),
      view: (v, dispatch) => el("div", { class: "chatbody" }, list.view(v, { set: dispatch, waiting: null })),
    }
  }

  // The column beside a conversation, read again when a name in it moved.
  export type Chats = { readonly destroy: () => void; readonly again: () => void }

  // The column beside a conversation: the same head and list, no heading of its own.
  export const mountChats = (target: HTMLElement, current: string | null): Chats => {
    const s = source.create(list.init(current, time.now()), list.step)
    const h = head(s.dispatch)
    const body = el("div", { class: "chatbody" })
    // One placeholder for the column's life: a wait drawn again is the same node, so its 300 ms never start again.
    const waiting = placeholder.view(placeholder.ROWS)
    const draw = (): void => {
      const next = list.view(s.state(), { set: s.dispatch, waiting })
      if (next !== waiting || body.childNodes.length !== 1 || body.firstChild !== waiting) body.replaceChildren(...(next instanceof Node ? [next] : []))
    }
    const read = (): void => lo.fold(rows.now(), { loading: () => {}, error: () => {}, val: r => s.dispatch(listed(r)) })
    const unsub = s.subscribe(draw)
    const unlisten = h.listen()
    const untick = ticking(s.dispatch)
    const unheard = rows.hear(read)
    target.replaceChildren(h.node, body)
    draw()
    read()
    return { again: rows.again, destroy: () => (unsub(), unlisten(), untick(), unheard()) }
  }

  export const mount = (root: HTMLElement): Promise<Mounted> => frame.mount(root, page())
}
export default secretary
