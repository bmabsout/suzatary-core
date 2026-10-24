// The box's release sample decodes, and the release page reads it as the box reads it.

import { test } from "node:test"
import assert from "node:assert/strict"
import { readFileSync } from "node:fs"

import payload from "../src/wire/generated/payload"
import releases from "../src/pages/releases"

test("the box's release sample decodes, and reads as the box reads it", () => {
  const samples = JSON.parse(readFileSync("app/core/tests/web/answers.json", "utf8")) as Record<string, { value: unknown }[]>
  const read = payload.ReleasesPayload(samples["ReleasesPayload"]?.[0]?.value)
  if (read.type !== "Val") throw new Error("ReleasesPayload: not decoded")
  const [two, twoone] = read.v.releases
  assert.ok(two !== undefined && twoone !== undefined)
  // wireonce reads open today and stays done; onboarding was moved out.
  assert.equal(releases.sentence(releases.tally(two)), "2 of 3 done, 1 cancelled, 1 moved out.")
  assert.deepEqual(releases.ordered(two).map(i => i.todo), ["portsuites", "calendarlinks", "wireonce", "instancesplit", "onboarding"])
  assert.equal(releases.standing(twoone), "Later.")
  assert.equal(twoone.held, null)
})
