// The page's half of revalidation: a read sends the tag of the answer it holds, a 304 is that answer again, a changed
// answer replaces it, and the loader's first read is taken once by the app's first ask of its path.

import { test } from "node:test"
import assert from "node:assert/strict"

import http from "../src/wire/http"
import primed from "../src/app/primed"
import d from "../src/wire/decoder"

// A box over one answer per path: it answers 304 to the answer's own tag, and writes down every tag it was sent.
const box = () => {
  const answers = new Map<string, string>()
  const sent: [string, string | null][] = []
  const tagOf = (body: string) => `W/"${body.length}-${[...body].reduce((h, c) => (h * 31 + c.charCodeAt(0)) >>> 0, 7)}"`
  const fetch = (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const path = typeof input === "string" ? input : input instanceof URL ? input.pathname : input.url
    const held = new Headers(init?.headers).get("if-none-match")
    sent.push([path, held])
    const body = answers.get(path) ?? "null"
    const tag = tagOf(body)
    return Promise.resolve(held === tag ? new Response(null, { status: 304, headers: { etag: tag } }) : new Response(body, { headers: { etag: tag, "content-type": "application/json" } }))
  }
  Object.defineProperty(globalThis, "fetch", { value: fetch, configurable: true })
  return { answers, sent, fetch }
}

test("a read sends the tag it holds, a 304 is the same answer, and a moved answer replaces it", async () => {
  const b = box()
  b.answers.set("/api/a", `{"n":1}`)
  assert.deepEqual(await http.get("/api/a", d.unknown), { n: 1 })
  assert.deepEqual(await http.get("/api/a", d.unknown), { n: 1 }, "the 304's answer is the one held")
  b.answers.set("/api/a", `{"n":2}`)
  assert.deepEqual(await http.get("/api/a", d.unknown), { n: 2 })
  assert.deepEqual(await http.get("/api/a", d.unknown), { n: 2 })
  const tags = b.sent.map(([, t]) => t)
  assert.equal(tags[0], null, "nothing held, no tag")
  assert.notEqual(tags[1], null)
  assert.equal(tags[2], tags[1], "the first answer's tag, until a new answer came")
  assert.notEqual(tags[3], tags[2], "the moved answer's tag is sent after it")
  assert.equal(tags[3], http.held("/api/a"))
})

test("two decodings of one held answer share nothing a caller could change", async () => {
  const b = box()
  b.answers.set("/api/shared", `{"list":[1,2]}`)
  const first = await http.get("/api/shared", d.unknown)
  const again = await http.get("/api/shared", d.unknown)
  assert.notEqual(first, again)
  assert.deepEqual(first, again)
})

test("only the most recently asked paths are held, and a path let go is asked with no tag", async () => {
  const b = box()
  for (let i = 0; i <= http.KEPT; i++) await http.get(`/api/p${i}`, d.unknown)
  assert.equal(http.held("/api/p0"), null)
  assert.notEqual(http.held(`/api/p${http.KEPT}`), null)
  b.sent.splice(0)
  await http.get("/api/p0", d.unknown)
  assert.deepEqual(b.sent, [["/api/p0", null]])
})

test("the loader's first read is the app's first answer of its path, taken once", async () => {
  const b = box()
  b.answers.set("/api/life/fold", `{"entries":[]}`)
  primed.start("/api/life/fold", b.fetch)
  assert.deepEqual(await http.get("/api/life/fold", d.unknown), { entries: [] })
  assert.equal(b.sent.length, 1, "asked once, by the loader")
  await http.get("/api/life/fold", d.unknown)
  assert.equal(b.sent.length, 2, "the next ask is the app's own")
  assert.notEqual(b.sent[1]?.[1], null, "with the tag of the loader's answer")
})

// Why a read failed, "" when it did not.
const failed = (p: Promise<unknown>): Promise<string> => p.then(() => "", (e: unknown) => String(e))

test("a loader's read the box refused is asked again by the app, and one that failed fails the app's ask", async () => {
  const b = box()
  b.answers.set("/api/x/fold", `{"entries":[1]}`)
  primed.start("/api/x/fold", () => Promise.resolve(new Response("{}", { status: 500 })))
  assert.deepEqual(await http.get("/api/x/fold", d.unknown), { entries: [1] })
  assert.equal(b.sent.length, 1, "asked again, signed")
  primed.start("/api/x/fold", () => Promise.reject(new Error("offline")))
  assert.match(await failed(http.get("/api/x/fold", d.unknown)), /offline/)
  assert.equal(b.sent.length, 1)
})

test("a loader's read the box never answers ends in the read's own timeout, and the next ask is the app's", async () => {
  const b = box()
  b.answers.set("/api/y/fold", `{"entries":[2]}`)
  const silent = (_: RequestInfo | URL, init?: RequestInit): Promise<Response> =>
    new Promise((_, no) => init?.signal?.addEventListener("abort", () => no(init.signal?.reason)))
  primed.start("/api/y/fold", silent, 50)
  assert.match(await failed(http.get("/api/y/fold", d.unknown)), /the box did not answer in 30 s/)
  assert.deepEqual(await http.get("/api/y/fold", d.unknown), { entries: [2] })
})
