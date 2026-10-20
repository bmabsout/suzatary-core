import { test } from "node:test";
import assert from "node:assert/strict";

import type { Entry, TodoPayload } from "../src/wire/generated/wire";
import reading from "../src/todo/reading";
import { SLUG, forAll } from "./gen";

const entry = (todo: string): Entry => ({
  todo,
  state: "open",
  states: ["open"],
  at: "",
  claimed: "",
  unconfirmed: false,
  recurring: false,
  value: 0.42,
  conflicts: {},
  authored: null,
  candidates: [],
  stream: [],
});

const payload = (todo: string, html: string | null = "<header></header>"): TodoPayload => ({
  todo,
  at: "2026-09-26T08:00:00",
  entry: entry(todo),
  reading: html,
  stream: [],
  diffs: {},
  links: { out: [], in: [] },
});

test("a page typst did not render is the entry's plain text, the same state as never fetched", () => {
  assert.deepEqual(reading.of(payload("a")), reading.page("<header></header>"));
  assert.deepEqual(reading.of(payload("a", null)), reading.plain(entry("a")));
});

test("the tab's cache keeps the last payload per todo, newest last, at most MAX", () => {
  forAll(50, (r) => {
    const ids = r.array(r.int(3 * reading.MAX), () => r.string(SLUG, 1 + r.int(2)));
    const held = ids.reduce((h, id) => reading.keep(h, payload(id)), reading.none);
    const distinct = [...new Set(ids.slice().reverse())].reverse();
    assert.deepEqual([...held.keys()], distinct.slice(Math.max(0, distinct.length - reading.MAX)));
  });
});

test("keeping is idempotent: the same payload twice is the cache once", () => {
  const once = reading.keep(reading.none, payload("a"));
  assert.deepEqual([...reading.keep(once, payload("a"))], [...once]);
});

test("a failed fetch answers from the tab's cache, then the fold, then not at all", () => {
  const held = reading.keep(reading.none, payload("a"));
  assert.equal(reading.fallback(held, [entry("a")], "a", "offline")?.type, "Held");
  assert.equal(reading.fallback(reading.none, [entry("a")], "a", "offline")?.type, "Unfetched");
  assert.equal(reading.fallback(held, [entry("b")], "b", "offline")?.type, "Unfetched");
  assert.equal(reading.fallback(held, [], "b", "offline"), null);
});

test("what a page draws from is the same whichever way it arrived", () => {
  const fetched: reading.Got = { type: "Fetched", payload: payload("a") };
  const held: reading.Got = { type: "Held", payload: payload("a"), why: "offline" };
  const unfetched: reading.Got = { type: "Unfetched", entry: entry("a"), why: "offline" };
  for (const g of [fetched, held, unfetched]) assert.deepEqual(reading.entryOf(g), entry("a"));
  assert.deepEqual(reading.readingOf(unfetched), reading.plain(entry("a")));
  assert.equal(reading.payloadOf(unfetched), null);
});

test("a plain page marks an unconfirmed todo and says it is shown as written, as the box's header does", () => {
  assert.deepEqual(reading.marks({ ...entry("deed"), unconfirmed: true }), { chips: ["unconfirmed"], note: "Shown the way it was written until you confirm it." });
  assert.deepEqual(reading.marks(entry("deed")).chips, []);
});
