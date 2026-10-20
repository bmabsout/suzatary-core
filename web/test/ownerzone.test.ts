// A cold start reads the owner's days in the zone the box last named, and a stored name that is no zone is UTC.

import { test } from "node:test"
import assert from "node:assert/strict"

import ownerzone from "../src/app/ownerzone"

test("a stored zone name is a zone only where Intl reads it as one", () => {
  assert.deepEqual(ownerzone.parse("America/New_York"), { name: "America/New_York" })
  assert.deepEqual(ownerzone.parse("Europe/Berlin"), { name: "Europe/Berlin" })
  for (const junk of [null, "", "Mars/Olympus", "-4", "<script>"]) assert.equal(ownerzone.parse(junk), null, String(junk))
})
