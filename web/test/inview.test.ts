/**
 * What reference is in view for a screen (`docs/CHATSHAPE-PLAN-2026-09-15.md`
 * §8.2, §8.5 commit 11). Pure, so every shape below is a plain object.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import type { Screen } from "../src/shell/layout";
import { referenceInView } from "../src/chat/inview";

test("the todo list with nothing selected is the whole conversation", () => {
  const screen: Screen = { type: "todos", selected: "", chain: "life", prefill: "" };
  assert.deepEqual(referenceInView(screen, "life"), { type: "none" });
});

test("the todo list with a row selected is that todo, on the current chain", () => {
  const screen: Screen = { type: "todos", selected: "x", chain: "life", prefill: "" };
  assert.deepEqual(referenceInView(screen, "life"), {
    type: "one",
    reference: { kind: "todo", chain: "life", id: "x" },
  });
});

test("the standalone todo screen is that todo, on the current chain", () => {
  const screen: Screen = { type: "todo", id: "book-flight" };
  assert.deepEqual(referenceInView(screen, "suzatary"), {
    type: "one",
    reference: { kind: "todo", chain: "suzatary", id: "book-flight" },
  });
});

test("a message is its hash", () => {
  const screen: Screen = { type: "message", id: "a1b2c3d4e5f6" };
  assert.deepEqual(referenceInView(screen, "life"), {
    type: "one",
    reference: { kind: "mail", hash: "a1b2c3d4e5f6" },
  });
});

test("a thread is its id", () => {
  const screen: Screen = { type: "thread", id: "0000000000001b2c" };
  assert.deepEqual(referenceInView(screen, "life"), {
    type: "one",
    reference: { kind: "thread", id: "0000000000001b2c" },
  });
});

test("a briefing is its day", () => {
  const screen: Screen = { type: "briefing", date: "2026-09-10" };
  assert.deepEqual(referenceInView(screen, "life"), {
    type: "one",
    reference: { kind: "briefing", date: "2026-09-10" },
  });
});

test("the inbox, mail and everywhere else are the whole conversation", () => {
  const screens: Screen[] = [
    { type: "inbox" },
    { type: "mail", query: "", account: "" },
    { type: "chain", focus: "" },
    { type: "usage" },
    { type: "about" },
    { type: "briefings" },
    { type: "secretary", refs: [] },
    { type: "conversation", id: "default", refs: [] },
    { type: "unknown", hash: "#/nope" },
  ];
  for (const screen of screens) {
    assert.deepEqual(referenceInView(screen, "life"), { type: "none" }, JSON.stringify(screen));
  }
});
