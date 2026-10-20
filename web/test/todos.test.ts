/**
 * The list, as a function.
 *
 * The home page's whole behaviour — which todos, in what order, under which
 * heading — is `todos.ts`, and these are the properties it must not lose. The
 * first one is the system's: **LOW MEANS URGENT**, ascending. A regression that
 * flipped it would look like a working app showing exactly the wrong todo
 * first, which is why it is pinned here as well as on the box, where
 * `site::urgency` and `suzatary todos open` order the same way.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import type { Authored, Entry, SpecNode } from "../src/wire/generated/wire";
import {
  ANY,
  CLAMP_CHARS,
  DEFAULT_FILTER,
  FORTNIGHT_DAYS,
  byUrgency,
  categoriesOf,
  expandable,
  headline,
  narrowing,
  is,
  isDated,
  isDecision,
  isDone,
  isUnconfirmed,
  isWaiting,
  kindsOf,
  listing,
  matches,
  matchesQuery,
  needsAttention,
  parseFilter,
  preview,
  printFilter,
  signature,
  subtodoCount,
  total,
} from "../src/todo/todos";
import cast from "./cast";

const { counsel, city } = cast.world;

/** A fixed instant, so a boundary test does not depend on when it ran. */
const NOW = "2026-09-13T00:00:00.000Z";

function decay(endDate: string): SpecNode {
  return { kind: "decay", start: 0.6, end: 0.05, endDate, leadUpHours: 336 };
}

function flat(value: number): SpecNode {
  return { kind: "flat", value };
}

function authored(over: Partial<Authored> = {}): Authored {
  return {
    rich: { body: null, detail: null, rationale: null, waiting_on: null, keys: { body: null, detail: null, rationale: null, waiting_on: null } },
    block: { kind: "todo", name: "todo" },
    created: "2026-08-30",
    body: `Reply to the ${counsel.name} thread`,
    text: `Reply to the ${counsel.name} thread`,
    detail: "",
    category: "study",
    waiting_on: "",
    rationale: [],
    spec: null,
    actor: "ada",
    at: "2026-09-01T09:15:00",
    source: null,
    subtodos: [],
    ...over,
  };
}

function entry(over: Partial<Entry> = {}): Entry {
  return {
    todo: "harrowreply",
    state: "open",
    states: ["open"],
    at: "",
    claimed: "",
    unconfirmed: false,
    recurring: false,
    value: 0.42,
    conflicts: {},
    authored: authored(),
    candidates: [],
    stream: [],
    ...over,
  };
}

// --- the ordering ------------------------------------------------------------

test("ascending fulfillment: the lowest number is the most urgent", () => {
  const urgent = entry({ todo: "a", value: 0.2 });
  const calm = entry({ todo: "b", value: 0.9 });
  assert.deepEqual([calm, urgent].sort(byUrgency), [urgent, calm]);
});

test("equal values break on the id, so a redraw never shuffles", () => {
  const b = entry({ todo: "b", value: 0.5 });
  const a = entry({ todo: "a", value: 0.5 });
  assert.deepEqual([b, a].sort(byUrgency).map((each) => each.todo), ["a", "b"]);
});

test("an unpriced todo sorts last, not first — no price is not a price of zero", () => {
  const unpriced = entry({ todo: "z", value: null });
  const priced = entry({ todo: "a", value: 0.05 });
  assert.deepEqual([unpriced, priced].sort(byUrgency).map((each) => each.todo), ["a", "z"]);
});

// --- filtering ---------------------------------------------------------------

test("done means completed OR cancelled", () => {
  assert.equal(isDone(entry({ state: "open" })), false);
  assert.equal(isDone(entry({ state: "completed" })), true);
  assert.equal(isDone(entry({ state: "cancelled" })), true);
});

test("the state filter partitions open from resolved, and `all` keeps both", () => {
  const open = entry({ todo: "o", state: "open" });
  const done = entry({ todo: "d", state: "completed" });
  assert.equal(matches(open, DEFAULT_FILTER, NOW), true);
  assert.equal(matches(done, DEFAULT_FILTER, NOW), false);
  assert.equal(matches(done, { ...DEFAULT_FILTER, state: "done" }, NOW), true);
  assert.equal(matches(open, { ...DEFAULT_FILTER, state: "done" }, NOW), false);
  for (const each of [open, done]) assert.equal(matches(each, { ...DEFAULT_FILTER, state: "all" }, NOW), true);
});

