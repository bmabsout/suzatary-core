/**
 * The calendar page's days: a week forward and back is where it started, over the change of
 * offset, and "" is today.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import calendar from "../src/pages/calendar";
import { day, forAll } from "./gen";
import time from "../src/app/time";

test("a shift and its inverse are the identity, across every change of offset", () => {
  forAll(200, (r, seed) => {
    const from = day(r);
    const n = r.int(400) - 200;
    assert.equal(calendar.shift(calendar.shift(from, n), -n), from, `seed ${seed}`);
  });
  assert.equal(calendar.shift("2026-10-25", 1), "2026-10-26");
  assert.equal(calendar.shift("2026-03-29", -1), "2026-03-28");
  assert.equal(calendar.shift("2026-12-31", 1), "2027-01-01");
});

test("no day is the owner's today, and a day is itself", () => {
  // 23:30 in New York is the next day in UTC; the owner's today is the 5th.
  const today = time.date({ name: "America/New_York" }, Date.UTC(2026, 9, 6, 3, 30));
  assert.equal(calendar.start("", today), "2026-10-05");
  assert.equal(calendar.start("2026-09-01", today), "2026-09-01");
});
