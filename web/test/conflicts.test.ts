/**
 * `describeConflicts`: `Entry.conflicts` turned into rows a page can draw.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import type { Entry } from "../src/wire/generated/wire";
import { describeConflicts } from "../src/todo/conflicts";

function entry(conflicts: Record<string, string[]>): Entry {
  return {
    todo: "harrowreply",
    state: "open",
    states: ["open"],
    at: "",
    claimed: "",
    unconfirmed: false,
    recurring: false,
    value: 0.42,
    conflicts,
    authored: null,
    candidates: [],
    stream: [],
  };
}

test("no conflicts describes as nothing", () => {
  assert.deepEqual(describeConflicts(entry({})), []);
});

test("one register, its hashes kept in the order the wire sent them", () => {
  assert.deepEqual(describeConflicts(entry({ state: ["b1", "a1"] })), [{ kind: "state", hashes: ["b1", "a1"] }]);
});

test("several registers come out sorted by kind, not by wire order", () => {
  assert.deepEqual(describeConflicts(entry({ spec: ["b", "e"], content: ["a", "f"], state: ["c", "d"] })), [
    { kind: "content", hashes: ["a", "f"] },
    { kind: "spec", hashes: ["b", "e"] },
    { kind: "state", hashes: ["c", "d"] },
  ]);
});

test("a register is a conflict exactly when the one reading says so: a single write is a value", () => {
  assert.deepEqual(describeConflicts(entry({ spec: ["b"], state: ["c", "d"] })), [{ kind: "state", hashes: ["c", "d"] }]);
});
