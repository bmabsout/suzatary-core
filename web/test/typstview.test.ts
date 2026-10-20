// The kit and the list view, compiled by the tab's own typst-wasm over generated states, node by node as the
// tab's fold compiles them: a wave of nodes is each node compiled alone, every page passes the box's tripwire,
// and prints every text it was handed as that text.

import { test } from "node:test"
import assert from "node:assert/strict"
import { createRequire } from "node:module"
import { readFileSync } from "node:fs"
import { pathToFileURL } from "node:url"

import type { Core } from "../src/prodrome/core"
import memo from "../src/algebra/memo"
import compiler from "../src/typst/compiler"
import intent from "../src/typst/intent"
import viewstate from "../src/typst/state"
import viewtree from "../src/typst/tree"
import fulfillment from "../src/todo/fulfillment"
import { HOSTILE, entry, forAll } from "./gen"

type Project = { set: (key: string, text: string) => string | undefined; compile: (main: string, state: string) => string }
type Glue = { initSync: (m: { module: BufferSource }) => unknown; Project: { new (): Project; restricted: () => Project } }

const built = (name: string): string => {
  const v = process.env[name]
  if (v === undefined || v === "") throw new Error(`${name} is unset — run scripts/web-test.sh`)
  return v
}

// The two glues are a build output with no types of their own here; each is named by the type the tab reads it at.
const core = createRequire(import.meta.url)(`${built("SUZATARY_PRODROME_WASM")}/nodejs/prodrome.js`) as Core
const glue = (await import(pathToFileURL(`${built("SUZATARY_TYPST_WASM")}/web/typst.js`).href)) as Glue
glue.initSync({ module: readFileSync(`${built("SUZATARY_TYPST_WASM")}/web/typst_bg.wasm`) })
const named: memo.Namer = (own, kids) => core.memo_name(own, kids)

// The packages the kit imports, keyed as the tab is handed them (scripts/typst-packages.mjs).
const packages = JSON.parse(readFileSync(built("SUZATARY_TYPST_PACKAGES"), "utf8")) as Record<string, string>

const world = new glue.Project()
for (const [key, text] of Object.entries(packages)) world.set(key, text)
for (const [key, file] of [["/kit.typ", "web/typst/kit.typ"], ["/list.typ", "web/typst/list.typ"]] as const) world.set(key, readFileSync(file, "utf8"))

const body = (html: string): string => html.slice(html.indexOf("<body>") + "<body>".length, html.lastIndexOf("</body>"))

// One compile of the view over a wave of nodes, as the fold's algebra compiles one.
const wave = (owns: readonly string[]): string =>
  compiler.fold(compiler.read(world.compile("/list.typ", `[${owns.join(",")}]`)), { html: body, failed: es => assert.fail(es.join("; ")) })

// The page a state draws, as one string: every node compiled alone, its children placed before its element closes,
// and a run drawn as its children.
const compile = (s: viewstate.State): string =>
  memo.plain(
    memo.pointwise<viewtree.Part, string>("page", (v, kids) => (v.node.type === "Run" ? kids.join("") : wave([v.own]).replace(/<\/[a-z0-9]+>$/, m => kids.join("") + m))),
    viewtree.of(s, named),
  )

const nodes = (t: viewtree.Tree): viewtree.Part[] => [t.v, ...t.kids.flatMap(nodes)]

// typst's own escaping of element text: `&` and `<`, and `>` left alone.
const escaped = (t: string): string => t.replace(/&/g, "&amp;").replace(/</g, "&lt;")

const held = (es: ReturnType<typeof entry>[], open: string): viewstate.Held => ({ entries: es, at: "2026-10-02T09:00:00Z", chain: "life", open, writes: true })

test("a wave of nodes compiles to each node compiled alone, in order", () =>
  forAll(20, r => {
    const es = r.array(r.int(10), () => entry(r))
    const owns = nodes(viewtree.of(viewstate.of(core, held(es, es.length > 0 && r.bool() ? (es[0]?.todo ?? "") : "")), named)).filter(p => p.node.type !== "Run").map(p => p.own)
    assert.equal(wave(owns), owns.map(o => wave([o])).join(""))
  }))

test("every generated state compiles, and the box's tripwire passes the page", () =>
  forAll(40, r => {
    const es = r.array(r.int(10), () => entry(r))
    const html = compile(viewstate.of(core, held(es, es.length > 0 && r.bool() ? (es[0]?.todo ?? "") : "")))
    assert.equal(core.suspicious(html), undefined)
  }))

test("a hostile title is printed as its text, never read as markup or HTML", () => {
  const title = HOSTILE.join(" ")
  const row: viewstate.Row = { ref: "todo:life/hostile", title: viewstate.prose(title), category: "home", value: 0.3, unconfirmed: false, complete: true }
  const html = compile({ schema: viewstate.SCHEMA, at: "", chain: "life", rows: [row], open: { type: "Todo", row, created: "", detail: viewstate.prose(title), why: [viewstate.prose(title)] } })
  assert.equal(core.suspicious(html), undefined)
  assert.ok(html.includes(`<span class="ttitle">${escaped(title)}</span>`), html)
  assert.ok(!/<(script|strong|a|code|pre)\b/.test(html), html)
})

