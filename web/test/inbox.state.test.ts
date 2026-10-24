// The Inbox's fold: the frame's step over the page's value, one test per event, and the random walk.

import { test } from "node:test";
import assert from "node:assert/strict";

import type { Entry } from "../src/wire/generated/wire";
import type { InboxProposal } from "../src/wire/generated/wire";
import type { Row } from "../src/inbox/inbox";
import waiting from "../src/inbox/waiting";
import inboxState from "../src/pages/inbox.state";
import intents from "../src/prodrome/intents";
import frame from "../src/view/frame";
import { authoredAs, forAll, rng, type Rng } from "./gen";
import { aCard } from "./cards";
import lo from "../src/algebra/lo";

type Loaded = inboxState.T;
type Event = frame.Event<Loaded, inboxState.Event>;
const step = frame.step(inboxState.step, inboxState.keeps);
const answer = (rows: Row[], settled: waiting.Settled[], me: string | null): Event =>
  frame.read(lo.val({ rows, settled, unreadable: [], unplaced: [], me, writes: me !== null, expanded: new Set(), settledOpen: false, opening: null, problem: null, box: "Answered", intents: intents.EMPTY, titles: new Map() }));

function entryOf(r: Rng): Entry {
  return {
    todo: r.string("abc", 6),
    state: r.pick(["open", "completed", "cancelled"]),
    states: [],
    at: r.string("0123456789T:-", 8),
    claimed: "",
    unconfirmed: false,
    recurring: false,
    value: r.bool() ? r.int(100) / 100 : null,
    conflicts: {},
    authored: authoredAs({ kind: "decision" }),
    candidates: [],
    stream: [],
  };
}

function proposalOf(r: Rng): InboxProposal {
  return {
    id: `p${r.string("0123456789abcdef", 8)}`,
    asked: `proposal:${r.string("abc", 4)}/${r.int(5)}`,
    serves: null,
    headline: r.string("abc ", 8),
    by: { kind: "secretary" },
    why: r.string("abc ", 8),
    state: r.pick<InboxProposal["state"]>([{ kind: "open" }, { kind: "completed", at: "2026-09-17T08:30:00" }, { kind: "cancelled", at: "2026-09-17T08:30:00" }]),
    value: null,
    at: r.string("0123456789T:-", 8),
  };
}

/** Something waiting, whichever of the two kinds. */
function waitingOf(r: Rng): waiting.Waiting {
  return r.bool()
    ? waiting.proposal(proposalOf(r))
    : waiting.decision(r.pick(["life", "suzatary"] as const), entryOf(r));
}

/** A row, whichever of the two kinds: the state does not care which. */
function row(r: Rng): Row {
  switch (r.int(2)) {
    case 0:
      return {
        kind: "proposal",
        w: waitingOf(r),
        conversation: {
          id: r.string("abc", 4),
          title: r.string("abc ", 8),
          model: "test",
          opened: "",
          turns: [],
          proposals: [],
          unplaced: [],
          state: "idle",
          first: 0,
          total: 0,
          again: null,
        },
        proposal: {
          index: r.int(5),
          act: { kind: "email", account: "a@b.test", to: [], cc: [], subject: "", body: "", attachments: [], held: { type: "Read", values: [] } },
          card: aCard(),
          why: "",
          sources: [],
          at: "",
          state: "standing",
          outcome: "",
          tries: 0,
          turn: 0,
          coding: null,
          announced: null,
          revises: null,
        },
      };
    default:
      return { kind: "unresolved", w: waitingOf(r), problem: r.string("abc ", 8) };
  }
}

function arrived(r: Rng): Event {
  return answer(r.array(r.int(3), () => row(r)), r.array(r.int(3), (): waiting.Settled => ({ w: waitingOf(r), how: r.pick(["completed", "cancelled"] as const), at: "2026-09-17T08:30:00" })), r.bool() ? null : r.string("abc", 4));
}

function event(r: Rng): Event {
  switch (r.int(5)) {
    case 0:
      return arrived(r);
    case 1:
      return frame.read(lo.error(r.string("abc ", 8)));
    case 2:
      return frame.own({ type: "SettledOpen", open: r.bool() });
    case 3:
      return frame.own({ type: "Expanded", key: r.pick(["k1", "k2"]), open: r.bool() });
    default:
      return frame.own({ type: "Writes", me: r.bool() ? null : r.string("abc", 4), writes: r.bool() });
  }
}

test("arrived: loads the payload, and the FIRST answer starts with a closed section", () => {
  const next = step(lo.loading, arrived(rng(1)));
  const value = lo.withDefault(next, null);
  assert.ok(value !== null);
  assert.equal(value?.settledOpen, false);
});

test("arrived: a later answer carries the openness forward", () => {
  const first = step(lo.loading, arrived(rng(2)));
  const opened = step(first, frame.own({ type: "SettledOpen", open: true }));
  const again = step(opened, arrived(rng(3)));
  assert.equal(lo.withDefault(again, null)?.settledOpen, true);
});

test("failed: fails the page when nothing has been read, and leaves the queue once something has", () => {
  const first = step(lo.loading, frame.read(lo.error("no answer")));
  assert.equal(first.type, "Error");
  const withData = step(lo.loading, answer([], [], null));
  assert.equal(step(withData, frame.read(lo.error("no answer"))), withData);
});

