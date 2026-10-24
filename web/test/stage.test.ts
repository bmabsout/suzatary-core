import { test } from "node:test"
import assert from "node:assert/strict"

import type { ProposalState } from "../src/wire/generated/wire"
import stage from "../src/inbox/stage"

const states: readonly ProposalState[] = [
  "standing", "sent", "applied", "started", "acted", "failed", "unknown", "checked", "dismissed", "superseded",
  "withdrawn", "unreadable",
]

test("law: whatever may be answered is open, a send that may have gone waits for his check, and settled states are neither", () => {
  for (const s of states) {
    assert.ok(!stage.answerable(s) || stage.open(s), s)
    const settled =
      s === "sent" || s === "applied" || s === "acted" || s === "checked" || s === "dismissed" || s === "superseded" ||
      s === "withdrawn" || s === "unreadable"
    assert.equal(stage.open(s), !settled, s)
  }
  assert.ok(stage.open("unknown") && !stage.answerable("unknown"), "open, and never sent or dismissed again")
})
