/**
 * The transcript regrouped into rounds (`docs/CHATSHAPE-PLAN-2026-09-15.md`
 * §4.1, §6 commit 2; each row's own `index` field, §8.5 commit 9). Pure, so
 * every shape below is a plain object — no DOM, no fetch.
 */

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import type { ProposalRow, TurnRow, TurnWho } from "../src/wire/generated/wire";
import { foldOf, rounds, verbFamily } from "../src/chat/rounds";
import { aCard } from "./cards";
import { forAll } from "./gen";

function person(index: number, text: string, at = "2026-09-15T09:00:00"): TurnRow {
  return {
    index,
    who: "person",
    verb: "",
    text,
    refused: false,
    references: [],
    attachments: [],
    queued: false,
    blocks: [],
    at,
    remembered: null,
    attempt: "current",
    gone: null,
  };
}

function act(index: number, text: string, at = "2026-09-15T09:00:00"): TurnRow {
  return {
    index,
    who: "act",
    verb: "",
    text,
    refused: false,
    references: [],
    attachments: [],
    queued: false,
    blocks: [],
    at,
    remembered: null,
    attempt: "current",
    gone: null,
  };
}

function model(index: number, text: string, at = "2026-09-15T09:00:01"): TurnRow {
  return {
    index,
    who: "model",
    verb: "",
    text,
    refused: false,
    references: [],
    attachments: [],
    queued: false,
    blocks: [],
    at,
    remembered: null,
    attempt: "current",
    gone: null,
  };
}

function verb(
  index: number,
  name: string,
  text = "answer",
  refused = false,
  at = "2026-09-15T09:00:02",
): TurnRow {
  return {
    index,
    who: "verb",
    verb: name,
    text,
    refused,
    references: [],
    attachments: [],
    queued: false,
    blocks: [],
    at,
    remembered: null,
    attempt: "current",
    gone: null,
  };
}

function proposalAt(index: number, turn: number): ProposalRow {
  const sources = ["mail:abc123abc123"];
  return {
    index,
    act: { kind: "email", account: "a", to: [], cc: [], subject: "s", body: "b", attachments: [] },
    card: aCard(),
    why: "because",
    sources,
    at: "2026-09-15T09:00:05",
    state: "standing",
    outcome: "",
    tries: 0,
    turn,
    coding: null,
    announced: null,
  };
}

test("a bare exchange is one round: opening, no trace, an answer", () => {
  const turns = [person(0, "hi"), model(1, "hello", "2026-09-15T09:00:10")];
  const [round] = rounds(turns, []);
  assert.equal(rounds(turns, []).length, 1);
  assert.equal(round!.opening?.index, 0);
  assert.equal(round!.trace.length, 0);
  assert.equal(round!.answer?.index, 1);
  assert.equal(foldOf(round!), null, "nothing was checked, so there is no fold line");
});

test("two status turns and three verbs fold into one round", () => {
  const turns = [
    person(0, "what changed"),
    model(1, "Reading mail.", "2026-09-15T09:00:01"),
    verb(2, "search", "3 hits"),
    verb(3, "show", "one message"),
    model(4, "Checking todos.", "2026-09-15T09:00:20"),
    verb(5, "todo_show", "one todo"),
    model(6, "Here is what changed.", "2026-09-15T09:00:42"),
  ];
  const [round] = rounds(turns, []);
  assert.equal(round!.opening?.index, 0);
  assert.equal(round!.trace.length, 5, "two status turns and three verbs");
  assert.equal(round!.answer?.index, 6);
  const fold = foldOf(round!);
  assert.ok(fold);
  assert.equal(fold.refused, false);
  assert.match(fold.label, /mail \(2\)/);
  assert.match(fold.label, /todos \(1\)/);
  assert.match(fold.label, / · 42 s$/);
});

test("a refused verb is marked in the fold", () => {
  const turns = [
    person(0, "send it"),
    model(1, "Sending.", "2026-09-15T09:00:01"),
    verb(2, "send", "cannot send from here", true),
    model(3, "I cannot send that.", "2026-09-15T09:00:05"),
  ];
  const [round] = rounds(turns, []);
  const fold = foldOf(round!);
  assert.ok(fold);
  assert.equal(fold.refused, true);
});

test("an unfinished round ends in a status turn's own trace, with no answer", () => {
  const turns = [
    person(0, "check something"),
    model(1, "Reading mail.", "2026-09-15T09:00:01"),
    verb(2, "search", "3 hits"),
  ];
  const [round] = rounds(turns, []);
  assert.equal(round!.trace.length, 2, "the status turn and its verb, still the whole round");
  assert.equal(round!.answer, null);
});

test("two rounds are two rounds", () => {
  const turns = [
    person(0, "first"),
    model(1, "first answer", "2026-09-15T09:00:02"),
    person(2, "second"),
    model(3, "second answer", "2026-09-15T09:00:05"),
  ];
  const grouped = rounds(turns, []);
  assert.equal(grouped.length, 2);
  assert.equal(grouped[0]!.opening?.index, 0);
  assert.equal(grouped[0]!.answer?.index, 1);
  assert.equal(grouped[1]!.opening?.index, 2);
  assert.equal(grouped[1]!.answer?.index, 3);
});

