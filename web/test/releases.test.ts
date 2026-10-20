// The release tree's pure half: items tally as a monoid, an item is done by the count's rule and not by its state today,
// a cancelled or moved-out item leaves the count, what is owed is listed first in its own order, and the box's sample
// decodes and reads as the page says it.

import { test } from "node:test"
import assert from "node:assert/strict"
import { readFileSync } from "node:fs"

import type { EntryState, Named, ReleaseItem, ReleaseRow, Standing } from "../src/wire/generated/wire"
import payload from "../src/wire/generated/payload"
import releases from "../src/pages/releases"
import { forAll, type Rng } from "./gen"

const STATES: readonly EntryState[] = ["open", "completed", "cancelled"]
const STANDINGS: readonly Standing[] = ["counted", "done", "cancelled", "open"]
const NAMED: readonly Named[] = ["current", "dropped"]

const item = (r: Rng): ReleaseItem => ({
  todo: r.string("abcdef", 8),
  held: r.bool(0.9)
    ? {
        row: { todo: "x", state: r.pick(STATES), value: r.bool() ? r.int(101) / 100 : null, unconfirmed: false },
        title: r.string("abc ", 12),
      }
    : null,
  standing: r.pick(STANDINGS),
  named: r.pick(NAMED),
})

const row = (r: Rng): ReleaseRow => ({
  todo: "suzatarytwo",
  held: { row: { todo: "suzatarytwo", state: "open", value: 0.4, unconfirmed: false }, title: "Suzatary 2.0" },
  major: 2,
  lot: "open",
  place: "next",
  items: Array.from({ length: r.int(12) }, () => item(r)),
})

const tally = (r: Rng): releases.Tally => ({ owed: r.int(9), done: r.int(9), cancelled: r.int(9), dropped: r.int(9) })

test("law: tallies are a monoid with none as identity", () => {
  forAll(300, r => {
    const [a, b, c] = [tally(r), tally(r), tally(r)]
    assert.deepEqual(releases.combine(releases.none, a), a)
    assert.deepEqual(releases.combine(a, releases.none), a)
    assert.deepEqual(releases.combine(releases.combine(a, b), c), releases.combine(a, releases.combine(b, c)))
  })
})

test("law: an item is done by the count's standing, whatever its state reads today", () => {
  forAll(300, r => {
    const i = item(r)
    const isDone = releases.mark(i).type === "Done"
    assert.equal(isDone, i.standing === "counted" || i.standing === "done")
  })
})

test("law: a release's tally counts each item once", () => {
  forAll(300, r => {
    const x = row(r)
    const t = releases.tally(x)
    assert.equal(t.owed + t.done + t.cancelled + t.dropped, x.items.length)
    assert.equal(t.cancelled, x.items.filter(i => i.standing === "cancelled").length)
  })
})

test("law: what is owed comes first, then what is done, then what is moot, each in its own order", () => {
  forAll(300, r => {
    const x = row(r)
    const rank = (i: ReleaseItem) => releases.fold(releases.mark(i), { Owed: 0, Done: 1, Cancelled: 2, Dropped: 3 })
    const ranks = releases.ordered(x).map(rank)
    assert.deepEqual(ranks, [...ranks].sort((a, b) => a - b))
    for (const k of [0, 1, 2, 3]) {
      assert.deepEqual(releases.ordered(x).filter(i => rank(i) === k), x.items.filter(i => rank(i) === k))
    }
  })
})

test("a cancelled or moved-out item leaves the count and is said apart", () => {
  assert.equal(releases.sentence({ owed: 3, done: 11, cancelled: 1, dropped: 0 }), "11 of 14 done, 1 cancelled.")
  assert.equal(releases.sentence({ owed: 0, done: 2, cancelled: 0, dropped: 1 }), "2 of 2 done, 1 moved out.")
  assert.equal(releases.sentence(releases.none), "No items yet.")
})

test("the box's release sample decodes, and reads as the count reads it", () => {
  const samples = JSON.parse(readFileSync("app/core/tests/web/answers.json", "utf8")) as Record<string, { value: unknown }[]>
  const read = payload.ReleasesPayload(samples["ReleasesPayload"]?.[0]?.value)
  if (read.type !== "Val") throw new Error("ReleasesPayload: not decoded")
  const [two, twoone] = read.v.releases
  assert.ok(two !== undefined && twoone !== undefined)
  // wireonce reads open today and stays counted; onboarding was moved out.
  assert.equal(releases.sentence(releases.tally(two)), "2 of 3 done, 1 cancelled, 1 moved out.")
  assert.deepEqual(releases.ordered(two).map(i => i.todo), ["portsuites", "calendarlinks", "wireonce", "instancesplit", "onboarding"])
  assert.equal(releases.standing(twoone), "Later.")
  assert.equal(twoone.held, null)
})

test("law: every item a revision dropped says it moved out, done ones included", () => {
  forAll(300, r => {
    const i = item(r)
    assert.equal(releases.movedOut(i), i.named === "dropped")
  })
  const done: ReleaseItem = { todo: "x", held: null, standing: "counted", named: "dropped" }
  assert.equal(releases.mark(done).type, "Done")
  assert.ok(releases.movedOut(done))
})

test("a release tree the box could not read is said, never hidden", () => {
  assert.deepEqual(releases.failed(new Error("the chain is held and not read")), {
    type: "Failed",
    why: "the chain is held and not read",
  })
  assert.deepEqual(releases.failed("plain"), { type: "Failed", why: "plain" })
})
