// The placeholder grammar's browser half: the Rust's spellings (`suzatary_core::held_value`) read here, and the tokens print back to their input.

import { test } from "node:test"
import assert from "node:assert/strict"

import heldvalue from "../src/view/heldvalue"

// Character for character the spellings `held_value`'s own tests assert.
const REVEALABLE = "[SSN ending 3456 · tap to show · held:Zm9ybS0xQGV4YW1wbGUudGVzdA.0]"
const WITHHELD = "[ITIN ending 1234 · withheld]"

test("the Rust's two spellings read as held values, with and without a handle", () => {
  const ts = heldvalue.tokens(`mine is ${REVEALABLE}; also ${WITHHELD}.`)
  assert.deepEqual(
    ts.map((t) => heldvalue.fold(t, (x) => ["text", x], (kind, hint, handle) => [kind, hint, handle])),
    [
      ["text", "mine is "],
      ["SSN", "3456", "held:Zm9ybS0xQGV4YW1wbGUudGVzdA.0"],
      ["text", "; also "],
      ["ITIN", "1234", null],
      ["text", "."],
    ],
  )
})

test("the tokens print back to their input, whatever the text", () => {
  for (const s of [
    "",
    "no number here",
    REVEALABLE,
    `${REVEALABLE}${WITHHELD}`,
    "[SSN ending 3456 · tap to show · held:bad base64!.0]",
    "[PIN ending 1234 · withheld]",
    "[SSN ending 3456 · tap to show ·",
  ]) {
    assert.equal(heldvalue.tokens(s).map(heldvalue.print).join(""), s)
  }
})

test("a spelling the box never writes stays text", () => {
  for (const s of ["[PIN ending 1234 · withheld]", "[SSN ending 3456 · tap to show · held:bad base64!.0]", "[SSN ending 12345 · withheld]"]) {
    assert.equal(heldvalue.holds(s), false, s)
  }
  assert.equal(heldvalue.holds(WITHHELD), true)
})