test("a proposal positioned after the answer of round one lands on round one", () => {
  const turns = [
    person(0, "first"),
    model(1, "first answer", "2026-09-15T09:00:02"),
    person(2, "second"),
    model(3, "second answer", "2026-09-15T09:00:05"),
  ];
  // turn: 2 means "drawn after the turn at index 1" — the first round's answer.
  const grouped = rounds(turns, [proposalAt(0, 2)]);
  assert.equal(grouped[0]!.proposals.length, 1);
  assert.equal(grouped[1]!.proposals.length, 0);
});

test("an empty answer yields a round with no answer to draw", () => {
  const turns = [person(0, "anything to add?"), model(1, "")];
  const [round] = rounds(turns, []);
  assert.equal(round!.answer, null);
  assert.equal(round!.trace.length, 0);
});

test("a leading model turn (the morning brief) opens with no person or act", () => {
  const turns = [model(0, "Your brief for Monday is ready.", "2026-09-15T10:03:00")];
  const [round] = rounds(turns, []);
  assert.equal(round!.opening, null);
  assert.equal(round!.answer?.index, 0);
  assert.equal(round!.key, 0);
});

test("a proposal past the last turn is drawn after the last round", () => {
  const turns = [person(0, "first"), model(1, "first answer", "2026-09-15T09:00:02")];
  const grouped = rounds(turns, [proposalAt(0, 99)]);
  assert.equal(grouped[grouped.length - 1]!.proposals.length, 1);
});

test("verb families are recognised and an unknown verb prints as itself", () => {
  assert.equal(verbFamily("search"), "mail");
  assert.equal(verbFamily("thread"), "mail");
  assert.equal(verbFamily("todo_add"), "todos");
  assert.equal(verbFamily("list_open_todos"), "todos");
  assert.equal(verbFamily("briefing_delta"), "notes");
  assert.equal(verbFamily("propose_email"), "proposals");
  assert.equal(verbFamily("record"), "record");
});

test("an act can open a round exactly like a person's turn", () => {
  const turns = [act(0, "You did: cancelled todo:life/x"), model(1, "Understood.", "2026-09-15T09:00:03")];
  const [round] = rounds(turns, []);
  assert.equal(round!.opening?.turn.who, "act");
  assert.equal(round!.answer?.index, 1);
});

test("a window that starts mid-transcript reads its own absolute indices, not positions", () => {
  // A windowed page (§6 commit 6): `turns[0]` is really turn 40, never 0.
  const turns = [person(40, "what now"), model(41, "Here.", "2026-09-15T09:00:02")];
  const [round] = rounds(turns, []);
  assert.equal(round!.opening?.index, 40);
  assert.equal(round!.answer?.index, 41);
  assert.equal(round!.startIndex, 40);
  assert.equal(round!.endIndex, 41);
});

test("a gap between two kept rounds (?about=) still groups each one correctly", () => {
  // `?about=` can drop whole rounds, so turns[2] jumps from 1 to 10 — the gap
  // must not be read as one round spanning it.
  const turns = [
    person(0, "first"),
    model(1, "first answer", "2026-09-15T09:00:02"),
    person(10, "much later", "2026-09-16T09:00:00"),
    model(11, "later answer", "2026-09-16T09:00:02"),
  ];
  const grouped = rounds(turns, []);
  assert.equal(grouped.length, 2);
  assert.equal(grouped[0]!.startIndex, 0);
  assert.equal(grouped[0]!.endIndex, 1);
  assert.equal(grouped[1]!.startIndex, 10);
  assert.equal(grouped[1]!.endIndex, 11);
});

/** One row of `app/core/tests/about-fixture.json`. */
interface FixtureTurn {
  readonly who: TurnWho;
  readonly text: string;
  readonly references: readonly string[];
  readonly at: string;
}

/**
 * THE SAME FIXTURE `suzatary_core::secretary::about`'s OWN TEST READS
 * (`docs/CHATSHAPE-PLAN-2026-09-15.md` §8.5 commit 9) — one transcript, two
 * independent groupings, pinned to agree: Rust's `round_spans` answers
 * `[(0,3),(4,5),(6,7)]` over the same eight turns, and this asserts `rounds`
 * partitions the identical three spans.
 */
test("the shared fixture groups into the same three spans the Rust side pins", () => {
  const rows = JSON.parse(readFileSync("app/core/tests/about-fixture.json", "utf8")) as FixtureTurn[];
  const turns: TurnRow[] = rows.map((row, index) => ({
    index,
    who: row.who,
    verb: row.who === "verb" ? "todo_show" : "",
    text: row.text,
    refused: false,
    references: [...row.references],
    attachments: [],
    queued: false,
    blocks: [],
    at: row.at,
    remembered: null,
    attempt: "current",
    gone: null,
  }));
  const grouped = rounds(turns, []);
  const spans = grouped.map((round) => [round.startIndex, round.endIndex]);
  assert.deepEqual(spans, [
    [0, 3],
    [4, 5],
    [6, 7],
  ]);
});