test("rows are least fulfilled first, a row with no number last", () => {
  const row = (ref: string, value: number | null): viewstate.Row => ({ ref, title: viewstate.prose(ref), category: "", value, unconfirmed: false, complete: false })
  const html = compile({ schema: viewstate.SCHEMA, at: "", chain: "life", rows: [row("todo:life/c", null), row("todo:life/b", 0.9), row("todo:life/a", 0.1)], open: { type: "None" } })
  const order = [...html.matchAll(/data-intent="open (todo:life\/[a-z])"/g)].map(m => m[1])
  assert.deepEqual(order, ["todo:life/a", "todo:life/b", "todo:life/c"])
})

test("every control a compiled page declares is an intent the shell parses", () =>
  forAll(40, r => {
    const es = r.array(1 + r.int(10), () => entry(r))
    const html = compile(viewstate.of(core, held(es, es[0]?.todo ?? "")))
    for (const m of html.matchAll(/data-intent="([^"]*)"/g)) assert.notEqual(intent.parse(m[1] ?? ""), null, m[1])
  }))

const markup = (source: string): viewstate.Text => ({ type: "Markup", source })
const opened = (title: viewstate.Text, detail: viewstate.Text = viewstate.prose("")): viewstate.State => {
  const row: viewstate.Row = { ref: "todo:life/marked", title, category: "home", value: 0.4, unconfirmed: false, complete: true }
  return { schema: viewstate.SCHEMA, at: "", chain: "life", rows: [row], open: { type: "Todo", row, created: "", detail, why: [] } }
}

test("confirmed markup is read as markup: the row keeps a link's words, the open todo keeps the link", () => {
  const html = compile(opened(markup('File the *permit* renewal at #link("https://example.invalid/permits")[the office]')))
  assert.equal(core.suspicious(html), undefined)
  const button = html.match(/<button[^>]*class="topen"[^>]*>(.*?)<\/button>/)?.[1] ?? ""
  assert.ok(button.includes("<strong>permit</strong>") && button.includes("the office") && !button.includes("<a "), button)
  assert.ok(/<h2 class="tpanetitle">.*<a href="https:\/\/example.invalid\/permits">the office<\/a>/.test(html), html)
})

test("the same text as prose is printed as its source", () => {
  const source = "File the *permit* renewal"
  const html = compile(opened(viewstate.prose(source)))
  assert.ok(html.includes(`<span class="ttitle">${source}</span>`) && !html.includes("<strong>"), html)
})

test("confirmed markup that makes a script, a handler or a javascript: link is refused by the box's tripwire", () => {
  for (const source of ['#html.elem("script")[alert(1)]', '#html.elem("span", attrs: (onclick: "alert(1)"))[x]', '#link("javascript:alert(1)")[go]']) {
    const html = compile(opened(markup(source)))
    assert.notEqual(core.suspicious(html), undefined, source)
  }
})

test("law: the colour the kit gives a value is the one the tab's own pies are drawn in, the package's scale", () =>
  forAll(10, r => {
    const vs = r.array(1 + r.int(30), () => r.int(1_000_001) / 1_000_000)
    world.set("/colours.typ", `#import "/kit.typ": colour\n#(${vs.join(", ")},).map(colour).join(";")`)
    const printed = compiler.fold(compiler.read(world.compile("/colours.typ", "{}")), { html: body, failed: es => assert.fail(es.join("; ")) })
    const drawn = [...printed.matchAll(/oklch\(([\d.]+)% ([\d.]+) ([\d.]+)\)/g)].map(m => ({ l: Number(m[1]), c: Number(m[2]), h: Number(m[3]) }))
    assert.equal(drawn.length, vs.length, printed)
    for (const [i, v] of vs.entries()) {
      const [kit, tab] = [drawn[i], fulfillment.lch(v)]
      assert.ok(kit !== undefined)
      assert.ok(Math.abs(kit.l - tab.l) < 0.01 && Math.abs(kit.c - tab.c) < 1e-4 && Math.abs(kit.h - tab.h) < 0.01, `${v}: ${JSON.stringify(kit)} ${JSON.stringify(tab)}`)
    }
  }))

// The closed world every writer's markup is admitted in: typst's library with `html` and `plugin` removed. The
// package's marks and callouts compile there too, so adopting it widened nothing that world holds; its pie, which
// reaches for `html.frame` wherever a `target` exists, is refused there rather than granted.
test("the packages compile in the restricted world, its page passes the box's tripwire, and its html stays out", () => {
  const closed = glue.Project.restricted()
  for (const [key, text] of Object.entries(packages)) assert.equal(closed.set(key, text), undefined, key)
  const uses = "#sep[a][b][c]\n#capsule-rule()\n#note(title: [A note])[its body]\n#notice[a notice]\n#panel[a panel]\n#percent(0.42) #fulfillment-color(0.42)"
  closed.set("/main.typ", `#import "@local/typst-design:0.1.0": *\n#show: callout-rules\n${uses}`)
  const html = compiler.fold(compiler.read(closed.compile("/main.typ", "{}")), { html: h => h, failed: es => assert.fail(es.join("; ")) })
  assert.equal(core.suspicious(html), undefined)
  closed.set("/pie.typ", `#import "@local/typst-design:0.1.0": pie\n#pie(0.42)`)
  compiler.fold(compiler.read(closed.compile("/pie.typ", "{}")), {
    html: h => assert.fail(h),
    failed: es => assert.ok(es.some(e => e.includes("does not contain `html`")), es.join("; ")),
  })
})
