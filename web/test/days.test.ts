import { test } from "node:test";
import assert from "node:assert/strict";

import { dayLabel, daySeparators } from "../src/chat/days";
import time from "../src/app/time";

test("a known calendar day prints as the plan's own example", () => {
  assert.equal(dayLabel("2026-09-14T09:00:00"), "Monday 14 September");
});

test("an unparseable instant is the empty string", () => {
  assert.equal(dayLabel(""), "");
  assert.equal(dayLabel("not a date"), "");
});

test("the first item gets a separator, and a day that repeats does not", () => {
  const marks = daySeparators([
    { key: 0, at: "2026-09-14T09:00:00" },
    { key: 1, at: "2026-09-14T14:00:00" },
    { key: 2, at: "2026-09-15T09:00:00" },
  ], time.UTC);
  assert.equal(marks.get(0), "Monday 14 September");
  assert.equal(marks.has(1), false);
  assert.equal(marks.get(2), "Tuesday 15 September");
});

test("an item with no instant carries no mark and does not reset the day", () => {
  const marks = daySeparators([
    { key: 0, at: "2026-09-14T09:00:00" },
    { key: 1, at: "" },
    { key: 2, at: "2026-09-14T20:00:00" },
  ], time.UTC);
  assert.equal(marks.size, 1);
  assert.equal(marks.has(1), false);
  assert.equal(marks.has(2), false);
});

test("a separator falls on the owner's day, not UTC's", () => {
  // 01:00 UTC on the 15th is the evening of the 14th in New York.
  const marks = daySeparators(
    [
      { key: 0, at: "2026-09-14T22:00:00" },
      { key: 1, at: "2026-09-15T01:00:00" },
      { key: 2, at: "2026-09-15T13:00:00" },
    ],
    { name: "America/New_York" },
  );
  assert.deepEqual([...marks], [[0, "Monday 14 September"], [2, "Tuesday 15 September"]]);
});