// --- asking again (docs/CHAT-PLAN-2026-09-25.md §4.4) -----------------------

const earlier = (row: TurnRow): TurnRow => ({ ...row, attempt: "earlier" });

test("a regenerated round keeps its question, the new answer, and the old one as an earlier attempt", () => {
  const grouped = rounds([person(0, "q"), earlier(model(1, "old")), model(2, "new")], []);
  assert.equal(grouped.length, 1);
  const [round] = grouped;
  assert.equal(round?.opening?.index, 0);
  assert.equal(round?.answer?.index, 2);
  assert.deepEqual(round?.earlier.map((p) => p.index), [1]);
});

test("an edited question and all after it are an earlier attempt of their own, before the new round", () => {
  const grouped = rounds(
    [earlier(person(0, "q")), earlier(model(1, "a")), person(2, "q, reworded"), model(3, "b")],
    [],
  );
  assert.deepEqual(
    grouped.map((round) => [round.key, round.opening?.index ?? null, round.earlier.map((p) => p.index)]),
    [
      [0, null, [0, 1]],
      [2, 2, []],
    ],
  );
});

test("a stopped round asked again draws the stop inside the earlier attempt, beside its question", () => {
  const grouped = rounds(
    [person(0, "q"), earlier(act(1, "Ada stopped the answer")), earlier(model(2, "")), model(3, "a")],
    [],
  );
  assert.equal(grouped.length, 1);
  assert.equal(grouped[0]?.answer?.index, 3);
  assert.deepEqual(grouped[0]?.earlier.map((p) => p.index), [1, 2]);
});

test("law: every turn is drawn once — a current one as opening, trace or answer, an earlier one in earlier", () => {
  forAll(300, (r) => {
    const makers = [person, act, model, (i: number) => verb(i, r.pick(["search", "show", "todo_add"]))];
    const turns = r.array(r.int(24), () => 0).map((_, index) => {
      const row = r.pick(makers)(index, r.pick(["", "text"]));
      return r.bool(0.3) ? earlier(row) : row;
    });
    const grouped = rounds(turns, []);
    const drawn = grouped.flatMap((round) => [
      ...(round.opening === null ? [] : [round.opening]),
      ...round.trace,
      ...(round.answer === null ? [] : [round.answer]),
    ]);
    const kept = grouped.flatMap((round) => round.earlier);
    assert.ok(drawn.every((p) => p.turn.attempt === "current"), "no earlier turn drawn as current");
    assert.ok(kept.every((p) => p.turn.attempt === "earlier"), "no current turn folded away");
    const seen = [...drawn, ...kept].map((p) => p.index).sort((a, b) => a - b);
    const expected = turns
      .filter((t) => !(t.attempt === "current" && t.who === "model" && t.text === "" && endsItsRound(turns, t.index)))
      .map((t) => t.index);
    assert.deepEqual(seen, expected);
  });
});

/** A current model turn nothing current follows before the next opening — the round's answer candidate. */
function endsItsRound(turns: readonly TurnRow[], index: number): boolean {
  const next = turns.slice(index + 1).find((t) => t.attempt === "current" || t.who === "person");
  return next === undefined || next.who === "person" || next.who === "act";
}

test("a proposal said in a round ?about= left out is drawn after the round before the gap", () => {
  const turns = [
    person(0, "first"),
    model(1, "first answer", "2026-09-15T09:00:02"),
    person(10, "much later", "2026-09-16T09:00:00"),
    model(11, "later answer", "2026-09-16T09:00:02"),
  ];
  const grouped = rounds(turns, [proposalAt(0, 6)]);
  assert.deepEqual(grouped.map((round) => round.proposals.length), [1, 0]);
});

test("law: every card is drawn once, in the last round begun by the turn it follows, else the first", () => {
  forAll(300, (r) => {
    const makers = [person, act, model, (i: number) => verb(i, "search")];
    let at = 0;
    const turns = r.array(r.int(16), () => 0).map(() => {
      at += 1 + r.int(3);
      return r.pick(makers)(at, r.pick(["", "text"]));
    });
    const cards = r.array(r.int(6), () => 0).map((_, n) => proposalAt(n, r.int(at + 4)));
    const grouped = rounds(turns, cards);
    if (grouped.length === 0) return assert.equal(turns.length, 0, "no round only with no turn");
    grouped.forEach((round, n) => {
      const next = grouped[n + 1];
      const mine = cards.filter(
        (p) => (n === 0 || round.startIndex <= p.turn - 1) && (next === undefined || next.startIndex > p.turn - 1),
      );
      assert.deepEqual(round.proposals, mine, "the cards begun by this round and not the next, in the order given");
    });
    assert.equal(grouped.flatMap((round) => round.proposals).length, cards.length, "each drawn once");
  });
});
