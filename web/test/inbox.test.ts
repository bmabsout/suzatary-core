/**
 * THE INBOX'S PURE READING (`docs/PROPOSALS-PLAN-2026-09-28.md`): what waits,
 * in what order, where each names its card, what settled today, and each
 * resolved against what was fetched — every part `web/src/inbox/waiting.ts` and
 * `web/src/inbox/inbox.ts` hold with no network.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import type { Authored, Entry } from "../src/wire/generated/wire";
import type { ConversationPayload, ProposalRow, ProposedObject } from "../src/wire/generated/wire";
import type { InboxProposal, ProposalsPayload, ProposedBatch } from "../src/wire/generated/wire";
import { oneCardPerBatch, rowOf, sameRows, splitSetup, waitingSentence } from "../src/inbox/inbox";
import type { Row } from "../src/inbox/inbox";
import asker from "../src/inbox/asker";
import spoken from "../src/inbox/spoken";
import { parse } from "../src/chat/refs";
import waiting from "../src/inbox/waiting";
import { forAll } from "./gen";
import time from "../src/app/time";
import { aCard } from "./cards";
import cast from "./cast";

const { counsel } = cast.world;

function authored(over: Partial<Authored> = {}): Authored {
  return {
    rich: { body: null, detail: null, rationale: null, waiting_on: null, keys: { body: null, detail: null, rationale: null, waiting_on: null } },
    block: { kind: "decision" },
    created: "2026-09-17",
    body: `Send the ${counsel.name} reply`,
    text: `Send the ${counsel.name} reply`,
    detail: `proposal:default/3\n\nTo: ${counsel.email}\nSubject: The deed`,
    category: "",
    waiting_on: "",
    rationale: ["He asked for the documents."],
    spec: null,
    actor: "assistant",
    at: "2026-09-17T09:00:00",
    source: null,
    subtodos: [],
    ...over,
  };
}

function entry(over: Partial<Entry> = {}): Entry {
  return {
    todo: "d1",
    state: "open",
    states: ["open"],
    at: "",
    claimed: "",
    unconfirmed: false,
    recurring: false,
    value: 0.42,
    conflicts: {},
    authored: authored(),
    candidates: [],
    stream: [],
    ...over,
  };
}

function asked(over: Partial<InboxProposal> = {}): InboxProposal {
  return {
    id: "p1",
    asked: "proposal:default/3",
    serves: null,
    headline: `email ${counsel.email} — The deed`,
    by: { kind: "secretary" },
    why: "He asked for the documents.",
    state: { kind: "open" },
    value: null,
    at: "2026-09-17T09:00:00",
    ...over,
  };
}

const openKeys = (proposals: readonly InboxProposal[], folds: readonly waiting.ChainFold[]): string[] =>
  waiting.open(waiting.all(proposals, folds)).map(waiting.key);

// --- the ordering --------------------------------------------------------------

test("open: ascending in urgency, across the inbox and every chain, ties on the key", () => {
  const folds: waiting.ChainFold[] = [
    { chain: "life", entries: [entry({ todo: "b", value: 0.6 })] },
    { chain: "suzatary", entries: [entry({ todo: "a", value: 0.2 })] },
  ];
  assert.deepEqual(openKeys([asked({ id: "p0", value: 0.4 })], folds), ["a", "p0", "b"]);
});

test("open: a resolved decision, a settled proposal and a todo are all left out", () => {
  const folds: waiting.ChainFold[] = [
    {
      chain: "life",
      entries: [
        entry({ todo: "done", state: "completed" }),
        entry({ todo: "plaintodo", authored: authored({ block: { kind: "todo", name: "todo" } }) }),
        entry({ todo: "open", state: "open" }),
      ],
    },
  ];
  assert.deepEqual(openKeys([asked({ id: "sent", state: { kind: "completed", at: "2026-09-17T08:30:00" } })], folds), ["open"]);
});

test("open: an unpriced proposal sorts after every priced row, but is still shown", () => {
  const folds: waiting.ChainFold[] = [{ chain: "life", entries: [entry({ todo: "priced", value: 0.1 })] }];
  assert.deepEqual(openKeys([asked({ id: "unpriced" })], folds), ["priced", "unpriced"]);
});

// --- the settled-today split ---------------------------------------------------

test("settledOn: kept to what was resolved today, dropping open ones and other days", () => {
  const folds: waiting.ChainFold[] = [
    {
      chain: "life",
      entries: [
        entry({ todo: "today1", state: "completed", at: "2026-09-17 09:05:00" }),
        entry({ todo: "today2", state: "cancelled", at: "2026-09-17 08:00:00" }),
        entry({ todo: "yesterday", state: "completed", at: "2026-09-16 09:00:00" }),
        entry({ todo: "stillopen", state: "open" }),
      ],
    },
  ];
  const proposals = [
    asked({ id: "sent", state: { kind: "completed", at: "2026-09-17T08:30:00" } }),
    asked({ id: "old", state: { kind: "cancelled", at: "2026-09-16T08:30:00" } }),
  ];
  // Newest first.
  assert.deepEqual(
    waiting.settledOn(waiting.all(proposals, folds), "2026-09-17", time.UTC).map(waiting.key),
    ["today1", "sent", "today2"],
  );
});

test("settledOn: a todo (not a decision) resolved today is not counted here", () => {
  const folds: waiting.ChainFold[] = [
    { chain: "life", entries: [entry({ todo: "t1", authored: authored({ block: { kind: "todo", name: "todo" } }), state: "completed", at: "2026-09-17 09:00:00" })] },
  ];
  assert.deepEqual(waiting.settledOn(waiting.all([], folds), "2026-09-17", time.UTC), []);
});

test("settledOn: the day is the owner's, so a decision settled at 21:00 in New York is today's there", () => {
  const folds: waiting.ChainFold[] = [
    { chain: "life", entries: [entry({ todo: "late", state: "completed", at: "2026-09-18 01:00:00" })] },
  ];
  const all = waiting.all([], folds);
  assert.deepEqual(waiting.settledOn(all, "2026-09-17", { name: "America/New_York" }).map(waiting.key), ["late"]);
  assert.deepEqual(waiting.settledOn(all, "2026-09-17", time.UTC), []);
});

// --- the reference-to-row decision ----------------------------------------------

const decision = (e: Entry): waiting.Waiting => waiting.decision("life", e);

test("where: a proposal names the conversation it was asked in", () => {
  assert.deepEqual(waiting.where(waiting.proposal(asked())), { type: "Proposal", conversation: "default", index: 3 });
});

test("where: a decision reads its batch off detail's first line", () => {
  const found = waiting.where(decision(entry({ authored: authored({ detail: "batch:life/a1b2c3d4\n\nheld for review" }) })));
  assert.deepEqual(found, { type: "Batch", chain: "life", head: "a1b2c3d4" });
});

test("where: a reference this app has no card for, an unreadable one and no content are all unresolved", () => {
  for (const e of [
    entry({ authored: authored({ detail: "todo:life/x1\n\nsome text" }) }),
    entry({ authored: authored({ detail: "garbage" }) }),
    entry({ authored: authored({ detail: "" }) }),
    entry({ authored: null }),
  ]) {
    assert.equal(waiting.where(decision(e)).type, "Unresolved", JSON.stringify(e.authored?.detail));
  }
});

// --- resolving a row against what was fetched -----------------------------------

function proposalRow(over: Partial<ProposalRow> = {}): ProposalRow {
  return {
    index: 3,
    act: { kind: "email", account: "a@b.test", to: ["c@d.test"], cc: [], subject: "The deed", body: "…", attachments: [] },
    card: aCard(),
    why: "he asked",
    sources: [],
    at: "2026-09-17T09:00:00",
    state: "standing",
    outcome: "",
    tries: 0,
    turn: 0,
    coding: null,
    announced: null,
    ...over,
  };
}

function conversationPayload(over: Partial<ConversationPayload> = {}): ConversationPayload {
  return {
    id: "default",
    title: "Default",
    model: "test",
    opened: "2026-09-17T08:00:00",
    turns: [],
    proposals: [proposalRow()],
    unplaced: [],
    state: "idle",
    first: 0,
    total: 0,
    again: null,
    ...over,
  };
}

function batchOf(over: Partial<ProposedBatch> = {}): ProposedBatch {
  return { head: "a1b2c3d4", at: "2026-09-17T08:00:00", actor: "triage", objects: [object("a1b2c3d4", [])], ...over };
}

function object(hash: string, needs: string[]): ProposedObject {
  return { hash, act: "added", todo: "t1", body: "", at: "", actor: "triage", note: "", changes: [], needs, pending_todo: false };
}

/** A held batch that grew after the hold: `a1b2c3d4` was the head the
 * secretary held, and `e5f6a7b8` was appended on top of it. */