test("settledOpen and me: set once, no-op on repeat", () => {
  const start = step(lo.loading, answer([], [], null));
  const opened = step(start, frame.own({ type: "SettledOpen", open: true }));
  assert.equal(lo.withDefault(opened, null)?.settledOpen, true);
  assert.equal(step(opened, frame.own({ type: "SettledOpen", open: true })), opened);
  const known = step(start, frame.own({ type: "Writes", me: "ada", writes: true }));
  assert.equal(lo.withDefault(known, null)?.me, "ada");
  assert.equal(step(known, frame.own({ type: "Writes", me: "ada", writes: true })), known);
});

test("a key trusted since moves the Inbox with the same writer, so its cards draw their acts", () => {
  const untrusted = step(step(lo.loading, answer([], [], "ada")), frame.own({ type: "Writes", me: "ada", writes: false }));
  const trusted = step(untrusted, frame.own({ type: "Writes", me: "ada", writes: true }));
  assert.notEqual(trusted, untrusted);
  assert.equal(lo.withDefault(trusted, null)?.writes, true);
});

test("expanded: a diff opens and closes by its key, a repeat is a no-op, and a refresh keeps it", () => {
  const start = step(lo.loading, answer([], [], null));
  const opened = step(start, frame.own({ type: "Expanded", key: "k1", open: true }));
  assert.deepEqual([...(lo.withDefault(opened, null)?.expanded ?? [])], ["k1"]);
  assert.equal(step(opened, frame.own({ type: "Expanded", key: "k1", open: true })), opened);
  assert.equal(step(start, frame.own({ type: "Expanded", key: "k1", open: false })), start);
  assert.deepEqual([...(lo.withDefault(step(opened, answer([], [], null)), null)?.expanded ?? [])], ["k1"]);
  const closed = step(opened, frame.own({ type: "Expanded", key: "k1", open: false }));
  assert.equal(lo.withDefault(closed, null)?.expanded.size, 0);
});

test("opening: the row the address opened stays marked across readings, and is scrolled to once", () => {
  const opened: Event = frame.read(lo.val({ rows: [], settled: [], unreadable: [], unplaced: [], me: null, writes: false, expanded: new Set(), settledOpen: false, opening: { key: "k", stage: "Pending" }, problem: null, box: "Answered", intents: intents.EMPTY, titles: new Map() }));
  const first = step(lo.loading, opened);
  assert.deepEqual(lo.withDefault(step(first, answer([], [], null)), null)?.opening, { key: "k", stage: "Pending" });
  const scrolled = step(first, frame.own({ type: "Scrolled" }));
  assert.deepEqual(lo.withDefault(scrolled, null)?.opening, { key: "k", stage: "Scrolled" });
  assert.equal(step(scrolled, frame.own({ type: "Scrolled" })), scrolled);
  assert.deepEqual(lo.withDefault(step(scrolled, opened), null)?.opening, { key: "k", stage: "Scrolled" });
});

test("law: the state after any walk is the fold of its events", () => {
  forAll(300, (r) => {
    const events = r.array(r.int(40), () => event(r));
    let state: lo.T<Loaded> = lo.loading;
    for (const e of events) {
      state = step(state, e);
    }
    assert.deepEqual(state, events.reduce(step, lo.loading));
  });
});

test("a link to a setup card unfolds the Setup row once; closed by the reader, no later reading opens it again", () => {
  const setup: Row = {
    kind: "proposal",
    w: waiting.proposal({ id: "p9", asked: "proposal:setup/0", serves: null, headline: "", by: { kind: "setup" }, why: "", state: { kind: "open" }, value: null, at: "" }),
    conversation: { id: "setup", title: "Setup", model: "test", opened: "", turns: [], proposals: [], unplaced: [], state: "idle", first: 0, total: 0, again: null },
    proposal: {
      index: 0,
      act: { kind: "email", account: "a@b.test", to: [], cc: [], subject: "", body: "", attachments: [], held: { type: "Read", values: [] } },
      card: aCard({ by: { kind: "setup" } }),
      why: "",
      sources: [],
      at: "",
      state: "standing",
      outcome: "",
      tries: 0,
      turn: 0,
      coding: null,
      announced: null,
      revises: null,
    },
  };
  const page = (rows: Row[]): Loaded => ({ rows, settled: [], unreadable: [], unplaced: [], me: null, writes: false, expanded: new Set(), settledOpen: false, opening: { key: "p9", stage: "Pending" }, problem: null, box: "Answered", intents: intents.EMPTY, titles: new Map() });
  // A reading that does not hold the card yet unfolds nothing and keeps waiting for it.
  assert.equal(inboxState.opened(page([])).expanded.has(inboxState.SETUP), false);
  const first = inboxState.opened(page([setup]));
  assert.equal(first.expanded.has(inboxState.SETUP), true);
  assert.equal(first.opening?.stage, "Unfolded");
  const closed = inboxState.step(first, { type: "Expanded", key: inboxState.SETUP, open: false });
  const again = inboxState.keeps(closed, page([setup]));
  assert.equal(again.expanded.has(inboxState.SETUP), false, "the reader's close stands");
  assert.equal(inboxState.keeps(inboxState.step(closed, { type: "Scrolled" }), page([setup])).expanded.has(inboxState.SETUP), false);
});
