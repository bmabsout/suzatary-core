/**
 * The sizing half of a sent photo: what fits is untouched, what does not keeps
 * its shape with the longest edge at the limit, and nothing collapses to zero.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { LONGEST, fitWithin } from "../src/chat/photo";

test("a photo that already fits is drawn at its own size", () => {
  assert.deepEqual(fitWithin(1200, 900), { width: 1200, height: 900 });
  assert.deepEqual(fitWithin(LONGEST, 10), { width: LONGEST, height: 10 });
});

test("a large photo keeps its shape with the longest edge at the limit", () => {
  assert.deepEqual(fitWithin(4032, 3024), { width: 2000, height: 1500 });
  assert.deepEqual(fitWithin(3024, 4032), { width: 1500, height: 2000 });
});

test("a sliver never rounds to nothing", () => {
  assert.deepEqual(fitWithin(40000, 1), { width: 2000, height: 1 });
});