test("the empty category is a value, not an absence — it can be filtered TO", () => {
  const none = entry({ authored: authored({ category: "" }) });
  const study = entry({ authored: authored({ category: "study" }) });
  assert.equal(matches(none, { ...DEFAULT_FILTER, category: is("") }, NOW), true);
  assert.equal(matches(study, { ...DEFAULT_FILTER, category: is("") }, NOW), false);
  assert.equal(matches(none, { ...DEFAULT_FILTER, category: ANY }, NOW), true);
});

test("a todo with no Authored record still filters, on its empty facets", () => {
  const bare = entry({ authored: null, value: null });
  assert.equal(matches(bare, DEFAULT_FILTER, NOW), true);
  assert.equal(matches(bare, { ...DEFAULT_FILTER, category: is("study") }, NOW), false);
  assert.equal(headline(bare), bare.todo);
});

test("a line's headline is the core's words for its markup, never the markup", () => {
  const it = entry({ unconfirmed: true, authored: authored({ body: `Email *${counsel.name}* about the deed`, text: `Email ${counsel.name} about the deed` }) });
  assert.equal(headline(it), `Email ${counsel.name} about the deed`);
});

test("a filter says what narrows the list, and the default says nothing", () => {
  assert.deepEqual(narrowing(DEFAULT_FILTER), []);
  assert.deepEqual(narrowing({ ...DEFAULT_FILTER, query: "   " }), []);
  assert.deepEqual(narrowing({ ...DEFAULT_FILTER, state: "done", category: is("study") }), ["Done", "study"]);
  assert.deepEqual(narrowing({ ...DEFAULT_FILTER, category: is(""), attention: true, waiting: true, dated: true, query: " deed " }), [
    "uncategorised", "needs a look", "waiting", "dated", "“deed”",
  ]);
});

test("the search reads the id and the body, case-insensitively", () => {
  const it = entry({ todo: "harrowreply", authored: authored({ body: `Reply to the #emph[${counsel.name}] thread` }) });
  assert.equal(matchesQuery(it, ""), true);
  assert.equal(matchesQuery(it, "  "), true);
  assert.equal(matchesQuery(it, counsel.name.toUpperCase()), true);
  assert.equal(matchesQuery(it, "emph"), true);
  assert.equal(matchesQuery(it, city.toLowerCase()), false);
});

// --- the listing -------------------------------------------------------------

test("unpriced todos are shown under their own heading, never hidden", () => {
  const list = listing(
    [entry({ todo: "b", value: 0.8 }), entry({ todo: "n", value: null }), entry({ todo: "a", value: 0.1 })],
    DEFAULT_FILTER,
    NOW,
  );
  assert.deepEqual(list.priced.map((each) => each.todo), ["a", "b"]);
  assert.deepEqual(list.unpriced.map((each) => each.todo), ["n"]);
  assert.equal(total(list), 3);
});

test("the facets come out in the deployment's order, unknowns after", () => {
  const entries = ["other", "zebra", "work", "", "study"].map((category, i) =>
    entry({ todo: `t${i}`, authored: authored({ category }) }),
  );
  assert.deepEqual(categoriesOf(entries, ["work", "study", "project", "home", "other"]), ["work", "study", "other", "", "zebra"]);
});

test("kinds come out sorted, and a missing Authored contributes the empty kind", () => {
  const entries = [
    entry({ todo: "a", authored: authored({ block: { kind: "todo", name: "todo" } }) }),
    entry({ todo: "b", authored: authored({ block: { kind: "todo", name: "ekaTodo" } }) }),
    entry({ todo: "c", authored: null }),
    entry({ todo: "d", authored: authored({ block: { kind: "todo", name: "todo" } }) }),
  ];
  assert.deepEqual(kindsOf(entries), ["", "ekaTodo", "todo"]);
});

// --- provenance --------------------------------------------------------------

test("unconfirmed covers provisional CONTENT and an unbound CLAIM", () => {
  assert.equal(isUnconfirmed(entry()), false);
  assert.equal(isUnconfirmed(entry({ unconfirmed: true })), true);
});

// --- the preview -------------------------------------------------------------

test("the preview collapses the indentation a typst block is stored with", () => {
  assert.equal(preview("\n  Find the proceedings\n  and file them.\n"), "Find the proceedings and file them.");
  assert.equal(preview(""), "");
  assert.equal(preview("   "), "");
});

