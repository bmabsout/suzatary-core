/**
 * `chat/aside.ts`, the owner's rule for the secretary beside a wide page: it opens by itself only on a page about one
 * thing when the conversation holds talk about it; a page of many things keeps it closed until a press; a press, open
 * or close, holds for that page alone, and a page of many things is its destination.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import aside from "../src/chat/aside";
import app from "../src/app/app";
import { step } from "../src/chat/source";
import type { SourceState } from "../src/chat/source";
import { parseRoute } from "../src/shell/route";
import type { ConversationPayload, TurnRow } from "../src/wire/generated/wire";
import { forAll } from "./gen";

const turn = (index: number): TurnRow => ({
  index,
  who: "person",
  verb: "",
  text: "hi",
  refused: false,
  references: [],
  attachments: [],
  queued: false,
  blocks: [],
  at: "2026-09-16T10:00:00",
  remembered: null,
  attempt: "current",
  gone: null,
});

const payload = (turns: TurnRow[]): ConversationPayload => ({
  id: "default",
  title: "Secretary",
  model: "claude",
  opened: "2026-09-16",
  turns,
  proposals: [],
  unplaced: [],
  state: "idle",
  first: 0,
  total: turns.length,
  again: null,
});

const LOADING: SourceState = { kind: "loading", id: "default", about: undefined };
const loaded = (turns: TurnRow[], fallback: boolean): SourceState =>
  step(LOADING, { kind: "window", mode: "newest", payload: payload(turns), fallback, at: 0 });

const at = (hash: string, wide = true): aside.Placement => aside.placement(app.initial(parseRoute(hash), wide, "life"));
const page = (hash: string): aside.Page => {
  const p = at(hash);
  assert.equal(p.type, "Beside", hash);
  return p.type === "Beside" ? p.page : { type: "Many", key: "" };
};
const NONE: aside.Presses = {};

test("a page about one thing is that thing; a page of many things is its destination", () => {
  for (const hash of ["#/todo/file-permit", "#/mail/m1%40example.com", "#/mail/thread/000000000000c3fb", "#/briefings/2026-09-01"]) {
    assert.equal(page(hash).type, "One", hash);
  }
  for (const [hash, key] of [["#/", "todos"], ["#/inbox", "inbox"], ["#/settings", "settings"], ["#/mail", "mail"], ["#/calendar", "calendar"], ["#/project", "project"], ["#/about", "about"]] as const) {
    assert.deepEqual(page(hash), { type: "Many", key }, hash);
  }
  assert.equal(page("#/todo/file-permit").key, "todo:life/file-permit");
});

test("no panel on a phone, nor beside the secretary's own pages", () => {
  assert.equal(at("#/todo/file-permit", false).type, "Absent");
  for (const hash of ["#/secretary", "#/secretary/default", "#/secretary/memory"]) assert.equal(at(hash).type, "Absent", hash);
});

test("talk is turns about the page, never the whole conversation drawn as a fallback", () => {
  assert.equal(aside.talk(LOADING), "Quiet");
  assert.equal(aside.talk(loaded([], false)), "Quiet");
  assert.equal(aside.talk(loaded([turn(0)], true)), "Quiet");
  assert.equal(aside.talk(loaded([turn(0)], false)), "Talking");
});

test("it opens by itself on a page about one thing with talk, and never on a page of many", () => {
  assert.equal(aside.shown(page("#/todo/file-permit"), NONE, "Talking"), true);
  assert.equal(aside.shown(page("#/todo/file-permit"), NONE, "Quiet"), false);
  for (const hash of ["#/", "#/inbox", "#/settings", "#/mail", "#/calendar"]) {
    assert.equal(aside.shown(page(hash), NONE, "Talking"), false, hash);
  }
});

test("a press opens it for that page alone", () => {
  const said = aside.pressed(page("#/inbox"), NONE, "Quiet");
  assert.equal(aside.shown(page("#/inbox"), said, "Quiet"), true);
  assert.equal(aside.shown(page("#/settings"), said, "Quiet"), false, "not on the next page");
  assert.equal(aside.shown(page("#/todo/file-permit"), said, "Quiet"), false);
});

test("a page of many things is the same page through its dates and searches", () => {
  const calendar = aside.pressed(page("#/calendar"), NONE, "Quiet");
  assert.equal(aside.shown(page("#/calendar/2026-10-20"), calendar, "Quiet"), true, "next week is still the calendar");
  const mail = aside.pressed(page("#/mail"), NONE, "Quiet");
  assert.equal(aside.shown(page("#/mail?q=x"), mail, "Quiet"), true, "a search is still Mail");
});

test("a press that closes it holds for that page alone, and every other todo still opens by itself", () => {
  const todo = page("#/todo/file-permit");
  const closed = aside.pressed(todo, NONE, "Talking");
  assert.equal(aside.shown(todo, closed, "Talking"), false);
  assert.equal(aside.shown(page("#/todo/book-flight"), closed, "Talking"), true);
  assert.equal(aside.shown(todo, aside.pressed(todo, closed, "Talking"), "Talking"), true);
});

test("law: a press turns over what is drawn on its page and nowhere else", () => {
  const pages = ["#/", "#/inbox", "#/calendar", "#/todo/file-permit", "#/todo/book-flight", "#/mail/m1%40example.com"].map(page);
  forAll(300, (r) => {
    const p = r.pick(pages);
    const presses: aside.Presses = Object.fromEntries(pages.filter(() => r.bool()).map((q) => [q.key, r.pick(["open", "closed"] as const)]));
    const talk = r.pick(["Quiet", "Talking"] as const);
    const after = aside.pressed(p, presses, talk);
    assert.equal(aside.shown(p, after, talk), !aside.shown(p, presses, talk));
    for (const q of pages) if (q.key !== p.key) assert.equal(aside.shown(q, after, talk), aside.shown(q, presses, talk), q.key);
  });
});
