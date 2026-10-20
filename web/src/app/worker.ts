// The service worker's work: each build's files from the cache named by that build, under the names that build's pages
// load them by; the page and its loader from this worker's own build; the box's build as last heard; never the API. It
// keeps no record of who runs what: a page asks for its own build's files by name, and a cache goes once no open page
// answers that it runs that build.

import shell from "./shell"

export namespace worker {
  type Waiting = Event & { waitUntil: (p: Promise<unknown>) => void }
  // Chromium's install event, which takes routes the browser follows without waking the worker.
  type Route = { condition: { urlPattern: string }; source: "network" }
  type Installing = Waiting & { addRoutes: (routes: readonly Route[]) => Promise<void> }
  type Fetching = Waiting & { request: Request; respondWith: (r: Promise<Response>) => void }
  type Messaged = Event & { data: unknown }
  type Page = { postMessage: (message: unknown, transfer: Transferable[]) => void }
  type Worker = { scriptURL: string } | null
  export type Scope = {
    location: { href: string }
    registration: { scope: string; installing: Worker; waiting: Worker }
    skipWaiting: () => Promise<void>
    clients: { claim: () => Promise<void>; matchAll: (o: { type: "window" }) => Promise<readonly Page[]> }
    addEventListener: (type: string, f: (e: Event) => void) => void
  }

  export const isScope = (g: unknown): g is Scope => typeof g === "object" && g !== null && "registration" in g && "skipWaiting" in g
  const waiting = (e: Event): e is Waiting => "waitUntil" in e
  const fetching = (e: Event): e is Fetching => waiting(e) && "respondWith" in e && "request" in e
  const messaged = (e: Event): e is Messaged => "data" in e
  const routing = (e: Waiting): e is Installing => "addRoutes" in e
  const said = (v: unknown, type: string): boolean => typeof v === "object" && v !== null && "type" in v && v.type === type

  // How long a page has to say which build it runs before the sweep keeps every cache.
  const ANSWER_MS = 3000

  export const run = (self: Scope, id: string, chunks: readonly string[]): void => {
    const files = shell.files(chunks)
    const at = (file: string) => new URL(file, self.registration.scope).href
    const own = (file: string) => at(shell.under(id, file))

    // The build the box serves, as the loader learns it: one source for both.
    const served = (): Promise<string> =>
      fetch(at(shell.CURRENT), { cache: "no-store" })
        .then(r => (r.ok ? r.text() : ""))
        .then(t => t.trim())

    // All of one build or none: the box is asked which build it serves before and after, and a deploy in between fails the
    // install; the page and its loader carry no build in their names, so only the box's answer says whose they are.
    const precache = async (): Promise<void> => {
      const before = await served()
      if (before !== id) throw new Error(`the box serves ${before}, this worker is ${id}`)
      const urls = [...shell.ROOT.map(at), ...files.map(own)]
      const got = await Promise.all(
        urls.map(u =>
          fetch(u, { cache: "reload" }).then(r => {
            if (!r.ok) throw new Error(`${u}: HTTP ${r.status}`)
            return { u, r }
          }),
        ),
      )
      const after = await served()
      if (after !== id) throw new Error(`the box moved to ${after} while ${id} was being kept`)
      const cache = await caches.open(shell.cacheOf(id))
      await Promise.all(got.map(({ u, r }) => cache.put(u, r)))
      // The box's build as heard now, so the next start asks the network for nothing before its app runs.
      await caches.open(shell.HEARD).then(c => c.put(at(shell.CURRENT), new Response(id)))
    }

    const answer = (page: Page): Promise<string | null> =>
      new Promise(done => {
        const channel = new MessageChannel()
        channel.port1.onmessage = e => done(typeof e.data === "string" && e.data !== "" ? e.data : null)
        setTimeout(() => done(null), ANSWER_MS)
        page.postMessage({ type: "which" }, [channel.port2])
      })

    // Each open page is asked which build it runs; a build's cache goes once none answers it, and nothing goes while one
    // does not answer. A worker installing or waiting keeps its build's cache too, which it is filling or will serve.
    const sweep = async (): Promise<void> => {
      const open = await self.clients.matchAll({ type: "window" })
      const builds = await Promise.all(open.map(answer))
      const next = [self.registration.installing, self.registration.waiting].flatMap(w => (w === null ? [] : [shell.named(shell.buildOf(w.scriptURL))]))
      const names = await caches.keys()
      await Promise.all(shell.unused(names, id, [...builds, ...next]).map(n => caches.delete(n)))
    }

    // Kept whole, it takes over at once: every page loads its own build's files by name, so whichever worker serves a page
    // serves it the same files, and the next cold start offline runs the newest build kept. A failed install deletes
    // nothing: open pages of this build may be reading its cache, and only the sweep, which asks them, lets one go.
    self.addEventListener("install", e => {
      if (!waiting(e)) return
      const routed = routing(e) ? e.addRoutes(shell.NETWORK.map(p => ({ condition: { urlPattern: at(p) }, source: "network" }))) : Promise.resolve()
      e.waitUntil(Promise.all([routed, precache()]).then(() => self.skipWaiting()))
    })

    self.addEventListener("activate", e => {
      if (waiting(e)) e.waitUntil(self.clients.claim().then(sweep))
    })

    self.addEventListener("message", e => {
      if (!messaged(e)) return
      if (said(e.data, "sweep") && waiting(e)) e.waitUntil(sweep())
    })

    // A build's file is the box's under that build's name only while the box serves that build, so what the network
    // answers under it is that build's file or a refusal, never another build's; it is kept in that build's cache.
    const built = async (b: string, file: string, request: Request): Promise<Response> => {
      const url = at(shell.under(b, file))
      const cache = await caches.open(shell.cacheOf(b))
      const hit = await cache.match(url)
      if (hit !== undefined) return hit
      const r = await fetch(request)
      if (r.ok) await cache.put(url, r.clone())
      return r
    }

    // The box's build as last heard, and the box asked again behind the answer: a start runs that build at once, and a
    // newer one is the page's to offer (`newbuild`). With nothing heard yet, the box's answer is the answer.
    const current = async (e: Fetching): Promise<Response> => {
      const url = at(shell.CURRENT)
      const cache = await caches.open(shell.HEARD)
      const hit = await cache.match(url)
      const asked = fetch(url, { cache: "no-store" }).then(async r => {
        if (r.ok) await cache.put(url, r.clone())
        return r
      })
      if (hit === undefined) return asked
      e.waitUntil(asked.catch(() => undefined))
      return hit
    }

    const respond = (e: Fetching, r: Exclude<shell.Route, { type: "Pass" }>): Promise<Response> => {
      switch (r.type) {
        case "Root":
          return caches.open(shell.cacheOf(id)).then(c => c.match(at(r.file))).then(hit => hit ?? fetch(e.request))
        case "Current":
          return current(e)
        case "Shell":
        case "Kept":
          return built(r.build, r.file, e.request)
      }
    }

    self.addEventListener("fetch", e => {
      if (!fetching(e)) return
      const r = shell.route(e.request.url, self.registration.scope, e.request.method)
      if (r.type !== "Pass") e.respondWith(respond(e, r))
    })
  }
}
export default worker