const grown = (): ProposedBatch =>
  batchOf({ head: "e5f6a7b8", objects: [object("a1b2c3d4", []), object("e5f6a7b8", ["a1b2c3d4"])] });

test("rowOf: a decision is about the batch that holds its object, and reviewed only what it saw", () => {
  const held = decision(entry({ authored: authored({ detail: "batch:life/a1b2c3d4\n\nheld" }) }));
  const batches = new Map([["life", { chain: "life", batches: [grown()], reviews: [] }]]);
  const row = rowOf(held, new Map(), batches);
  assert.ok(row.kind === "batch");
  assert.equal(row.batch.head, "e5f6a7b8");
  assert.deepEqual([...row.reviewed], ["a1b2c3d4"]);
});

test("oneCardPerBatch: a batch held twice is drawn once, by the decision that saw the most", () => {
  const batches = new Map([["life", { chain: "life", batches: [grown()], reviews: [] }]]);
  const first = rowOf(decision(entry({ todo: "d1", authored: authored({ detail: "batch:life/a1b2c3d4" }) })), new Map(), batches);
  const second = rowOf(decision(entry({ todo: "d2", authored: authored({ detail: "batch:life/e5f6a7b8" }) })), new Map(), batches);
  const other = rowOf(decision(entry({ todo: "d3", authored: authored({ detail: "garbage" }) })), new Map(), batches);
  assert.deepEqual(oneCardPerBatch([first, other, second]), [other, second]);
});

