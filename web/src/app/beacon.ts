// A tab's failure told to the box: an uncaught error, a rejection nobody handled, a core that would not load.

import { about } from "../wire/api"
import http from "../wire/http"
import type { ClientReport } from "../wire/generated/wire"
import source from "../algebra/source"
import once from "../algebra/once"
import { parseRoute } from "../shell/route"
import type { Route } from "../shell/route"

export namespace beacon {
  export type Kind = "error" | "rejection" | "core" | "finding"
  export type Report = { kind: Kind; build: string; agent: string; page: Route["type"]; message: string; stack: string }
  // The page is the route's kind: its address can hold a todo's text or a mail search.
  export type Where = { build: string; agent: string; page: Route["type"] }
  export type Sent = { count: number; seen: ReadonlySet<string> }

  export const MOST = 10
  const MESSAGE_CHARS = 500
  const STACK_CHARS = 4000

  export const none: Sent = { count: 0, seen: new Set() }

  const cut = (s: string, n: number) => (s.length > n ? `${s.slice(0, n)}…` : s)

  const said = (thrown: unknown): { message: string; stack: string } =>
    thrown instanceof Error
      ? { message: thrown.message === "" ? thrown.name : `${thrown.name}: ${thrown.message}`, stack: thrown.stack ?? "" }
      : { message: typeof thrown === "string" ? thrown : String(thrown), stack: "" }

  export const of = (kind: Kind, thrown: unknown, at: Where): Report => {
    const { message, stack } = said(thrown)
    return { kind, ...at, message: cut(message.trim() === "" ? "(no message)" : message, MESSAGE_CHARS), stack: cut(stack, STACK_CHARS) }
  }

  export const key = (r: Report) => `${r.kind}\n${r.message}\n${r.stack}`

  // A tab says each failure once and at most MOST times in all: a loop is told by its first line.
  export const step = (s: Sent, r: Report): Sent =>
    s.count >= MOST || s.seen.has(key(r)) ? s : { count: s.count + 1, seen: new Set([...s.seen, key(r)]) }

  export type Effects = { post: (r: Report) => Promise<unknown>; build: () => Promise<string>; where: () => Omit<Where, "build"> }

  const reports = source.create<Sent, Report>(none, step)
  let effects: Effects | null = null

  export const tell = (kind: Kind, thrown: unknown): void => {
    const fx = effects
    if (fx === null) return
    void fx
      .build()
      .catch(() => "?")
      .then(build => reports.dispatch(of(kind, thrown, { build, ...fx.where() })))
  }

  export const post = (r: Report): Promise<unknown> => fetch("/api/client-errors", http.actRequest(r satisfies ClientReport))

  export const install = (fx: Effects = live()): void => {
    if (effects !== null) return
    effects = fx
    reports.subscribe((_, r) => void fx.post(r).catch(() => {}))
    window.addEventListener("error", e => tell("error", e.error ?? e.message))
    window.addEventListener("unhandledrejection", e => tell("rejection", e.reason))
  }

  const live = (): Effects => ({
    post,
    build: once.of(() => about().then(a => a.build)).get,
    where: () => ({ agent: navigator.userAgent, page: parseRoute(window.location.hash).type }),
  })
}
export default beacon
