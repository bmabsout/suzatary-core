/**
 * THE TYPST LIST, IN A BROWSER: the open list compiled in the tab by typst-wasm from a typed state
 * (docs/APP-AS-READING-PLAN-2026-10-02.md, stages 0 and 4), beside today's list.
 *
 * Laws: no horizontal overflow at 390 and 1280 px, every `data-intent` the compiled page holds parses,
 * a crafted title renders as its literal text, a long title takes three lines beside a narrow mark and is
 * whole in the open todo, the owner's markup renders while the agent's reads as its source, confirmed
 * markup that makes a script, a handler or a `javascript:` link is refused by the tripwire, and Done
 * completes through the existing act path, and the core and typst-wasm compile while they stream.
 * Then the numbers (stage 4, the draw as one memoised fold): the modules' and the bundle's bytes on a first
 * visit's wire, time to first view, the page's redraw after an Open intent, and the draw's first draw, one-row
 * change and Open intent over 53 and 250 rows, at no throttling and at CPU throttling 4 and 6, the median of
 * ten runs each, printed and written to `$SUZATARY_SHOTS/typst-measure.json` beside the screenshots.
 */

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { BASE, DESKTOP, PHONE, WRITE_BASE, row, suite, untilReplica, pickCategory } from "./harness";
import type { Entry } from "../src/wire/generated/wire";
import intent from "../src/typst/intent";
import viewstate from "../src/typst/state";
import viewtree from "../src/typst/tree";

const OUT = process.env["SUZATARY_SHOTS"] ?? join(tmpdir(), "suzatary-shots");
mkdirSync(OUT, { recursive: true });

const app = await suite("typstview", PHONE);

