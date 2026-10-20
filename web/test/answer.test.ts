/**
 * An answer in blocks (`docs/CHAT-PLAN-2026-09-25.md` §4.5, §5): the page
 * agrees with the box over `app/core/tests/web/blocks.json`.
 */

import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";

import type { BlockKind, Frame } from "../src/wire/generated/wire";
import answer from "../src/chat/answer";
import clock from "../src/algebra/clock";
import { type Rng, forAll } from "./gen";

type Cut = { answer: string; blocks: { source: string; plain: string; kind: BlockKind; sha: string }[] };

const cuts: Cut[] = JSON.parse(readFileSync("app/core/tests/web/blocks.json", "utf8"));

test("law: block sources concatenate to the answer", () => {
  for (const c of cuts) {
    assert.equal(c.blocks.map((b) => b.source).join(""), c.answer.trim() === "" ? "" : c.answer, c.answer);
  }
});

test("a block's name is the sha256 of its source, the only thing the page asks the box for", () => {
  for (const b of cuts.flatMap((c) => c.blocks)) {
    assert.equal(createHash("sha256").update(b.source, "utf8").digest("hex"), b.sha);
  }
});


test("the page agrees: streamed as fragments and block frames, each vector rebuilds its answer", () => {
  forAll(40, (r) => {
    for (const c of cuts.filter((c) => c.blocks.length > 0)) {
      let a: answer.T = { type: "Thinking", since: 0 };
      let rest = c.answer;
      while (rest.length > 0) {
        const n = 1 + r.int(8);
        a = answer.step(a, { kind: "delta", text: rest.slice(0, n) }, 1);
        rest = rest.slice(n);
      }
      c.blocks.slice(0, -1).forEach((b, n) => {
        a = answer.step(a, { kind: "block", n, sha: b.sha, source: b.source }, 2);
      });
      assert.equal(a.type, "Streaming");
      if (a.type !== "Streaming") return;
      assert.deepEqual(a.blocks.map((b) => b.sha), c.blocks.slice(0, -1).map((b) => b.sha));
      assert.equal(a.tail, c.blocks[c.blocks.length - 1]?.source);
    }
  });
});

const frame = (r: Rng, said: string): Frame => {
  switch (r.int(6)) {
    case 0: return { kind: "state", state: r.pick(["idle", "queued", "thinking", "answering"] as const) };
    case 1: return { kind: "working", verb: r.pick(["search", "show", "context"]) };
    case 2: return { kind: "turn" };
    case 3: {
      const cut = r.int(said.length + 1);
      return { kind: "block", n: r.int(4), sha: "s", source: r.bool() ? said.slice(0, cut) : "elsewhere" };
    }
    default: return { kind: "delta", text: r.string("ab \n", 1 + r.int(5)) };
  }
};

test("law: answer.step reaches only legal states", () => {
  forAll(300, (r) => {
    let a: answer.T = answer.done;
    let said = "";
    let at = 0;
    for (let i = r.int(40); i > 0; i -= 1) {
      at += r.int(3);
      const tail = a.type === "Streaming" ? a.tail : "";
      const f = frame(r, tail);
      const before = a;
      a = answer.step(a, f, at);
      if (f.kind === "delta") said = (before.type === "Streaming" ? said : "") + f.text;
      switch (a.type) {
        case "Streaming":
          assert.deepEqual(a.blocks.map((b) => b.n), a.blocks.map((_, n) => n), "blocks close in order");
          assert.equal(a.blocks.map((b) => b.source).join("") + a.tail, said, "sources and tail tile what was said");
          break;
        case "Working":
          assert.ok(a.checked >= 1 && a.since <= at);
          break;
        case "Thinking":
          assert.ok(a.since <= at);
          break;
        case "Done":
          break;
      }
      if (f.kind === "block" && before.type !== "Streaming") assert.equal(a, before, "a block with nothing streaming changes nothing");
    }
  });
});

test("Copy takes an answer's words block by block, never its markup", () => {
  for (const c of cuts) {
    const copied = answer.plain(c.blocks);
    assert.equal(copied, c.blocks.map((b) => b.plain).join("\n\n"));
  }
});

test("the place says what the round is doing, and nothing once text lands", () => {
  assert.equal(answer.label({ type: "Thinking", since: 0 }, 12_400), "Thinking · 12 s");
  assert.equal(answer.label({ type: "Working", verb: "search", checked: 3, since: 0 }, 41_000), "Searching mail · 3 checked · 41 s");
  assert.equal(answer.label({ type: "Streaming", blocks: [], tail: "x" }, 0), "");
});

test("a question the box holds is thinking from the moment it is seen, with a clock that moves, until it is idle", () => {
  const held = answer.settle(answer.done, "queued", 5_000);
  assert.deepEqual(held, { type: "Thinking", since: 5_000 });
  assert.equal(answer.label(held, 5_000), "Thinking · 0 s");
  assert.equal(answer.label(held, 8_200), "Thinking · 3 s");
  assert.deepEqual(answer.next(held, 8_200), clock.at(9_000), "the count is drawn again at its next second");
  forAll(200, (r) => {
    const states = r.array(1 + r.int(6), () => r.pick(["queued", "thinking", "answering"] as const));
    const a = states.reduce<answer.T>((a, s, i) => answer.settle(a, s, 5_000 + i * 1_000), held);
    assert.deepEqual(a, held, "the round starting keeps the clock it began with");
    assert.equal(answer.settle(a, "idle", 99_000), answer.done);
  });
});
