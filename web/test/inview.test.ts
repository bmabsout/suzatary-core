/**
 * What reference is in view for a screen (`docs/CHATSHAPE-PLAN-2026-09-15.md`
 * §8.2, §8.5 commit 11). Pure, so every shape below is a plain object.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import type { Screen } from "../src/shell/layout";
import { referenceInView } from "../src/chat/inview";
import type { Reference } from "../src/chat/refs";

test("the todo list with nothing selected is the whole conversation", () => {
  const screen: Screen = { type: "todos", selected: "", chain: "life", prefill: "" };
  assert.deepEqual(referenceInView(screen, "life"), { type: "none" });
});

test("a screen about one thing is that thing: a todo on the current chain, a message, a thread, a briefing", () => {
  const cases: [Screen, string, Reference][] = [
    [{ type: "todos", selected: "x", chain: "life", prefill: "" }, "life", { kind: "todo", chain: "life", id: "x" }],
    [{ type: "todo", id: "book-flight" }, "suzatary", { kind: "todo", chain: "suzatary", id: "book-flight" }],
    [{ type: "message", id: "a1b2c3d4e5f6" }, "life", { kind: "mail", hash: "a1b2c3d4e5f6" }],
    [{ type: "thread", id: "0000000000001b2c" }, "life", { kind: "thread", id: "0000000000001b2c" }],
    [{ type: "briefing", date: "2026-09-10" }, "life", { kind: "briefing", date: "2026-09-10" }],
  ];
  for (const [screen, chain, reference] of cases) {
    assert.deepEqual(referenceInView(screen, chain), { type: "one", reference }, screen.type);
  }
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
