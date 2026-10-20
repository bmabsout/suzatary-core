/**
 * The width half of the router.
 *
 * `#/todo/<id>` means one todo and always has; what changes with the window is
 * whether that is a pushed screen or the pane beside the list. That decision is
 * the only place in the app where a pixel count picks a page, so it is a pure
 * function and these are its cases: the edge itself, both answers for a todo,
 * and the fact that no other page moves.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { DEFAULT_CHAIN, PROJECT_CHAIN } from "../src/app/chain";
import { WIDE_MIN_PX, isWide, screenFor } from "../src/shell/layout";
import cast from "./cast";
import { parseRoute } from "../src/shell/route";
import type { Route } from "../src/shell/route";

const PHONE = 390;
const DESKTOP = 1280;

test("the breakpoint is inclusive, and it is the stylesheet's", () => {
  assert.equal(WIDE_MIN_PX, 992); // 62rem at the 16px root — style.css says 62rem
  assert.equal(isWide(WIDE_MIN_PX), true);
  assert.equal(isWide(WIDE_MIN_PX - 1), false);
  assert.equal(isWide(PHONE), false);
  assert.equal(isWide(DESKTOP), true);
});

test("a todo is a pushed screen on a phone and a selection on a desktop", () => {
  const route = parseRoute("#/todo/permitrenewal");
  assert.deepEqual(screenFor(route, isWide(PHONE)), { type: "todo", id: "permitrenewal" });
  // "" — a todo NAMES no chain, so opening one from either list stays on the
  // list it came from. Resolving it here would be this file deciding it.
  assert.deepEqual(screenFor(route, isWide(DESKTOP)), {
    type: "todos",
    selected: "permitrenewal",
    chain: "",
    prefill: "",
  });
});

test("home is the list with nothing selected, at every width", () => {
  for (const width of [PHONE, DESKTOP]) {
    assert.deepEqual(screenFor(parseRoute("#/"), isWide(width)), {
      type: "todos",
      selected: "",
      chain: DEFAULT_CHAIN,
      prefill: "",
    });
  }
});

test("the two list destinations are the same page over two chains", () => {
  // `#/project` is `#/` with one field different, and that field is the ONLY
  // difference — which is what makes it a destination and not a second page.
  for (const width of [PHONE, DESKTOP]) {
    assert.deepEqual(screenFor(parseRoute("#/project"), isWide(width)), {
      type: "todos",
      selected: "",
      chain: PROJECT_CHAIN,
      prefill: "",
    });
  }
});

test("no other page moves with the width", () => {
  const routes: Route[] = [
    // A message is a destination, not a detail of the inbox: reading it is the
    // point, and a mail list pinned beside it would spend the wide screen on
    // the page you just left.
    { type: "mail", query: "", account: "" },
    { type: "mail", query: "deed", account: cast.world.owner.email },
    { type: "message", id: "m1@example.com" },
    { type: "thread", id: "000000000000c3fb" },
    { type: "chain", focus: "" },
    { type: "chain", focus: "cc324f2c" },
    { type: "briefings" },
    { type: "briefing", date: "2026-09-01" },
    { type: "calendar", date: "" },
    { type: "calendar", date: "2026-10-05" },
    // The secretary does not move with the width either: a conversation is
    // prose with a compose box, which is the destination and not a detail.
    { type: "secretary", refs: [] },
    { type: "conversation", id: "2026-09-09-1", refs: ["todo:life/permitfiling"] },
    { type: "unknown", hash: "#/nowhere" },
  ];
  for (const route of routes) {
    assert.deepEqual(screenFor(route, isWide(PHONE)), route);
    assert.deepEqual(screenFor(route, isWide(DESKTOP)), route);
  }
});

test("a todo with no id never reaches the list as a selection", () => {
  // `#/todo/` is unknown, not a todo — so a wide screen cannot be told to
  // select "" by a malformed link and silently render home instead of saying so.
  assert.deepEqual(screenFor(parseRoute("#/todo/"), isWide(DESKTOP)), { type: "unknown", hash: "#/todo/" });
});

/**
 * A SHARE IS THE LIST WITH THE SHEET UP, at every width (2026-09-10). It is
 * not a page of its own on a phone and a pane on a desktop — there is exactly
 * one place a todo is born (`addsheet.ts`) and the share target opens THAT.
 *
 * It also names no chain, like a todo and unlike the two list destinations: a
 * link shared from a browser has no opinion about which list it belongs on, so
 * it lands on whichever the reader was already reading.
 */
test("a share is the list with the sheet prefilled, at every width", () => {
  for (const width of [PHONE, DESKTOP]) {
    assert.deepEqual(screenFor(parseRoute("#/add?text=Renew%20the%20lease"), isWide(width)), {
      type: "todos",
      selected: "",
      chain: "",
      prefill: "Renew the lease",
    });
    // The title and the text are ONE body by the time a screen exists, and
    // `share.body` is the one rule that says how — not this file.
    assert.deepEqual(
      screenFor(parseRoute("#/add?text=https%3A%2F%2Fexample.test%2Fa&title=A%20title"), isWide(width)),
      { type: "todos", selected: "", chain: "", prefill: "A title\nhttps://example.test/a" },
    );
    // A bare `#/add` is the sheet with nothing in it, which is the list.
    assert.deepEqual(screenFor(parseRoute("#/add"), isWide(width)), {
      type: "todos",
      selected: "",
      chain: "",
      prefill: "",
    });
  }
});

test("the inbox does not move with the width", () => {
  // A batch is a list of diffs with two buttons under it — the destination,
  // not a detail of the list these proposals are not on yet.
  assert.deepEqual(screenFor(parseRoute("#/inbox"), isWide(390)), { type: "inbox" });
  assert.deepEqual(screenFor(parseRoute("#/inbox"), isWide(1280)), { type: "inbox" });
});
