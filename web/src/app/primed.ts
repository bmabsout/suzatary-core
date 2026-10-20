// The first read, started by the loader while the app is on its way and taken by the app's first ask of the same path
// (`wire/http.ts`). The loader and the app are two bundles, so the one thing they share is a property of the window.

import wait from "../wire/wait"

declare global {
  var suzataryPrimed: Map<string, Promise<primed.Read>> | undefined
}

export namespace primed {
  // The read's answer, or why it has none; held as a value, so nothing rejects with no one listening.
  export type Read = { readonly type: "Answered"; readonly r: Response } | { readonly type: "Failed"; readonly e: unknown }

  // Asked as a plain read, bounded as every read is (`ms`): the loader holds no key, and a read needs none.
  export const start = (path: string, f: typeof fetch = fetch, ms: number = wait.MS): void => {
    const held = (globalThis.suzataryPrimed ??= new Map())
    held.set(
      path,
      f(path, { headers: { accept: "application/json" }, signal: AbortSignal.timeout(ms) }).then(
        (r): Read => ({ type: "Answered", r }),
        (e: unknown): Read => ({ type: "Failed", e }),
      ),
    )
  }

  // The read started for `path`, once: a second ask of it is the app's own.
  export const take = (path: string): Promise<Read> | null => {
    const held = globalThis.suzataryPrimed
    const read = held?.get(path) ?? null
    held?.delete(path)
    return read
  }
}
export default primed
