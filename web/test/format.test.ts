/**
 * THE THREE SHAPES A DATE TAKES ON A PAGE, and the percentage.
 *
 * `format.ts` is the one place an instant, a day or an age becomes something a
 * person reads — it was four hand-rolled printers across five modules until
 * 2026-09-10, which is exactly how two pages come to spell the same moment two
 * ways. These are the properties that must survive the consolidation, and they
 * are the ones a browser is not needed for.
 *
 * ⚠️ WHAT IS NOT TESTED HERE, and why: nothing that builds a NODE (`chip`,
 * `shortHash`, `hashLink`) — those need a DOM, and `web/e2e` is where the DOM
 * is real. This file is the pure half.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { ageLabel, day, iso, pct, percent, recent, shortDay, when, whenOf } from "../src/view/format";
import time from "../src/app/time";
import { everywhere } from "./zones";

// --- the percentage ----------------------------------------------------------

test("a fulfillment is one decimal, and absence is a dash and not a zero", () => {
  assert.equal(pct(0.42), "42.0%");
  assert.equal(pct(0), "0.0%");
  assert.equal(pct(1), "100.0%");
  // The distinction the whole app rests on: no price is not a price of zero.
  assert.equal(pct(null), "—");
  assert.equal(pct(undefined), "—");
});

// --- an instant --------------------------------------------------------------

test("an instant a person reads is the wire's, with the T spent", () => {
  assert.equal(iso("2026-09-04T11:32:07"), "2026-09-04 11:32:07");
  assert.equal(iso(""), "");
  // One T, the separator's — a value with no T is handed back as it came.
  assert.equal(iso("2026-09-04"), "2026-09-04");
});

// --- a day -------------------------------------------------------------------

test("the day is the leading ten characters, not a trip through Date", () => {
  assert.equal(day("2026-09-04T11:32:07"), "2026-09-04");
  assert.equal(day("2026-09-04"), "2026-09-04");
  assert.equal(day(""), "");
});

test("the day of a UTC instant is the day the instant NAMES", () => {
  // A `new Date(...).getDate()` here would move the day by a zone, which is
  // the bug a string operation cannot have: the wire's instants are
  // naive-local and the app must not re-interpret them.
  assert.equal(day("2026-01-01T00:30:00+00:00"), "2026-01-01");
  assert.equal(day("2026-12-31T23:30:00+00:00"), "2026-12-31");
});

// --- a mail instant ----------------------------------------------------------

test("a mail row's date is to the minute, on the owner's clock and never the device's", () =>
  everywhere(() => {
    assert.equal(whenOf("2026-09-03T09:05:00", time.UTC), "2026-09-03 09:05");
    assert.equal(whenOf("2026-09-03T01:05:00", { name: "America/New_York" }), "2026-09-02 21:05");
    assert.equal(whenOf("2026-09-03T01:05:00-04:00", { name: "Europe/Berlin" }), "2026-09-03 07:05");
  }));

test("a Date header that would not parse is shown, not invented", () => {
  // The server sends "" for a header it could not read; an epoch here would be
  // a date the message never had.
  assert.equal(whenOf("", time.UTC), "");
  assert.equal(whenOf("not a date", time.UTC), "not a date");
});

// --- a recent instant, in a list ---------------------------------------------

test("a conversation's last turn reads as its minute today, yesterday, or its day, on the owner's clock", () =>
  everywhere(() => {
    const now = Date.UTC(2026, 9, 13, 15, 40);
    assert.equal(recent("2026-10-13T15:38:00", now, time.UTC), "15:38");
    assert.equal(recent("2026-10-12T23:59:00", now, time.UTC), "yesterday");
    assert.equal(recent("2026-10-11T09:00:00", now, time.UTC), "2026-10-11");
    // 01:00 UTC on the 13th is still the 12th in New York.
    assert.equal(recent("2026-10-13T01:00:00", now, { name: "America/New_York" }), "yesterday");
    assert.equal(recent("", now, time.UTC), "");
    assert.equal(recent("not a date", now, time.UTC), "not a date");
  }));

// --- a browser clock ---------------------------------------------------------

test("the staleness banner's moment is the owner's clock and then the day", () =>
  everywhere(() => {
    assert.equal(when(Date.UTC(2026, 8, 4, 11, 32, 7), time.UTC), "11:32 on 2026-09-04");
    assert.equal(when(Date.UTC(2026, 8, 4, 0, 5), time.UTC), "00:05 on 2026-09-04");
    assert.equal(when(Date.UTC(2026, 8, 4, 0, 5), { name: "Europe/Berlin" }), "02:05 on 2026-09-04");
    assert.equal(when(Date.UTC(2026, 8, 4, 0, 5), { name: "America/New_York" }), "20:05 on 2026-09-03");
  }));

test("a zero is a moment nobody recorded, and says so rather than reading 1970", () => {
  assert.equal(when(0, time.UTC), "an unknown time");
});

// --- an age ------------------------------------------------------------------

test("the age is coarse, because a day either way is the same news", () => {
  const now = Date.UTC(2026, 8, 10, 12);
  assert.equal(ageLabel("2026-09-10", now, time.UTC), "today");
  assert.equal(ageLabel("2026-09-09", now, time.UTC), "1d");
  assert.equal(ageLabel("2026-09-01", now, time.UTC), "9d");
  assert.equal(ageLabel("2026-08-20", now, time.UTC), "3w");
  assert.equal(ageLabel("2026-05-10", now, time.UTC), "4mo");
  assert.equal(ageLabel("2024-09-10", now, time.UTC), "2y");
});

test("no created date, an unparseable one, and a clock from the future all say nothing", () => {
  const now = Date.UTC(2026, 8, 10, 12);
  assert.equal(ageLabel("", now, time.UTC), "");
  assert.equal(ageLabel("not a date", now, time.UTC), "");
  // A device with a wrong clock is a stated limit of this system; "in -3d" is
  // worse than silence.
  assert.equal(ageLabel("2026-09-13", now, time.UTC), "");
});

test("an age is the owner's days, whatever the device's zone", () =>
  everywhere(() => {
    // 02:00 UTC on the 11th is still the 10th in New York, so a todo made on the 10th is today's there.
    const late = Date.UTC(2026, 8, 11, 2);
    assert.equal(ageLabel("2026-09-10", late, { name: "America/New_York" }), "today");
    assert.equal(ageLabel("2026-09-10", late, time.UTC), "1d");
    assert.equal(ageLabel("2026-09-11T01:00:00", late, { name: "America/New_York" }), "today");
  }));

test("an age is taken from the DAY, so a stored time of day does not shift it", () => {
  const now = Date.UTC(2026, 8, 10, 12);
  assert.equal(ageLabel("2026-09-09T23:59:59", now, time.UTC), "1d");
  assert.equal(ageLabel("2026-09-09T00:00:01", now, time.UTC), "1d");
});

test("a short day has no year in this year, and names the year in another", () => {
  const z = time.UTC;
  const now = time.ms("2026-10-13T12:00:00") ?? 0;
  assert.ok(!shortDay("2026-09-23T15:38:56", z, now).includes("2026"));
  assert.ok(shortDay("2025-09-23T15:38:56", z, now).includes("2025"));
  assert.equal(shortDay("", z, now), "");
});

test("a list prints the whole percent beside its pie", () => {
  assert.equal(percent(0.129), "13%");
  assert.equal(percent(1), "100%");
});
