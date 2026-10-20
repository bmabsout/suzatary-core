import { test } from "node:test";
import assert from "node:assert/strict";

import { distanceFromBottom, staysAtBottom } from "../src/chat/scrollpos";

test("at the bottom reads zero distance and stays stuck", () => {
  assert.equal(distanceFromBottom(900, 1000, 100), 0);
  assert.equal(staysAtBottom(900, 1000, 100), true);
});

test("a container shorter than its viewport is fully at the bottom", () => {
  assert.equal(distanceFromBottom(0, 50, 200), 0);
  assert.equal(staysAtBottom(0, 50, 200), true);
});

test("scrolled up less than a screen still sticks", () => {
  // 1000 tall content, 100 tall viewport, scrolled up 80px from the bottom.
  assert.equal(distanceFromBottom(820, 1000, 100), 80);
  assert.equal(staysAtBottom(820, 1000, 100), true);
});

test("scrolled up more than a screen stops sticking", () => {
  assert.equal(distanceFromBottom(700, 1000, 100), 200);
  assert.equal(staysAtBottom(700, 1000, 100), false);
});

test("exactly one screen up is still stuck (the boundary is inclusive)", () => {
  assert.equal(distanceFromBottom(800, 1000, 100), 100);
  assert.equal(staysAtBottom(800, 1000, 100), true);
});
