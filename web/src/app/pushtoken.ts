// The Android portal's push token: its native code holds no key, so it hands the token to this page, which reports it signed.

import kv from "./kv"

export namespace pushtoken {
  export type Kept = { type: "None" } | { type: "Reported"; token: string; at: number }

  export const FRESH_MS = 7 * 24 * 60 * 60 * 1000
  export const KEY = "suzatary.pushtoken"
  export const HELD = "suzatary.pushtoken.held"
  export const MOST = 4096

  export const none: Kept = { type: "None" }

  const json = (raw: string | null): unknown => {
    try {
      return raw === null ? null : JSON.parse(raw)
    } catch {
      return null
    }
  }

  // What the app's postWebMessage gives: no source window and an empty origin. A window or a frame of a page has an origin.
  export const fromApp = (source: unknown, origin: string): boolean => source === null && origin === ""

  // The token a message from the app carries, or null.
  export const heard = (data: unknown, app: boolean): string | null => {
    const v = !app || typeof data !== "string" ? null : json(data)
    const token = typeof v === "object" && v !== null && "pushToken" in v ? v.pushToken : null
    return typeof token === "string" && token.length > 0 && token.length <= MOST ? token : null
  }

  export const due = (kept: Kept, token: string, now: number): boolean =>
    kept.type === "None" || kept.token !== token || now < kept.at || now - kept.at >= FRESH_MS

  export const read = (raw: string | null): Kept => {
    const v = json(raw)
    return typeof v === "object" && v !== null && "token" in v && "at" in v && typeof v.token === "string" && typeof v.at === "number"
      ? { type: "Reported", token: v.token, at: v.at }
      : none
  }

  export const print = (token: string, at: number): string => JSON.stringify({ token, at })

  export const session: kv.Kv = kv.over(() => window.sessionStorage)

  // In the boot script: the app hands the token when the page loads, which can be before the app's module is imported.
  export const hold = (held: kv.Kv = session): void =>
    window.addEventListener("message", (e: MessageEvent) => {
      if (fromApp(e.source, e.origin) && typeof e.data === "string") held.set(HELD, e.data)
    })

  export const install = (report: (token: string) => Promise<unknown>, store: kv.Kv = kv.local, held: kv.Kv = session, now: () => number = Date.now): void => {
    const take = (token: string | null): void => {
      if (token === null || !due(read(store.get(KEY)), token, now())) return
      void report(token).then(
        () => store.set(KEY, print(token, now())),
        () => undefined,
      )
    }
    const waiting = held.get(HELD)
    held.drop(HELD)
    take(heard(waiting, true))
    window.addEventListener("message", (e: MessageEvent) => take(heard(e.data, fromApp(e.source, e.origin))))
  }
}
export default pushtoken
