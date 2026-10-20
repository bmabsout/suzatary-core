/**
 * The palette's ranking.
 *
 * Two properties carry it. A text that does not contain the query's letters IN
 * ORDER does not match at all — a fuzzy matcher that always answers makes the
 * list a lottery — and among the ones that do, a word-boundary hit beats a
 * buried one, so typing `fr` finds "Freight reply" rather than the todo with
 * an f and an r somewhere in the middle of it.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { rank, score } from "../src/algebra/match";

test("a subsequence matches; anything else does not", () => {
  assert.notEqual(score("hwr", "harrowreply"), null);
  assert.notEqual(score("harrow", "harrowreply"), null);
  // The letters are all there, in the wrong order.
  assert.equal(score("rwh", "harrowreply"), null);
  assert.equal(score("zzz", "harrowreply"), null);
});

test("an empty query matches everything, equally", () => {
  assert.equal(score("", "anything"), 0);
  assert.equal(score("   ", "anything"), 0);
});

test("case does not matter on either side", () => {
  assert.notEqual(score("HARR", "harrowreply"), null);
  assert.notEqual(score("harr", "HARROWREPLY"), null);
});

test("a word-boundary hit outranks a buried one", () => {
  const boundary = score("fr", "freight reply") as number;
  const buried = score("fr", "waiting for a reply") as number;
  assert.ok(boundary > buried, `${boundary} should beat ${buried}`);
});

test("consecutive letters outrank scattered ones", () => {
  const together = score("abc", "abcdefgh") as number;
  const apart = score("abc", "axxbxxcxx") as number;
  assert.ok(together > apart, `${together} should beat ${apart}`);
});

test("a short exact answer outranks a long one that also matches", () => {
  const short = score("deed", "deed") as number;
  const long = score("deed", "deed transfer paperwork, the whole of it, in one line") as number;
  assert.ok(short > long);
});

test("rank keeps the caller's order when nothing distinguishes the items", () => {
  const items = ["one", "two", "three"];
  assert.deepEqual(rank("", items, (each) => each), items);
});

test("rank drops non-matches and respects the limit", () => {
  const items = ["harrowreply", "deedtransfer", "permitoffsite", "harrowcall"];
  // Both start with the query, so the shorter one is the better answer.
  assert.deepEqual(rank("harr", items, (each) => each), ["harrowcall", "harrowreply"]);
  assert.equal(rank("e", items, (each) => each, 2).length, 2);
});

test("the best answer comes first", () => {
  const items = ["waiting for a reply", "freight reply"];
  assert.equal(rank("fr", items, (each) => each)[0], "freight reply");
});
