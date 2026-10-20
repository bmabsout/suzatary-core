// The Typst view's state: plain data built from what the list holds, every text a string kept as written.

import { test } from "node:test"
import assert from "node:assert/strict"

import viewstate from "../src/typst/state"
import fulfillment from "../src/todo/fulfillment"
import { DEFAULT_FILTER, matches, preview } from "../src/todo/todos"
import { entry, forAll, rng } from "./gen"

// Escaped markup has every special behind a backslash; anything else a person wrote stays source.
const reads: viewstate.Reads = { prose: m => (/(^|[^\\])[#\[\]*_<>`$@]/.test(m) ? undefined : m.replace(/\\(.)/g, "$1")) }

const held = (seed: (n: number) => number, writes: boolean, open = ""): ((es: ReturnType<typeof entry>[]) => viewstate.Held) =>
  es => ({ entries: es, at: "2026-10-02T09:00:00Z", chain: "life", open: open === "" && es.length > 0 ? (es[seed(es.length)]?.todo ?? "") : open, writes })

test("the state is plain data: it prints and reads back as itself", () =>
  forAll(200, r => {
    const s = viewstate.of(reads, held(r.int, r.bool())(r.array(r.int(12), () => entry(r))))
    assert.deepEqual(JSON.parse(viewstate.printed(s)), s)
  }))

test("the rows are the list's default rows, each named by its printed reference", () =>
  forAll(200, r => {
    const es = r.array(r.int(12), () => entry(r))
    const s = viewstate.of(reads, held(r.int, true)(es))
    assert.deepEqual(s.rows.map(x => x.ref), es.filter(e => matches(e, DEFAULT_FILTER, s.at)).map(e => `todo:life/${e.todo}`))
  }))

test("where the box rendered nothing, a title is the body's text as written: markup a person wrote is kept, never read", () =>
  forAll(200, r => {
    const es = r.array(1 + r.int(8), () => entry(r))
    const s = viewstate.of({ prose: () => undefined }, held(r.int, true)(es))
    for (const x of s.rows) {
      const e = es.find(e => `todo:life/${e.todo}` === x.ref)
      const body = preview(e?.authored?.body ?? "")
      assert.deepEqual(x.title, viewstate.prose(body === "" ? (e?.todo ?? "") : body))
    }
  }))

test("escaped markup reaches the state as the prose it was escaped from", () => {
  const e = entry(rng(7))
  const authored = {
    rich: { body: null, detail: null, rationale: null, waiting_on: null, keys: { body: null, detail: null, rationale: null, waiting_on: null } },
    block: { kind: "todo" as const, name: "todo" }, created: "2026-10-01", body: "\\#strong\\[x\\] \\<script\\>", text: "#strong[x] <script>", detail: "", category: "home",
    waiting_on: "", rationale: [], spec: null, actor: "owner", at: "", source: null, subtodos: [],
  }
  const s = viewstate.of(reads, { entries: [{ ...e, state: "open", authored }], at: "", chain: "life", open: "", writes: true })
  assert.deepEqual(s.rows[0]?.title, viewstate.prose("#strong[x] <script>"))
})

test("nothing completes for a login that does not write", () =>
  forAll(200, r => {
    const s = viewstate.of(reads, held(r.int, false)(r.array(r.int(12), () => entry(r))))
    assert.ok(s.rows.every(x => !x.complete))
    assert.ok(s.open.type === "None" || !s.open.row.complete)
  }))

test("an open todo the fold does not hold opens nothing", () => {
  const s = viewstate.of(reads, { entries: [], at: "", chain: "life", open: "gone", writes: true })
  assert.deepEqual(s.open, { type: "None" })
})

test("a row's value is its entry's, as the store observed it", () =>
  forAll(200, r => {
    const es = r.array(r.int(12), () => ({ ...entry(r), value: r.bool(0.9) ? fulfillment.whole(r.int(1_000_001) / 1_000_000) : null }))
    const s = viewstate.of(reads, held(r.int, true)(es))
    assert.deepEqual(s.rows.map(x => x.value), es.filter(e => matches(e, DEFAULT_FILTER, s.at)).map(e => (typeof e.value === "number" ? e.value : null)))
  }))

test("a text is markup only where the box rendered it and it is not escaped prose; markup is kept byte for byte", () =>
  forAll(300, r => {
    const es = r.array(1 + r.int(8), () => {
      const e = entry(r)
      return e.authored === null ? e : { ...e, authored: { ...e.authored, rich: { ...e.authored.rich, body: r.bool() ? "<p>rendered</p>" : null } } }
    })
    const s = viewstate.of(reads, held(r.int, true)(es))
    for (const x of s.rows) {
      const e = es.find(e => `todo:life/${e.todo}` === x.ref)
      const body = e?.authored?.body ?? ""
      viewstate.fold(x.title, {
        Prose: () => undefined,
        Markup: source => {
          assert.equal(source, body)
          assert.notEqual(e?.authored?.rich.body ?? null, null)
          assert.equal(reads.prose(body), undefined)
        },
      })
      if ((e?.authored?.rich.body ?? null) === null) assert.equal(x.title.type, "Prose")
    }
  }))
