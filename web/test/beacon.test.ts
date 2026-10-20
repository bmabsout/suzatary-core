// beacon's laws: a report names the build, the page and the device, and a tab says each failure once and a bounded number of times.

import { test } from "node:test"
import assert from "node:assert/strict"

import beacon from "../src/app/beacon"
import { SLUG, forAll } from "./gen"
import type { Rng } from "./gen"

const at: beacon.Where = { build: "abc123", agent: "Phone", page: "todo" }
const report = (r: Rng): beacon.Report =>
  beacon.of(r.pick<beacon.Kind>(["error", "rejection", "core"]), r.bool() ? new Error(r.string(SLUG, r.int(4))) : r.string(SLUG, 1 + r.int(3)), at)

test("an Error is its name, message and stack; anything else is its text", () => {
  const e = new TypeError("x is undefined")
  const r = beacon.of("error", e, at)
  assert.equal(r.message, "TypeError: x is undefined")
  assert.equal(r.stack, e.stack)
  assert.deepEqual(beacon.of("rejection", 42, at), { kind: "rejection", ...at, message: "42", stack: "" })
  assert.equal(beacon.of("core", "", at).message, "(no message)")
  assert.ok(beacon.of("error", "m".repeat(10_000), at).message.length <= 501)
})

test("law: the reports sent are the distinct ones, in order, and never more than MOST", () =>
  forAll(300, r => {
    const rs = r.array(r.int(30), () => report(r))
    const sent: beacon.Report[] = []
    rs.reduce((s, x) => {
      const next = beacon.step(s, x)
      if (next !== s) sent.push(x)
      return next
    }, beacon.none)
    const distinct = rs.filter((x, i) => rs.findIndex(y => beacon.key(y) === beacon.key(x)) === i)
    assert.deepEqual(sent, distinct.slice(0, beacon.MOST))
  }))
