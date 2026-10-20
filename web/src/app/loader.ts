// What the loader decides, apart from the page it runs in: which builds to try, in which order, and what each is asked
// for beside its app. The page's effects are passed in (`boot.ts`), so a test runs a start against a worker of its own.

export namespace loader {
  export type Effects = {
    // The build the box serves, as the worker last heard it; null when nothing answered.
    readonly served: () => Promise<string | null>
    // The build of the worker serving this page, which is the build of this loader; null with no worker.
    readonly worker: () => string | null
    // Asks for build `b`'s stylesheets, its app and `chunks` of it beside each other, before any of them is needed.
    readonly preload: (b: string, chunks: readonly string[]) => void
    // Whether build `b`'s app started.
    readonly load: (b: string) => Promise<boolean>
    readonly said: (text: string) => void
  }

  export type Started = { readonly type: "Ran"; readonly build: string } | { readonly type: "Refused"; readonly said: string }

  export const NAMELESS = "The box did not say which version it serves. Reload to ask again."
  export const UNLOADED = "This version of the app did not load. Reload to ask the box again."

  // The box's build first, then the worker's, each once. `chunks` are what this loader's own build's app imports as it
  // loads: the names are that build's, so another build is asked for its stylesheets and app alone. This loader is
  // the worker's build when one serves the page, and the box's when none does, since the box sent it.
  export const start = async (fx: Effects, chunks: readonly string[]): Promise<Started> => {
    const served = await fx.served().catch(() => null)
    const worker = fx.worker()
    const own = worker ?? served
    const tries = [served, worker].filter((b, i, all): b is string => b !== null && all.indexOf(b) === i)
    const refused = (said: string): Started => (fx.said(said), { type: "Refused", said })
    if (tries.length === 0) return refused(NAMELESS)
    for (const b of tries) {
      fx.preload(b, b === own ? chunks : [])
      if (await fx.load(b)) return { type: "Ran", build: b }
    }
    return refused(UNLOADED)
  }
}
export default loader
