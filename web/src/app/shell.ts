// The app shell: which files are one build, the names they are loaded by and the cache that holds them. A build's files
// are named by it (`b/<build>/app.js`), so a page and every worker it starts load only their own build's files: no page
// runs two builds' files, whichever worker serves it, and a page on an old build is safe to run for as long as it is open.

export namespace shell {
  export type Build = { type: "Stamped"; id: string } | { type: "Dev" }
  // A request to the worker: the page and its loader, which every build serves from its own cache; the name of the build
  // the box serves, answered as last heard and asked again behind the answer; a file of one build, kept at install or on
  // first use; or nothing of the shell's, the API least of all.
  export type Route =
    | { type: "Root"; file: string }
    | { type: "Current" }
    | { type: "Shell"; build: string; file: string }
    | { type: "Kept"; build: string; file: string }
    | { type: "Pass" }
  // What a page running a build says of it, by the builds this device last ran: nothing on its first, once on a new one.
  export type News = { type: "Known" } | { type: "First"; id: string } | { type: "New"; id: string }
  // The line across the top: the build this page started on, said once per device, or a served build the reader may load.
  export type Line =
    | { type: "None" }
    | { type: "Updated"; version: string; revision: string }
    | { type: "Ready"; version: string }

  // A dev binary names no build, so nothing can be kept under it: a dev tab always runs the network's files.
  export const DEV = "dev"
  export const build = (id: string): Build => (id === "" || id === DEV ? { type: "Dev" } : { type: "Stamped", id })
  // The name a build's files are loaded under, `dev` for a dev binary, as the box serves them.
  export const named = (b: Build): string => (b.type === "Stamped" ? b.id : DEV)
  export const BUILT = "b/"
  // Where the box names the build it serves, beside the files (`serve::versioned`).
  export const CURRENT = "b/current"
  export const under = (b: string, file: string): string => `${BUILT}${b}/${file}`
  // The build a page's or a worker's own file was loaded under; a file named by none is a dev build's.
  export const ownBuild = (fileUrl: string): Build => {
    const m = /\/b\/([^/]+)\//.exec(new URL(fileUrl).pathname)
    return build(m?.[1] ?? "")
  }
  export const fold = <R>(b: Build, on: { stamped: (id: string) => R; dev: () => R }): R =>
    b.type === "Stamped" ? on.stamped(b.id) : on.dev()

  export const PREFIX = "suzatary-shell-"
  export const cacheOf = (id: string) => `${PREFIX}${id}`
  // The caches a worker of `id` may delete, by the build each open page answered it runs (`null`: no answer). A page
  // still on an older build keeps loading that build's files, so its cache stays until no page runs it, and nothing
  // goes while a page has not answered.
  export const unused = (names: readonly string[], id: string, pages: readonly (string | null)[]): string[] =>
    pages.includes(null) ? [] : names.filter(n => n.startsWith(PREFIX) && n !== cacheOf(id) && !pages.some(p => p !== null && n === cacheOf(p)))

  // The two small faces, kept with the shell; the two Source Serif files (250 KB of the 330) are kept as the stylesheet
  // first draws a text in them, which the browser fetches only then.
  export const FACES: readonly string[] = ["CrimsonPro-200-900.woff2", "source-sans-3-latin-wght-normal.woff2"]
  export const SERIF: readonly string[] = ["source-serif-4-latin-opsz-italic.woff2", "source-serif-4-latin-opsz-normal.woff2"]
  export const FONTS: readonly string[] = [...FACES, ...SERIF]
  // The Typst kit and its views: data, fetched the first time a Typst view is drawn, with the packages they import.
  export const TYPST: readonly string[] = ["kit.typ", "list.typ"]
  export const PACKAGES = "packages.json"
  // Kept on first use rather than at install: the Typst module is 22 MB and most visits never draw a Typst view.
  export const KEPT: readonly string[] = [
    "typst.js",
    "typst_bg.wasm",
    "typstwork.js",
    ...[...TYPST, PACKAGES].map(f => `typst/${f}`),
    ...SERIF.map(f => `fonts/${f}`),
  ]
  // The cache the box's build is last heard in: one name for every build, and no shell's, so no sweep takes it.
  export const HEARD = "suzatary-current"
  // The page and its loader: build-free names, each worker's own copy.
  export const ROOT: readonly string[] = ["index.html", "boot.js"]
  // The files every build has by these names. A build's pages and the code they share are chunks besides, named by their
  // content when it is bundled (`scripts/build-web.sh`), so the worker is handed its build's list as it is built.
  export const FILES: readonly string[] = [
    "app.js",
    "prodrome.js",
    "prodrome_bg.wasm",
    "prodromework.js",
    "tokens.css",
    "style.css",
    ...FACES.map(f => `fonts/${f}`),
  ]
  // A build's shell: its fixed files and its chunks, all kept at install.
  export const files = (chunks: readonly string[]): readonly string[] => [...FILES, ...chunks]
  // A chunk is named by its content (`<name>-<hash>.js`), so its shape says it is one of some build's: a worker serves
  // an older build's tab that build's chunks from that build's cache, though they are not its own.
  export const chunk = (file: string): boolean => /^[a-z][a-z0-9_]*-[A-Z0-9]{8}\.js$/.test(file)

  // Relative to the worker's scope; the scope itself is the page, which is index.html.
  export const route = (url: string, scope: string, method: string): Route => {
    if (method !== "GET" || !url.startsWith(scope)) return { type: "Pass" }
    const rel = url.slice(scope.length).replace(/[?#].*$/, "")
    const root = rel === "" ? "index.html" : rel
    if (ROOT.includes(root)) return { type: "Root", file: root }
    if (rel === CURRENT) return { type: "Current" }
    const m = /^b\/([^/]+)\/(.+)$/.exec(rel)
    if (m === null) return { type: "Pass" }
    const [, b = "", file = ""] = m
    return FILES.includes(file) || chunk(file) ? { type: "Shell", build: b, file } : KEPT.includes(file) ? { type: "Kept", build: b, file } : { type: "Pass" }
  }

  // What the browser sends to the box without waking the worker (Chromium's static routes): the API, which the worker
  // passes anyway. Woken for each of the page's calls, an older worker would get an event at the moment it gives way to
  // a newer one, and Chromium then holds the newer one waiting.
  export const NETWORK: readonly string[] = ["api/*"]

  export const script = (id: string) => `./sw.js?build=${encodeURIComponent(id)}`
  export const buildOf = (scriptUrl: string): Build => build(new URL(scriptUrl).searchParams.get("build") ?? "")

  // `last` is the build this device last announced; a dev build has no revision to name.
  export const news = (last: readonly string[], running: Build): News =>
    running.type === "Dev" || last.includes(running.id) ? { type: "Known" }
    : last.length === 0 ? { type: "First", id: running.id }
    : { type: "New", id: running.id }
}
export default shell