const TITLE = '#strong[x] <script>alert(1)</script> ] #link("javascript:alert(1)")[go] ```raw```';
// The add sheet takes markup, so a title meant literally is typed escaped; it is stored as escaped prose, which the
// view reads back into the prose it was.
const typed = (title: string): string => title.replace(/[\\[\]#$@*_<>`]/g, (c) => `\\${c}`);
// The id authoring derives: the body's letters and digits, lowercased, cut at 24 (`authoring::slug`).
const ID = TITLE.toLowerCase().replace(/[^a-z0-9]+/g, "").slice(0, 24);
// A title as long as the owner's longest: several sentences, which a phone would otherwise print over a dozen lines.
const LONG =
  "Call the registry about the permit renewal before Friday. They said the form changed in March and the old one will be refused at the counter. Ask whether the fee can be paid online, and if not, which office still takes cheques. Bring the approval notice and a second copy of the deed.";
const LONG_ID = LONG.toLowerCase().replace(/[^a-z0-9]+/g, "").slice(0, 24);
// The owner wrote the first in markup, the agent the second (`suzatary_core::cast`).
const OWNERS = { id: "file-permit", body: "File the *permit* renewal", strong: "permit" };
const AGENTS = { id: "email-harrow", body: "Email *Harrow* about the deed" };
const HOSTILE_MARKUP = ['#html.elem("script")[alert(1)]', '#html.elem("span", attrs: (onclick: "alert(1)"))[x]', '#link("javascript:alert(1)")[go]'];
const RATES = [1, 4, 6] as const;
const RUNS = 10;

const median = (xs: readonly number[]): number => {
  const s = [...xs].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 === 0 ? ((s[m - 1] ?? 0) + (s[m] ?? 0)) / 2 : (s[m] ?? 0);
};

const drawn = `document.querySelector(".typstframe .tview") !== null`;

async function typst(hash = "#/typst"): Promise<void> {
  await app.page.navigate(`${WRITE_BASE}/${hash}`);
  await app.page.waitFor(drawn, { timeout: 30000 });
}

async function add(title: string, id: string): Promise<void> {
  await app.open(WRITE_BASE);
  await untilReplica(app.page);
  await app.page.click(".addsheet .pickerhandle");
  await app.page.waitFor(`document.querySelector(".addsheet.open") !== null`);
  await app.page.waitFor(`document.activeElement?.className.includes("draftbody") === true`);
  await app.page.type(".draftbody", typed(title));
  await pickCategory(app.page, "home");
  await app.page.click(".addbtn");
  await app.page.waitFor(`document.querySelector('${row(id)}') !== null`, { timeout: 8000 });
}

// The Typst list's row for a todo, as an expression the page evaluates.
const trow = (id: string): string =>
  `[...document.querySelectorAll(".typstframe .trow")].find((r) => r.querySelector('[data-intent="open todo:life/${id}"]') !== null)`;

app.scene("a crafted title renders as its literal text, and nothing in it becomes an element", async () => {
  await add(TITLE, ID);
  await typst();
  const at = trow(ID);
  await app.page.waitFor(`${at} !== undefined`, { timeout: 8000 });
  assert.equal(await app.page.evaluate<string>(`${at}.querySelector(".ttitle").textContent`), TITLE);
  assert.equal(
    await app.page.evaluate<number>(`${at}.querySelectorAll("script, strong, a, code, pre").length`),
    0,
  );
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll(".typstframe script, .typstframe a").length`), 0);
});

app.scene("a long title shows three lines in the list, beside a narrow mark, and whole in the open todo", async () => {
  await add(LONG, LONG_ID);
  for (const size of [PHONE, DESKTOP]) {
    await app.resize(size);
    await typst();
    const at = trow(LONG_ID);
    await app.page.waitFor(`${at} !== undefined`, { timeout: 8000 });
    const shown = await app.page.evaluate<{ text: string; name: string; lines: number; clipped: boolean; mark: number; title: number; row: number; tall: number; high: number }>(`(() => {
      const r = ${at};
      const t = r.querySelector(".ttitle");
      const line = parseFloat(getComputedStyle(t).lineHeight);
      return {
        text: t.textContent,
        name: r.querySelector(".topen").textContent,
        lines: Math.round(t.getBoundingClientRect().height / line),
        clipped: t.scrollHeight > t.clientHeight,
        mark: r.querySelector(".tmark").getBoundingClientRect().width,
        title: t.getBoundingClientRect().width,
        row: r.getBoundingClientRect().width,
        tall: r.getBoundingClientRect().height,
        high: t.getBoundingClientRect().height,
      };
    })()`);
    assert.equal(shown.text, LONG, "the list's title holds the whole text; the clamp only hides it");
    assert.ok(shown.name.includes(LONG), "the row's control is named by the whole title");
    assert.ok(shown.lines <= 3, `${size[0]} px: the title takes ${shown.lines} lines`);
    if (size === PHONE) assert.ok(shown.clipped, "at 390 px the long title is clamped");
    assert.ok(shown.mark <= 40, `${size[0]} px: the mark is ${shown.mark} px wide`);
    assert.ok(shown.title >= 0.6 * shown.row, `${size[0]} px: the title has ${shown.title} of the row's ${shown.row} px`);
    // Every row a thumb scans is its text and its padding, nothing more.
    assert.ok(shown.tall <= shown.high + 20, `${size[0]} px: a ${shown.high} px title in a ${shown.tall} px row`);
    await app.page.screenshot(join(OUT, `typst-long-${size[0]}.png`));
  }
  await app.resize(PHONE);
  await typst(`#/typst/${LONG_ID}`);
  await app.page.waitFor(`document.querySelector(".typstframe .tinplace .tpanetitle") !== null`);
  const whole = await app.page.evaluate<{ text: string; clipped: boolean }>(
    `(() => { const h = document.querySelector(".typstframe .tinplace .tpanetitle"); return { text: h.textContent, clipped: h.scrollHeight > h.clientHeight }; })()`,
  );
  assert.deepEqual(whole, { text: LONG, clipped: false });
  await app.page.evaluate(`(window.scrollTo(0, document.querySelector(".typstframe .trow:has(> .tinplace)").getBoundingClientRect().top + window.scrollY - 64), true)`);
  await app.page.screenshot(join(OUT, "typst-open-390.png"));
});

// On the read-only fixture: the write fixture's `file-permit` is resolved by `sync.test.ts` before this suite runs.
app.scene("rendered means confirmed: the owner's markup renders, the agent's reads as its source", async () => {
  await app.page.navigate(`${BASE}/#/typst`);
  await app.page.waitFor(drawn, { timeout: 30000 });
  const title = (id: string): string => `${trow(id)}.querySelector(".ttitle")`;
  await app.page.waitFor(`${trow(OWNERS.id)} !== undefined && ${trow(AGENTS.id)} !== undefined`, { timeout: 8000 });
  assert.equal(await app.page.evaluate<string>(`${title(OWNERS.id)}.querySelector("strong")?.textContent ?? ""`), OWNERS.strong);
  assert.equal(await app.page.evaluate<string>(`${title(OWNERS.id)}.textContent`), OWNERS.body.replace(/\*/g, ""));
  assert.equal(await app.page.evaluate<string>(`${title(AGENTS.id)}.textContent`), AGENTS.body);
  assert.equal(await app.page.evaluate<number>(`${title(AGENTS.id)}.querySelectorAll("*").length`), 0);
});

// Every node of a state's tree that draws, as the fold hands the view a wave; names do not matter to a compile.
const nodes = (s: viewstate.State): viewtree.Node[] => {
  const all = (t: viewtree.Tree): viewtree.Node[] => [t.v.node, ...t.kids.flatMap(all)];
  return all(viewtree.of(s, (own, kids) => `${own}${kids.join("")}`)).filter((n) => n.type !== "Run");
};

// A confirmed text that makes one of these is refused, like any node: the tab's own typst-wasm compiles the kit's
// list over the nodes of a state holding it, and the core's tripwire, the box's, reads them before any door would.
app.scene("confirmed markup that makes a script, a handler or a javascript: link is refused by the tripwire", async () => {
  await app.page.navigate(`${BASE}/#/typst`);
  await app.page.waitFor(drawn, { timeout: 30000 });
  const fold = await app.page.evaluate<{ entries: Entry[]; at: string }>(`fetch("/api/fold").then((r) => r.json())`);
  const base = viewstate.of({ prose: (m) => m.replace(/\\(.)/g, "$1") }, { entries: fold.entries, at: fold.at, chain: "life", open: OWNERS.id, writes: true });
  const marked = (source: string): viewstate.State => {
    const t: viewstate.Text = { type: "Markup", source };
    const at = (r: viewstate.Row): viewstate.Row => (r.ref === `todo:life/${OWNERS.id}` ? { ...r, title: t } : r);
    return { ...base, rows: base.rows.map(at), open: base.open.type === "Todo" ? { ...base.open, row: at(base.open.row) } : base.open };
  };
  assert.equal(base.open.type, "Todo", "the owner's todo is open on the read-only fixture");
  const verdicts = await app.page.evaluate<(string | null)[]>(`(async () => {
    const g = await import(new URL("./b/dev/typst.js", location.href).href);
    await g.default();
    const c = await import(new URL("./b/dev/prodrome.js", location.href).href);
    await c.default({ module_or_path: new URL("./b/dev/prodrome_bg.wasm", location.href).href });
    const read = (f) => fetch(new URL("./b/dev/typst/" + f, location.href).href).then((r) => r.text());
    const [kit, list, packages] = await Promise.all(["kit.typ", "list.typ", "packages.json"].map(read));
    return ${JSON.stringify([HOSTILE_MARKUP.map(marked), [base]].flat().map(nodes))}.map((wave) => {
      const a = JSON.parse(g.compile_html(list, JSON.stringify({ ...JSON.parse(packages), "/kit.typ": kit, "/list.typ": list, "/state.json": JSON.stringify(wave) })));
      if (a.html === null) throw new Error(JSON.stringify(a.diagnostics));
      return c.suspicious(a.html) ?? null;
    });
  })()`);
  const control = verdicts.pop();
  assert.equal(control, null, "the same page with the owner's own title passes");
  for (const [k, v] of verdicts.entries()) assert.notEqual(v, null, HOSTILE_MARKUP[k]);
});

app.scene("no page overflows sideways at 390 or 1280, and every control's intent parses", async () => {
  for (const size of [PHONE, DESKTOP]) {
    await app.resize(size);
    await typst();
    await typst(`#/typst/${ID}`);
    await app.page.waitFor(`document.querySelector(".typstframe .tpane") !== null`);
    const [scroll, client] = await app.page.evaluate<[number, number]>(
      `[document.documentElement.scrollWidth, document.documentElement.clientWidth]`,
    );
    assert.ok(scroll <= client, `${size[0]} px: scrollWidth ${scroll} > clientWidth ${client}`);
    const intents = await app.page.evaluate<string[]>(`[...document.querySelectorAll("[data-intent]")].map((n) => n.getAttribute("data-intent"))`);
    assert.ok(intents.length > 0);
    for (const text of intents) assert.notEqual(intent.parse(text), null, text);
    // At 1280 the list stands beside the open todo; at 390 one column holds both.
    const columns = await app.page.evaluate<number>(`getComputedStyle(document.querySelector(".tview")).gridTemplateColumns.split(" ").length`);
    assert.equal(columns, size === PHONE ? 1 : 2, `${size[0]} px: ${columns} columns`);
    await app.page.screenshot(join(OUT, `typst-list-${size[0]}.png`));
    await app.page.navigate(`${WRITE_BASE}/${size === PHONE ? "#/" : `#/todo/${ID}`}`);
    await app.page.waitFor(`document.querySelectorAll(".tree .row").length > 0`);
    await app.page.screenshot(join(OUT, `today-list-${size[0]}.png`));
  }
  await app.resize(PHONE);
});

app.scene("opening a todo on a phone moves nothing above the row that was tapped", async () => {
  await app.resize(PHONE);
  await typst();
  const top = (ref: string): string =>
    `(() => { const r = document.querySelector('.typstframe .trow [data-intent="open ${ref}"]').closest(".trow").getBoundingClientRect(); return r.top + window.scrollY; })()`;
  const ref = await app.page.evaluate<string>(`document.querySelectorAll(".typstframe .topen")[1].getAttribute("data-intent").slice("open ".length)`);
  const before = await app.page.evaluate<number>(top(ref));
  await app.page.click(`.typstframe [data-intent="open ${ref}"]`);
  await app.page.waitFor(`document.querySelector('.typstframe .trow:has(> .tinplace) [data-intent="open ${ref}"]') !== null`);
  assert.equal(await app.page.evaluate<number>(top(ref)), before);
  assert.equal(
    await app.page.evaluate<boolean>(`document.querySelector(".typstframe .trow > .tinplace") !== null`),
    true,
    "the open todo reads in place, inside its row",
  );
});

app.scene("scroll and focus survive a redraw, an Open intent computes only its row's spine and the pane", async () => {
  await app.resize([390, 480]);
  await typst();
  const opens = await app.page.evaluate<string[]>(`[...document.querySelectorAll(".typstframe .trow > .topen")].map((b) => b.getAttribute("data-intent"))`);
  assert.ok(opens.length >= 6, `${opens.length} rows`);
  assert.ok(await app.page.evaluate<boolean>(`document.documentElement.scrollHeight > window.innerHeight`), "the page scrolls");
  const focused = opens[2] ?? "";
  const target = opens[opens.length - 1] ?? "";
  await app.page.evaluate(`(() => {
    window.scrollTo(0, (document.documentElement.scrollHeight - window.innerHeight) / 2);
    document.querySelector('.typstframe [data-intent="${focused}"]').focus({ preventScroll: true });
    window.__focused = document.activeElement;
    window.__rows = [...document.querySelectorAll(".typstframe .trow")];
    window.__y = window.scrollY;
    window.__draws = performance.getEntriesByName("typst:draw").length;
    return true;
  })()`);
  await app.page.evaluate(`(location.hash = "#/typst/${target.slice("open todo:life/".length)}", true)`);
  await app.page.waitFor(`document.querySelector('.typstframe .trow:has(> .tinplace) [data-intent="${target}"]') !== null`);
  const after = await app.page.evaluate<{ same: boolean; y: number; was: number; kept: number; others: number }>(`(() => {
    const now = [...document.querySelectorAll(".typstframe .trow")];
    const others = window.__rows.filter((e) => e.querySelector('[data-intent="${target}"]') === null);
    return { same: document.activeElement === window.__focused, y: window.scrollY, was: window.__y, kept: others.filter((e) => now.includes(e)).length, others: others.length };
  })()`);
  assert.ok(after.same, "the focused control is the element it was");
  assert.equal(after.y, after.was, "the page did not scroll");
  assert.equal(after.kept, after.others, "every other row is the element it was");
  const draws = await app.page.evaluate<{ detail: { computed: Computed[]; compiled: number } }[]>(`${entries("typst:draw")}.slice(window.__draws)`);
  const moved = draws.map((d) => d.detail).filter((d) => d.computed.length > 0);
  assert.equal(moved.length, 1, "one redraw moved anything");
  const computed = moved[0]?.computed ?? [];
  assert.deepEqual(computed.filter((c) => c.type === "Row").map((c) => `open ${c.key ?? ""}`), [target]);
  assert.deepEqual(computed.filter((c) => c.type !== "Row" && c.type !== "Run").map((c) => c.type).sort(), ["InPlace", "List", "Pane", "Rows", "View"]);
  assert.equal(moved[0]?.compiled, 2, "what the open todo shows, in place and in the pane, and nothing else");
  // The control pressed keeps the focus: its row stays the element it was, the open todo placed inside it.
  const other = opens[3] ?? "";
  await app.page.click(`.typstframe [data-intent="${other}"]`);
  await app.page.waitFor(`document.querySelector('.typstframe .trow:has(> .tinplace) [data-intent="${other}"]') !== null`);
  assert.equal(await app.page.evaluate<string | null>(`document.activeElement?.isConnected ? document.activeElement.getAttribute("data-intent") : null`), other);
  await app.resize(PHONE);
});

app.scene("Done is an intent: the shell parses it and completes the todo through the act path", async () => {
  await typst();
  await untilReplica(app.page);
  await app.page.click(`.typstframe [data-intent="complete todo:life/${ID}"]`);
  await app.page.waitFor(`document.querySelector('.typstframe [data-intent="open todo:life/${ID}"]') === null`, { timeout: 15000 });
  await app.page.waitFor(
    `fetch("/api/fold").then((r) => r.json()).then((b) => b.entries.some((e) => e.todo === "${ID}" && e.state === "completed"))`,
    { timeout: 15000 },
  );
});

type Detail = Readonly<Record<string, number>>;
const entries = (name: string): string => `performance.getEntriesByName(${JSON.stringify(name)}).map((e) => ({ duration: e.duration, end: e.startTime + e.duration, detail: e.detail }))`;

// The page's own draw over states this scene hands it (`typstbench.ts`), bundled for the browser here: the same
// tree, fold, algebra, door and placement, on a host of its own. Evaluated, never imported from a `blob:`, which
// the page's policy refuses as it should. Its modules are named as the dev build's, which the app's own resolve to
// beside `app.js`.
const BENCH = execFileSync(
  "esbuild",
  ["web/e2e/typstbench.ts", "--bundle", "--format=iife", "--global-name=typstbench", "--target=es2022", "--define:__DEV__=false", "--external:./typst.js", "--external:./prodrome.js", "--log-level=error"],
  { encoding: "utf8" },
).replaceAll(`"./typst.js"`, `"./b/dev/typst.js"`).replaceAll(`"./prodrome.js"`, `"./b/dev/prodrome.js"`);

// Installs the bench and a drawer over the kit the page serves, at `window.__bench` (a fresh one each call).
const bench = `(async () => {
  window.typstbench ??= (() => { ${BENCH}; return typstbench; })();
  if (window.__glue === undefined) {
    const g = await import(new URL("./b/dev/typst.js", location.href).href);
    await g.default({ module_or_path: new URL("./b/dev/typst_bg.wasm", location.href).href });
    const c = await import(new URL("./b/dev/prodrome.js", location.href).href);
    await c.default({ module_or_path: new URL("./b/dev/prodrome_bg.wasm", location.href).href });
    const read = (f) => fetch(new URL("./b/dev/typst/" + f, location.href).href).then((r) => r.text());
    const [kit, list, packages] = await Promise.all(["kit.typ", "list.typ", "packages.json"].map(read));
    window.__glue = { g, c, files: { ...JSON.parse(packages), "/kit.typ": kit, "/list.typ": list } };
  }
  window.__bench?.done();
  window.__bench = window.typstbench.drawer(window.__glue.g, window.__glue.c, window.__glue.files, "/list.typ");
  return true;
})()`;

type Computed = { type: string; key: string | null; depth: number };
type Redraw = { ms: number; tree: number; fold: number; place: number; compiled: number; computed: Computed[]; placed: { kept: number; refilled: number; made: number; moved: number } };
const redraw = (s: viewstate.State): string => `window.__bench.draw(${JSON.stringify(s)})`;

// The fixture's rows, `n` of them under distinct references, titles and values, as long a list as the owner's or longer.
const sized = (small: viewstate.State, n: number, salt: string): viewstate.State => ({
  ...small,
  rows: Array.from({ length: n }, (_, i) => {
    const r = small.rows[i % small.rows.length] ?? small.rows[0];
    if (r === undefined) throw new Error("the fixture has no rows");
    const title = viewstate.fold<viewstate.Text>(r.title, { Prose: (t) => viewstate.prose(`${t} ${salt}${i}`), Markup: (m) => ({ type: "Markup", source: `${m} ${salt}${i}` }) });
    return { ...r, ref: `todo:life/row${i}`, title, value: ((i * 37) % 100) / 100 };
  }),
});
const retitled = (s: viewstate.State, at: number, title: string): viewstate.State => ({ ...s, rows: s.rows.map((r, i) => (i === at ? { ...r, title: viewstate.prose(title) } : r)) });
const opened = (s: viewstate.State, at: number): viewstate.State => {
  const row = s.rows[at];
  return row === undefined ? s : { ...s, open: { type: "Todo", row, created: "2026-10-01", detail: viewstate.prose("what the todo says, in full"), why: [] } };
};

async function fixtureState(base: string): Promise<viewstate.State> {
  await app.page.navigate(`${base}/#/typst`);
  await app.page.waitFor(drawn, { timeout: 30000 });
  const fold = await app.page.evaluate<{ entries: Entry[]; at: string }>(`fetch("/api/fold").then((r) => r.json())`);
  return viewstate.of({ prose: (m) => m.replace(/\\(.)/g, "$1") }, { entries: fold.entries, at: fold.at, chain: "life", open: "", writes: true });
}

app.scene("a one-row change computes and places exactly that row's spine, and keeps every other row's element", async () => {
  const small = await fixtureState(BASE);
  const s = sized(small, 120, "spine");
  await app.page.evaluate(bench);
  const first = await app.page.evaluate<Redraw>(redraw(s));
  assert.equal(first.computed.filter((c) => c.type === "Row").length, s.rows.length, "the first draw computes every row");
  await app.page.evaluate(`(window.__rows = [...window.__bench.host.querySelectorAll(".trow")], true)`);
  const at = 57;
  const ref = s.rows[at]?.ref ?? "";
  const r = await app.page.evaluate<Redraw>(redraw(retitled(s, at, "a title that changed")));
  const kinds = r.computed.map((c) => c.type).sort();
  const runs = r.computed.filter((c) => c.type === "Run").length;
  assert.deepEqual(r.computed.filter((c) => c.type === "Row").map((c) => c.key), [ref], "the changed row, and no other");
  assert.deepEqual(kinds, ["List", "Row", "Rows", ...Array.from({ length: runs }, () => "Run"), "View"].sort());
  // The spine is one node per depth, from the root down to the row.
  const depths = r.computed.map((c) => c.depth).sort((a, b) => a - b);
  assert.deepEqual(depths, depths.map((_, i) => i));
  assert.ok(runs <= 2 * Math.ceil(Math.log2(s.rows.length)), `${runs} runs over ${s.rows.length} rows`);
  assert.equal(r.compiled, 1, "one node compiled: the changed row");
  assert.equal(r.placed.made, 1, "one element made");
  const kept = await app.page.evaluate<{ same: number; others: number; changed: string }>(`(() => {
    const now = [...window.__bench.host.querySelectorAll(".trow")];
    const others = window.__rows.filter((e) => e.querySelector('[data-intent="open ${ref}"]') === null);
    return { same: others.filter((e) => now.includes(e)).length, others: others.length, changed: now.find((e) => !window.__rows.includes(e))?.querySelector(".ttitle")?.textContent ?? "" };
  })()`);
  assert.equal(kept.same, kept.others, "every other row is the element it was");
  assert.equal(kept.changed, "a title that changed");
  // Placing means drawing: the redrawn DOM is what an empty cache draws for the same state.
  const redrawn = await app.page.evaluate<string>(`window.__bench.host.innerHTML`);
  await app.page.evaluate(bench);
  await app.page.evaluate<Redraw>(redraw(retitled(s, at, "a title that changed")));
  assert.equal(await app.page.evaluate<string>(`window.__bench.host.innerHTML`), redrawn);
  await app.page.evaluate(`(window.__bench.done(), true)`);
});

app.scene("the core and typst-wasm compile while they stream, as application/wasm, never from bytes read whole", async () => {
  const stop = await app.page.beforeEveryDocument(`(() => {
    const w = WebAssembly, streaming = w.instantiateStreaming, whole = w.instantiate;
    window.__wasm = { streamed: [], whole: 0 };
    w.instantiateStreaming = (r, imports) => Promise.resolve(r).then((res) => {
      window.__wasm.streamed.push(new URL(res.url).pathname + " " + res.headers.get("content-type"));
      return streaming.call(w, res, imports);
    });
    w.instantiate = (...args) => { window.__wasm.whole++; return whole.apply(w, args); };
  })()`);
  try {
    await app.page.cold(true);
    await app.page.navigate("about:blank");
    await typst();
    const got = await app.page.evaluate<{ streamed: string[]; whole: number }>(`window.__wasm`);
    assert.deepEqual([...got.streamed].sort(), ["/b/dev/prodrome_bg.wasm application/wasm", "/b/dev/typst_bg.wasm application/wasm"], "each under its build's name");
    assert.equal(got.whole, 0, "a module was instantiated from bytes read whole");
  } finally {
    await app.page.cold(false);
    await stop();
  }
});

app.scene("measured: bytes, first view, and the draw's first, one-row and open redraws, at throttling 1, 4 and 6", async () => {
  await app.page.cold(true);
  await app.page.navigate("about:blank");
  await typst();
  await app.page.cold(false);
  const sizes = (file: string) =>
    `(() => { const r = performance.getEntriesByType("resource").find((e) => e.name.endsWith("/${file}")); return { decoded: r?.decodedBodySize ?? 0, encoded: r?.encodedBodySize ?? 0 }; })()`;
  const bytes = {
    typst: await app.page.evaluate<{ decoded: number; encoded: number }>(sizes("typst_bg.wasm")),
    core: await app.page.evaluate<{ decoded: number; encoded: number }>(sizes("prodrome_bg.wasm")),
    bundle: await app.page.evaluate<{ decoded: number; encoded: number }>(sizes("app.js")),
  };
  const small = await fixtureState(WRITE_BASE);
  const SHAPES = [53, 250] as const;

  const results: Record<string, unknown>[] = [];
  for (const rate of RATES) {
    await app.page.throttle(rate);
    const first: { load: Detail; view: number; sinceNavigation: number; draw: number }[] = [];
    await app.page.cold(true);
    for (let k = 0; k < RUNS; k++) {
      await app.page.navigate("about:blank");
      await app.page.navigate(`${WRITE_BASE}/#/typst`);
      await app.page.waitFor(`performance.getEntriesByName("typst:first-view").length > 0`, { timeout: 120000 });
      const [load] = await app.page.evaluate<{ detail: Detail }[]>(entries("typst:load"));
      const [view] = await app.page.evaluate<{ duration: number; end: number }[]>(entries("typst:first-view"));
      const draws = (await app.page.evaluate<{ duration: number; end: number }[]>(entries("typst:draw"))).filter((d) => d.end <= (view?.end ?? 0));
      first.push({ load: load?.detail ?? {}, view: view?.duration ?? NaN, sinceNavigation: view?.end ?? NaN, draw: draws.reduce((t, d) => t + d.duration, 0) });
    }
    await app.page.cold(false);

    // The page's own redraws after an Open intent, pressed ten times on two rows: each moves two rows and the pane.
    await typst();
    const ids = await app.page.evaluate<string[]>(`[...document.querySelectorAll(".typstframe .topen")].slice(0, 2).map((b) => b.getAttribute("data-intent"))`);
    const before = await app.page.evaluate<number>(`performance.getEntriesByName("typst:draw").length`);
    for (let k = 0; k < RUNS; k++) {
      const text = ids[k % ids.length] ?? "";
      await app.page.click(`.typstframe [data-intent="${text}"]`);
      await app.page.waitFor(`performance.getEntriesByName("typst:draw").length > ${before + k} && document.querySelector('.trow:has(> .tinplace) [data-intent="${text}"]') !== null`, { timeout: 20000 });
    }
    const redraws = (await app.page.evaluate<{ duration: number; detail: Detail }[]>(entries("typst:draw"))).slice(before);

    // The draw over the owner's list's length and the spike's, on a drawer of its own: a first draw over titles typst
    // has never seen, a one-row change, an Open intent on a row never opened, and one alternating between two rows
    // as the page's does, which after the first two finds the whole page in the cache.
    const shapes: Record<string, unknown> = {};
    for (const n of SHAPES) {
      const firsts: Redraw[] = [];
      const ones: Redraw[] = [];
      const opens: Redraw[] = [];
      const reopens: Redraw[] = [];
      for (let k = 0; k < RUNS; k++) {
        await app.page.evaluate(bench);
        const s = sized(small, n, `r${rate}k${k}n`);
        firsts.push(await app.page.evaluate<Redraw>(redraw(s)));
        if (k === 0) {
          for (let j = 0; j < RUNS; j++) {
            ones.push(await app.page.evaluate<Redraw>(redraw(retitled(s, (j * 7) % n, `changed ${rate} ${j}`))));
          }
          await app.page.evaluate<Redraw>(redraw(s));
          for (let j = 0; j < RUNS; j++) opens.push(await app.page.evaluate<Redraw>(redraw(opened(s, (5 + 11 * j) % n))));
          for (let j = 0; j < RUNS; j++) reopens.push(await app.page.evaluate<Redraw>(redraw(opened(s, j % 2 === 0 ? 3 : 4))));
        }
      }
      await app.page.evaluate(`(window.__bench.done(), true)`);
      const summary = (rs: Redraw[]) => ({
        ms: median(rs.map((r) => r.ms)),
        tree: median(rs.map((r) => r.tree)),
        fold: median(rs.map((r) => r.fold)),
        place: median(rs.map((r) => r.place)),
        computed: median(rs.map((r) => r.computed.length)),
        compiled: median(rs.map((r) => r.compiled)),
        rowsComputed: median(rs.map((r) => r.computed.filter((c) => c.type === "Row").length)),
        made: median(rs.map((r) => r.placed.made)),
      });
      shapes[`rows${n}`] = { first: summary(firsts), one: summary(ones), open: summary(opens), reopen: summary(reopens) };
    }
    results.push({
      rate,
      fetch: median(first.map((f) => f.load["fetch"] ?? NaN)),
      instantiate: median(first.map((f) => f.load["instantiate"] ?? NaN)),
      firstDraws: median(first.map((f) => f.draw)),
      firstView: median(first.map((f) => f.view)),
      firstViewSinceNavigation: median(first.map((f) => f.sinceNavigation)),
      redraw: median(redraws.map((r) => r.duration)),
      redrawFold: median(redraws.map((r) => r.detail["fold"] ?? NaN)),
      redrawPlace: median(redraws.map((r) => r.detail["place"] ?? NaN)),
      redrawTree: median(redraws.map((r) => r.detail["tree"] ?? NaN)),
      redrawCompiled: median(redraws.map((r) => r.detail["compiled"] ?? NaN)),
      fixtureRows: small.rows.length,
      ...shapes,
    });
  }
  await app.page.throttle(1);
  const measured = { bytes, runs: RUNS, results };
  writeFileSync(join(OUT, "typst-measure.json"), JSON.stringify(measured, null, 2));
  console.log(JSON.stringify(measured, null, 2));
  const at4 = results.find((r) => r["rate"] === 4);
  assert.ok(at4 !== undefined && at4["rows250"] !== undefined, "the draw was measured at rate 4");
});