test("the preview is still SOURCE — nothing is interpreted or unescaped", () => {
  assert.equal(preview('#link("https://x")[y] #emph[z]'), '#link("https://x")[y] #emph[z]');
});

test("only a body that hides something offers to expand", () => {
  assert.equal(expandable("one line"), false);
  assert.equal(expandable(""), false);
  assert.equal(expandable("\n  two\n  lines\n"), true);
  assert.equal(expandable("x".repeat(CLAMP_CHARS + 1)), true);
});

test("the checklist count reads done-of-total, and says nothing when there is none", () => {
  assert.equal(subtodoCount(entry()), "");
  assert.equal(
    subtodoCount(
      entry({
        authored: authored({
          subtodos: [
            { body: "a", done: true },
            { body: "b", done: false },
            { body: "c", done: true },
          ],
        }),
      }),
    ),
    "2 of 3 done",
  );
});

// --- what the chain has not settled ------------------------------------------

test("attention means provisional content OR a register two writers moved", () => {
  assert.equal(needsAttention(entry()), false);
  assert.equal(needsAttention(entry({ unconfirmed: true })), true);
  assert.equal(needsAttention(entry({ conflicts: { Authored: ["aa", "bb"] } })), true);
});

test("the attention filter narrows and never widens", () => {
  const settled = entry({ todo: "a" });
  const provisional = entry({ todo: "b", unconfirmed: true });
  const only = { ...DEFAULT_FILTER, attention: true };
  assert.equal(matches(settled, only, NOW), false);
  assert.equal(matches(provisional, only, NOW), true);
  // Off, it is not a filter at all: both are still here.
  for (const each of [settled, provisional]) assert.equal(matches(each, DEFAULT_FILTER, NOW), true);
});

// --- waiting (docs/APP.md §4 item 5, chain item `listfilters`) --------------

test("waiting means waiting_on is set — empty is not blocked on anything", () => {
  assert.equal(isWaiting(entry()), false);
  assert.equal(isWaiting(entry({ authored: authored({ waiting_on: "" }) })), false);
  assert.equal(isWaiting(entry({ authored: authored({ waiting_on: "counsel" }) })), true);
  assert.equal(isWaiting(entry({ authored: null })), false);
});

test("the waiting filter narrows and never widens", () => {
  const blocked = entry({ todo: "a", authored: authored({ waiting_on: "counsel" }) });
  const free = entry({ todo: "b" });
  const only = { ...DEFAULT_FILTER, waiting: true };
  assert.equal(matches(blocked, only, NOW), true);
  assert.equal(matches(free, only, NOW), false);
  for (const each of [blocked, free]) assert.equal(matches(each, DEFAULT_FILTER, NOW), true);
});

// --- dated (docs/APP.md §4 item 5, chain item `listfilters`) ----------------

test("dated means a Decay ends within the fortnight, both ends inclusive", () => {
  assert.equal(FORTNIGHT_DAYS, 14);
  const withinAWeek = entry({ authored: authored({ spec: decay("2026-09-16T00:00:00.000Z") }) });
  assert.equal(isDated(withinAWeek, NOW), true);

  // Exactly fourteen days out is still "the next fortnight".
  const exactlyFourteen = entry({ authored: authored({ spec: decay("2026-09-27T00:00:00.000Z") }) });
  assert.equal(isDated(exactlyFourteen, NOW), true);

  // A day later is not.
  const fifteenDaysOut = entry({ authored: authored({ spec: decay("2026-09-28T00:00:00.000Z") }) });
  assert.equal(isDated(fifteenDaysOut, NOW), false);

  // A term with no Decay at all: nothing to be dated by.
  assert.equal(isDated(entry({ authored: authored({ spec: flat(0.5) }) }), NOW), false);
  assert.equal(isDated(entry({ authored: authored({ spec: null }) }), NOW), false);
  assert.equal(isDated(entry({ authored: null }), NOW), false);

  // A deadline already past does not qualify — the window starts at `now`.
  const already = entry({ authored: authored({ spec: decay("2026-09-01T00:00:00.000Z") }) });
  assert.equal(isDated(already, NOW), false);
});

