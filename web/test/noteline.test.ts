/**
 * The one decision `noteline.ts` makes: what "something to say" means.
 * `noteField`'s DOM building is exercised end to end by the pages that call
 * it (`web-e2e.sh`'s writes scenes) — this pins the pure half only.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { hasNote, noteOf } from "../src/view/noteline";

test("an untouched field is empty and has no note", () => {
  assert.equal(noteOf(""), "");
  assert.equal(hasNote(""), false);
});

test("whitespace alone is still nothing to say", () => {
  assert.equal(noteOf("   \n\t "), "");
  assert.equal(hasNote("   \n\t "), false);
});

test("a real line is trimmed, not dropped", () => {
  assert.equal(noteOf("  the meeting moved to October  "), "the meeting moved to October");
  assert.equal(hasNote("  the meeting moved to October  "), true);
});
