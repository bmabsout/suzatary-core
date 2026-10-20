/**
 * What a key means, and — more importantly — when it means nothing.
 *
 * The failure this suite exists to prevent is a letter eaten: someone typing a
 * todo body that contains "c" and watching a different todo get completed. So
 * the first properties are the negative ones — while a field has the keyboard,
 * only Escape is claimed, and a modified key is always the browser's.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { SHORTCUTS, keyAction } from "../src/shell/keys";

const IDLE = { typing: false, pending: "", modified: false } as const;

test("typing eats everything but Escape", () => {
  for (const key of ["j", "k", "c", "x", "n", "/", "?", "g", "a", "Enter"]) {
    assert.equal(keyAction(key, { ...IDLE, typing: true }), null, key);
  }
  assert.deepEqual(keyAction("Escape", { ...IDLE, typing: true }), { type: "escape" });
});

test("a modified key belongs to the browser, except the palette's one chord", () => {
  for (const key of ["c", "n", "r", "/", "a", "Escape"]) {
    assert.equal(keyAction(key, { ...IDLE, modified: true }), null, key);
  }
  assert.deepEqual(keyAction("k", { ...IDLE, modified: true }), { type: "palette" });
  // Including out of a field: the palette is how you leave the one you are in.
  assert.deepEqual(keyAction("K", { typing: true, pending: "", modified: true }), { type: "palette" });
});

/**
 * `a`: "ask the secretary" WITH NOTHING ON SCREEN TO POINT AT (project chain
 * `askfromanywhere`, docs/APP.md §4 item 8). Negatives first, as the rest of
 * this suite does — typing and modifiers are already covered above; what is
 * specific to this verb is the conversation page itself.
 */
test("a does nothing on the conversation page — the compose box it would focus is already there", () => {
  assert.equal(keyAction("a", { ...IDLE, onConversation: true }), null);
});

test("a opens the secretary with the compose box's focus intent, everywhere else", () => {
  assert.deepEqual(keyAction("a", IDLE), {
    type: "go",
    route: { type: "secretary", refs: [], focus: true },
  });
  // Absent reads as "not there", the same way a context built before this
  // verb existed still would.
  assert.deepEqual(keyAction("a", { ...IDLE, onConversation: false }), {
    type: "go",
    route: { type: "secretary", refs: [], focus: true },
  });
});

test("the list keys", () => {
  assert.deepEqual(keyAction("j", IDLE), { type: "move", by: 1 });
  assert.deepEqual(keyAction("k", IDLE), { type: "move", by: -1 });
  assert.deepEqual(keyAction("Enter", IDLE), { type: "open" });
  assert.deepEqual(keyAction("l", IDLE), { type: "open" });
  assert.deepEqual(keyAction("h", IDLE), { type: "close" });
});

test("c completes and x cancels, in the wire's own vocabulary", () => {
  assert.deepEqual(keyAction("c", IDLE), { type: "act", name: "done" });
  assert.deepEqual(keyAction("x", IDLE), { type: "act", name: "cancel" });
});

/** `u`/`U`: undo/redo (§10.5). A SEPARATE kind from `act` — they are not one
 * of `todos.applicable`'s verbs. */
test("u undoes and U redoes", () => {
  assert.deepEqual(keyAction("u", IDLE), { type: "undo" });
  assert.deepEqual(keyAction("U", IDLE), { type: "redo" });
  // Shift alone is not a chord `context.modified` reserves — `U` reaches
  // this table exactly like any other unmodified key.
  assert.deepEqual(keyAction("U", { ...IDLE, modified: false }), { type: "redo" });
});

test("the page keys", () => {
  assert.deepEqual(keyAction("/", IDLE), { type: "focusSearch" });
  assert.deepEqual(keyAction("n", IDLE), { type: "add" });
  assert.deepEqual(keyAction("?", IDLE), { type: "help" });
  assert.deepEqual(keyAction("Escape", IDLE), { type: "escape" });
});

test("g arms, and the next key is the destination", () => {
  assert.deepEqual(keyAction("g", IDLE), { type: "arm" });
  const armed = { ...IDLE, pending: "g" };
  assert.deepEqual(keyAction("t", armed), { type: "go", route: { type: "todos" } });
  // The bar's own four, by their first letters (2026-09-10).
  assert.deepEqual(keyAction("i", armed), { type: "go", route: { type: "inbox" } });
  assert.deepEqual(keyAction("p", armed), { type: "go", route: { type: "project" } });
  assert.deepEqual(keyAction("m", armed), { type: "go", route: { type: "mail", query: "", account: "" } });
  assert.deepEqual(keyAction("s", armed), { type: "go", route: { type: "secretary", refs: [] } });
  assert.deepEqual(keyAction("b", armed), { type: "go", route: { type: "briefings" } });
});

test("an armed prefix swallows the key rather than firing the unprefixed one", () => {
  // `g c` must not complete a todo. It is nothing, and the prefix is spent.
  assert.equal(keyAction("c", { ...IDLE, pending: "g" }), null);
});

test("an unbound key is nobody's", () => {
  for (const key of ["q", "F5", "Tab", "ArrowDown", " "]) {
    assert.equal(keyAction(key, IDLE), null, key);
  }
});

test("every key the shortcuts sheet advertises is a key that does something", () => {
  // The sheet is prose beside a table; this is what keeps the two honest.
  const advertised = SHORTCUTS.flatMap((row) => row.keys.split(" / "));
  const chords = advertised.filter((keys) => keys.includes("⌘") || keys.includes("Ctrl"));
  assert.equal(chords.length, 2);
  for (const keys of chords) {
    assert.deepEqual(keyAction(keys.slice(-1).toLowerCase(), { ...IDLE, modified: true }), { type: "palette" });
  }
  const unprefixed = advertised.filter(
    (keys) => !keys.startsWith("g ") && !keys.includes("⌘") && !keys.includes("Ctrl"),
  );
  for (const keys of unprefixed) {
    const key = keys === "Enter" || keys === "Escape" ? keys : keys.trim();
    assert.notEqual(keyAction(key, IDLE), null, `${key} is advertised and does nothing`);
  }
  for (const keys of advertised.filter((each) => each.startsWith("g "))) {
    const key = keys.slice(2);
    assert.notEqual(keyAction(key, { ...IDLE, pending: "g" }), null, `g ${key} is advertised and does nothing`);
  }
});
