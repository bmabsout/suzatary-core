/**
 * The keyed list's plan.
 *
 * The property that matters is not "the ops are minimal" but "applying them
 * produces exactly the new order" — so most of this drives a tiny array the way
 * `card.ts` drives the DOM, over hand-picked shapes and then over a few hundred
 * random ones. Minimality is checked separately, because it is the whole point:
 * a plan that moved every node would be correct and would still throw away the
 * scroll position this exists to keep.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { reconcile } from "../src/algebra/reconcile";

/** `card.ts`'s DOM work, as array surgery: remove, then insert before a key. */
function apply(oldKeys: readonly string[], newKeys: readonly string[]): { after: string[]; moved: number } {
  const list = [...oldKeys];
  let moved = 0;
  for (const op of reconcile(oldKeys, newKeys)) {
    const at = list.indexOf(op.key);
    if (op.op === "remove") {
      assert.notEqual(at, -1, `remove of a key that is not there: ${op.key}`);
      list.splice(at, 1);
      continue;
    }
    if (op.op === "move") {
      assert.notEqual(at, -1, `move of a key that is not there: ${op.key}`);
      list.splice(at, 1);
      moved += 1;
    } else {
      assert.equal(at, -1, `insert of a key already there: ${op.key}`);
    }
    const anchor = op.before === null ? list.length : list.indexOf(op.before);
    assert.notEqual(anchor, -1, `before names a key that is not placed yet: ${String(op.before)}`);
    list.splice(anchor, 0, op.key);
  }
  return { after: list, moved };
}

function drives(oldKeys: readonly string[], newKeys: readonly string[]): number {
  const { after, moved } = apply(oldKeys, newKeys);
  assert.deepEqual(after, [...newKeys]);
  return moved;
}

test("an unchanged list is no work at all", () => {
  assert.deepEqual(reconcile(["a", "b", "c"], ["a", "b", "c"]), []);
});

test("the empty cases", () => {
  assert.equal(drives([], []), 0);
  assert.equal(drives([], ["a", "b"]), 0);
  assert.equal(drives(["a", "b"], []), 0);
});

test("one todo becoming urgent moves ONE node, not the list", () => {
  // The fold that this exists for: a card climbs from the bottom to the top.
  assert.equal(drives(["a", "b", "c", "d", "e"], ["e", "a", "b", "c", "d"]), 1);
});

test("a completed todo leaving the open list is one removal", () => {
  const ops = reconcile(["a", "b", "c"], ["a", "c"]);
  assert.deepEqual(ops, [{ op: "remove", key: "b" }]);
});

test("a fresh add joins without disturbing what is already ordered", () => {
  assert.equal(drives(["a", "b"], ["a", "new", "b"]), 0);
  assert.deepEqual(reconcile(["a", "b"], ["a", "new", "b"]), [
    { op: "insert", key: "new", before: "b" },
  ]);
});

test("appending names no anchor", () => {
  assert.deepEqual(reconcile(["a"], ["a", "b"]), [{ op: "insert", key: "b", before: null }]);
});

test("a reversal is the worst case and costs n-1 moves, never n", () => {
  assert.equal(drives(["a", "b", "c", "d"], ["d", "c", "b", "a"]), 3);
});

test("insert, remove and move together still land on the new order", () => {
  assert.equal(drives(["a", "b", "c", "d"], ["d", "x", "b"]), 1);
});

test("random folds: applying the plan always yields the new order", () => {
  // A deterministic generator: a failing case has to be reproducible, and a
  // seeded xorshift is twenty characters against a dependency.
  let seed = 0x5eed;
  const next = (): number => {
    seed ^= seed << 13;
    seed ^= seed >>> 17;
    seed ^= seed << 5;
    return Math.abs(seed);
  };
  const pool = "abcdefghijklmnop".split("");
  for (let round = 0; round < 400; round += 1) {
    const pick = (): string[] => {
      const kept = pool.filter(() => next() % 3 !== 0);
      // shuffle
      for (let i = kept.length - 1; i > 0; i -= 1) {
        const j = next() % (i + 1);
        [kept[i], kept[j]] = [kept[j] as string, kept[i] as string];
      }
      return kept;
    };
    const before = pick();
    const after = pick();
    assert.deepEqual(apply(before, after).after, after);
  }
});
