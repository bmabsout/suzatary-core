// coding's laws: a diff stat's tally is a monoid, and the answers a card offers are
// exactly the steps its state admits.

import { test } from "node:test"
import assert from "node:assert/strict"

import type { CodingFile } from "../src/wire/generated/wire"
import coding from "../src/inbox/coding"
import { forAll } from "./gen"
import type { Rng } from "./gen"

const file = (r: Rng): CodingFile =>
  r.bool(0.2)
    ? { path: r.string("abc/.", 1 + r.int(8)), added: null, removed: null }
    : { path: r.string("abc/.", 1 + r.int(8)), added: r.int(400), removed: r.int(400) }
const tally = (r: Rng): coding.Tally => coding.tally(file(r))

const session = (r: Rng): coding.Session =>
  r.pick([
    { state: "fired", id: "session_1", url: "https://claude.ai/code/session_1", at: "2026-09-27T09:00:00" },
    { state: "reported", id: "session_1", branch: "claude/x", path: "docs/reports/x.md", report: "r", stat: [], handed: r.bool() ? "" : "todo:suzatary/mergeabc123" },
    { state: "merged", id: "session_1", sha: "a".repeat(40) },
    { state: "dropped", id: "session_1", reason: "done by hand" },
  ] satisfies coding.Session[])

test("law: the tally's zero is an identity and combine is associative", () =>
  forAll(300, r => {
    const [a, b, c] = [tally(r), tally(r), tally(r)]
    assert.deepEqual(coding.combine(coding.zero, a), a)
    assert.deepEqual(coding.combine(a, coding.zero), a)
    assert.deepEqual(coding.combine(coding.combine(a, b), c), coding.combine(a, coding.combine(b, c)))
  }))

test("law: a stat's total counts every file, and a binary file adds no lines", () =>
  forAll(300, r => {
    const fs = r.array(r.int(6), () => file(r))
    const t = coding.total(fs)
    assert.equal(t.files, fs.length)
    assert.equal(t.added, fs.reduce((n, f) => n + (f.added ?? 0), 0))
  }))

test("a card offers only what its state admits: hand once reported, drop while live", () =>
  forAll(100, r => {
    const s = session(r)
    const offered = coding.answers(s)
    const expected =
      s.state === "fired" ? ["Drop"]
      : s.state === "reported" ? (s.handed === "" ? ["Hand", "Drop"] : ["Drop"])
      : []
    assert.deepEqual(offered, expected)
    assert.notEqual(coding.headline(s), "")
    assert.notEqual(coding.signature(s), coding.signature(null))
  }))