test("rowOf: a proposal resolved against a fetched conversation mounts its card", () => {
  const conversations = new Map([["default", conversationPayload()]]);
  const row = rowOf(waiting.proposal(asked()), conversations, new Map());
  assert.equal(row.kind, "proposal");
  assert.ok(row.kind === "proposal" && row.proposal.index === 3);
  assert.ok(row.kind === "proposal" && row.conversation.title === "Default");
});

test("rowOf: a batch reference resolved against a fetched proposals list mounts the batch", () => {
  const held = decision(entry({ authored: authored({ detail: "batch:life/a1b2c3d4\n\nheld" }) }));
  const payload: ProposalsPayload = { chain: "life", batches: [batchOf()], reviews: [] };
  const batches = new Map([["life", payload]]);
  const row = rowOf(held, new Map(), batches);
  assert.equal(row.kind, "batch");
  assert.ok(row.kind === "batch" && row.batch.head === "a1b2c3d4");
});

test("rowOf: a proposal index this fetch does not carry is unresolved, never a half-built card", () => {
  const conversations = new Map([["default", conversationPayload({ proposals: [] })]]);
  const row = rowOf(waiting.proposal(asked()), conversations, new Map());
  assert.equal(row.kind, "unresolved");
});

test("rowOf: a batch head settled through another door is unresolved, not silently blank", () => {
  const held = decision(entry({ authored: authored({ detail: "batch:life/a1b2c3d4\n\nheld" }) }));
  const payload: ProposalsPayload = { chain: "life", batches: [], reviews: [] };
  const batches = new Map([["life", payload]]);
  const row = rowOf(held, new Map(), batches);
  assert.equal(row.kind, "unresolved");
});

test("rowOf: an unparseable reference never even asks the maps", () => {
  const row = rowOf(decision(entry({ authored: authored({ detail: "garbage" }) })), new Map(), new Map());
  assert.equal(row.kind, "unresolved");
});

