import { test } from "node:test";
import assert from "node:assert/strict";

import { distanceFromBottom, staysAtBottom } from "../src/chat/scrollpos";

test("the distance from the bottom, and whether it sticks: within one screen, the boundary included", () => {
  // [scrollTop, scrollHeight, clientHeight] and what they read as.
  for (const [[top, height, client], distance, sticks, why] of [
    [[900, 1000, 100], 0, true, "at the bottom"],
    [[0, 50, 200], 0, true, "a container shorter than its viewport"],
    [[820, 1000, 100], 80, true, "scrolled up less than a screen"],
    [[800, 1000, 100], 100, true, "exactly one screen up"],
    [[700, 1000, 100], 200, false, "scrolled up more than a screen"],
  ] as const) {
    assert.equal(distanceFromBottom(top, height, client), distance, why);
    assert.equal(staysAtBottom(top, height, client), sticks, why);
  }
});
