/**
 * `undo.ts`'s pure pieces: the ~8-second window and what one stream line
 * says. Nothing here touches the DOM or the network — `acts.ts` and
 * `todoview.ts` are the shells this drives.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { UNDO_WINDOW_MS, actLabel, undoStillLive } from "../src/todo/undo";

test("the undo window is live right up to ~8 seconds and dead after", () => {
  const shownAt = 1_000_000;
  assert.equal(undoStillLive(shownAt, shownAt), true);
  assert.equal(undoStillLive(shownAt, shownAt + UNDO_WINDOW_MS - 1), true);
  assert.equal(undoStillLive(shownAt, shownAt + UNDO_WINDOW_MS), false);
  assert.equal(undoStillLive(shownAt, shownAt + UNDO_WINDOW_MS + 1), false);
});

test("an ordinary event carries no label", () => {
  assert.equal(actLabel({ kind: "nothing" }, () => "Completed"), null);
});

test("undo is labelled by the KIND of the object it reverses, not by its own kind", () => {
  const kindOf = (hash: string): string | undefined =>
    ({ a: "Completed", b: "Cancelled", c: "Reopened", d: "SpecRevised", e: "Authored" })[hash];
  assert.equal(actLabel({ kind: "undo", of: "a" }, kindOf), "undid completion");
  assert.equal(actLabel({ kind: "undo", of: "b" }, kindOf), "undid cancellation");
  assert.equal(actLabel({ kind: "undo", of: "c" }, kindOf), "undid reopening");
  assert.equal(actLabel({ kind: "undo", of: "d" }, kindOf), "undid a price change");
  assert.equal(actLabel({ kind: "undo", of: "e" }, kindOf), "undid a content edit");
});

test("an undo naming a hash outside the lookup still says it undid something", () => {
  assert.equal(actLabel({ kind: "undo", of: "nowhere" }, () => undefined), "undid the previous act");
});

/** Redo stays GENERIC — `undo.ts`'s own header says why: the redone event's
 * own kind already says what happened. */
test("redo is always just redid", () => {
  const kindOf = (): string | undefined => "Completed";
  assert.equal(actLabel({ kind: "redo", of: "a" }, kindOf), "redid");
  assert.equal(actLabel({ kind: "redo", of: "anything" }, () => undefined), "redid");
});