test("sameRows: a coding session handed over is a different row, so its card is drawn again", () => {
  const reported = { state: "reported" as const, id: "s1", branch: "claude/x", path: "docs/reports/x.md", report: "done", stat: [], handed: "" };
  const of = (coding: ProposalRow["coding"]) =>
    rowOf(waiting.proposal(asked()), new Map([["default", conversationPayload({ proposals: [proposalRow({ coding })] })]]), new Map());
  assert.ok(sameRows([of(reported)], [of(reported)]));
  assert.ok(!sameRows([of(reported)], [of({ ...reported, handed: "2026-10-07T10:00:00" })]));
});

// --- the box's setup cards, folded into one row ---------------------------------

test("law: only the setup cards go after the work, each side in the order it came, and nothing is lost or added", () =>
  forAll(300, (r) => {
    const rows: Row[] = Array.from({ length: r.int(9) }, (_, i): Row => {
      const by = r.pick<ProposalRow["card"]["by"]>([{ kind: "secretary" }, { kind: "setup" }, { kind: "box" }, { kind: "plugin", name: "due" }]);
      const proposal = proposalRow({ index: i, card: aCard({ by }) });
      return r.int(4) === 0
        ? { kind: "unresolved", w: waiting.proposal(asked({ id: `u${i}` })), problem: "gone" }
        : { kind: "proposal", w: waiting.proposal(asked({ id: `p${i}` })), conversation: conversationPayload({ proposals: [proposal] }), proposal };
    });
    const { work, setup } = splitSetup(rows);
    // Only a setup card folds: an unlock, a sign-in, a grant or an erase card of the box's stays among the work.
    const isSetup = (row: Row): boolean => row.kind === "proposal" && row.proposal.card.by.kind === "setup";
    assert.deepEqual(setup, rows.filter(isSetup));
    assert.deepEqual(work, rows.filter((row) => !isSetup(row)));
  }));

test("the legend counts what is shown: the work, and the setup steps folded under it, apart", () => {
  assert.equal(waitingSentence(0, 0), "");
  assert.equal(waitingSentence(1, 0), "1 thing waiting on you.");
  assert.equal(waitingSentence(2, 0), "2 things waiting on you.");
  assert.equal(waitingSentence(0, 1), "1 setup step waiting on you.");
  assert.equal(waitingSentence(2, 2), "2 waiting on you, and 2 setup steps.");
  // The total is never said when part of it is folded away: it is the count the audit found shown nowhere.
  for (let work = 1; work < 6; work += 1)
    for (let steps = 1; steps < 6; steps += 1)
      assert.ok(!waitingSentence(work, steps).startsWith(`${work + steps} `), waitingSentence(work, steps));
});

test("a card's tag names who asked in words: none for the secretary, Setup for the box with why in its tip", () => {
  assert.equal(asker.tag({ kind: "secretary" }), null);
  assert.equal(asker.tag({ kind: "setup" })?.text, "Setup");
  assert.match(asker.tag({ kind: "setup" })?.tip ?? "", /not set up yet/);
  assert.equal(asker.tag({ kind: "box" })?.text, "App");
  assert.equal(asker.tag({ kind: "plugin", name: "due" })?.text, "due plugin");
  assert.equal(waiting.source(waiting.proposal(asked({ by: { kind: "setup" } }))), "Setup");
});

test("law: a card's source is named in words, never by its spelling, and a todo by its title when this device holds it", () =>
  forAll(300, (r) => {
    const text = r.pick(["todo:life/file-permit", "mail:a1b2c3d4e5f6", "thread:t-one", "briefing:2026-09-17", "proposal:setup/3", "batch:life/a1b2c3d4"]);
    const reference = parse(text);
    assert.ok(reference !== null);
    const held = r.bool() ? [entry({ todo: "file-permit", authored: authored({ body: "Renew the *permit*\nby Friday" }) })] : [];
    const said = spoken.of(reference, spoken.titles(held));
    assert.ok(!said.includes(":"), `${text} is said as ${said}`);
    if (reference.kind === "todo") assert.equal(said, held.length === 0 ? "a todo" : "Renew the permit");
  }));