test("dated finds a Decay wherever it is buried in the term", () => {
  // A Conj over a Flat and a Decay: the deadline that matters is not the root.
  const buried: SpecNode = {
    kind: "conj",
    p: -4,
    terms: [flat(0.7), decay("2026-09-20T00:00:00.000Z")],
  };
  assert.equal(isDated(entry({ authored: authored({ spec: buried }) }), NOW), true);

  // A Gate: the same rule applies to its "gate" and "body" slots.
  const gated: SpecNode = { kind: "gate", gate: flat(1), body: decay("2026-09-20T00:00:00.000Z") };
  assert.equal(isDated(entry({ authored: authored({ spec: gated }) }), NOW), true);

  // Two Decays in one Conj: either one falling in the window is enough.
  const either: SpecNode = {
    kind: "conj",
    p: -4,
    terms: [decay("2026-12-01T00:00:00.000Z"), decay("2026-09-18T00:00:00.000Z")],
  };
  assert.equal(isDated(entry({ authored: authored({ spec: either }) }), NOW), true);
});

test("dated finds a Decay inside a deferral's piece, a row of its Piecewise", () => {
  const deferred: SpecNode = {
    kind: "piecewise",
    head: flat(0.9),
    pieces: [{ at: "2026-09-10T00:00:00", term: decay("2026-09-20T00:00:00.000Z") }],
  };
  assert.equal(isDated(entry({ authored: authored({ spec: deferred }) }), NOW), true);
});

test("the dated filter narrows and never widens", () => {
  const soon = entry({ todo: "a", authored: authored({ spec: decay("2026-09-16T00:00:00.000Z") }) });
  const far = entry({ todo: "b", authored: authored({ spec: decay("2027-01-01T00:00:00.000Z") }) });
  const only = { ...DEFAULT_FILTER, dated: true };
  assert.equal(matches(soon, only, NOW), true);
  assert.equal(matches(far, only, NOW), false);
  for (const each of [soon, far]) assert.equal(matches(each, DEFAULT_FILTER, NOW), true);
});

// --- the remembered filter ---------------------------------------------------

test("the filter round-trips through the string localStorage holds", () => {
  const filter = {
    state: "all",
    category: is("work"),
    kind: ANY,
    query: "deed",
    attention: true,
    waiting: true,
    dated: true,
  } as const;
  assert.deepEqual(parseFilter(printFilter(filter)), filter);
  assert.deepEqual(parseFilter(printFilter(DEFAULT_FILTER)), DEFAULT_FILTER);
});

test("anything else in localStorage degrades to the default", () => {
  for (const stored of [null, undefined, "", "{", "[]", '"open"', "3", '{"state":"sideways"}', "{}"]) {
    assert.deepEqual(parseFilter(stored), DEFAULT_FILTER);
  }
});

test("a partial stored filter keeps the fields it does carry", () => {
  assert.deepEqual(parseFilter('{"state":"all","query":"work"}'), {
    ...DEFAULT_FILTER,
    state: "all",
    query: "work",
  });
});

// --- the card's signature -----------------------------------------------------

test("the signature changes when a card's own fields change", () => {
  const open = entry({ value: 0.2 });
  assert.equal(signature(open), signature(entry({ value: 0.2 })));
  assert.notEqual(signature(open), signature(entry({ value: 0.3 })));
  assert.notEqual(signature(open), signature(entry({ state: "completed" })));
  assert.notEqual(signature(open), signature(entry({ unconfirmed: true })));
  assert.notEqual(signature(open), signature(entry({ conflicts: { state: ["a", "b"] } })));
  assert.notEqual(
    signature(open),
    signature(entry({ authored: authored({ body: "something else" }) })),
  );
});

test("the signature ignores the stream: an append no card shows is not a redraw", () => {
  assert.equal(signature(entry({ stream: [] })), signature(entry({ stream: ["a", "b"] })));
});

/**
 * A DECISION IS NOT A TODO (`docs/INBOXPRODROME-PLAN-2026-09-17.md`). The
 * document has separated them since the kind existed; this pins that the
 * list does too, because a decision priced at 55% sitting among the open
 * todos is exactly what shipped to the box on 19 September.
 */
test("the listing leaves decisions to the Inbox", () => {
  const rows = [
    entry({ todo: "harrowreply", value: 0.42 }),
    entry({ todo: "settlebatchdb12e3b7", value: 0.55, authored: authored({ block: { kind: "decision" } }) }),
    entry({ todo: "bookflight", value: 0.61 }),
  ];
  const shown = listing(rows, DEFAULT_FILTER, "2026-09-19T17:00:00");
  assert.deepEqual(
    shown.priced.map((row) => row.todo),
    ["harrowreply", "bookflight"],
  );
  assert.equal(isDecision(rows[1]!), true);
  assert.equal(isDecision(rows[0]!), false);
});
