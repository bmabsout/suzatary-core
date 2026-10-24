// The Inbox's fold: the frame's step over the page's value, one test per event, and the random walk.

import { test } from "node:test";
import assert from "node:assert/strict";

import type { Entry } from "../src/wire/generated/wire";
import type { ProposedObject } from "../src/wire/generated/wire";
import type { InboxProposal, ProposedBatch } from "../src/wire/generated/wire";
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
const { pickedFor } = inboxState;
const answer = (rows: Row[], settled: waiting.Settled[], me: string | null): Event =>
  frame.read(lo.val({ rows, settled, unreadable: [], unplaced: [], me, writes: me !== null, picks: new Map(), expanded: new Set(), settledOpen: false, opening: null, problem: null, box: "Answered", intents: intents.EMPTY, titles: new Map() }));

function object(r: Rng, hash: string, needs: string[]): ProposedObject {
  return {
    hash,
    act: r.pick(["added", "edited", "completed", "cancelled", "reopened"]),
    todo: r.string("abc", 3),
    body: r.string("abc ", 8),
    at: r.string("0123456789T:-", 8),
    actor: r.string("abc", 4),
    note: "",
    changes: [],
    needs,
    pending_todo: false,
  };
}

function objects(r: Rng, n: number): ProposedObject[] {
  const hashes = Array.from({ length: n }, (_, i) => `h${i}`);
  // Each object may need any EARLIER one — closed by construction, so a
  // generated chain is always a valid `needs` graph.
  return hashes.map((hash, i) => object(r, hash, hashes.slice(0, i).filter(() => r.bool(0.3))));
}

function batch(r: Rng): ProposedBatch {
  const objs = objects(r, 1 + r.int(4));
  return {
    head: r.string("0123456789abcdef", 8),
    at: r.string("0123456789T:-", 8),
    actor: r.string("abc", 4),
    objects: objs,
  };
}

/** A batch row whose decision saw the whole batch, as a fresh hold does. */
function batchOf(r: Rng): Row {
  const b = batch(r);
  return {
    kind: "batch",
    w: waitingOf(r),
    entry: entryOf(r),
    chain: r.pick(["life", "suzatary"] as const),
    batch: b,
    reviewed: new Set(b.objects.map((o) => o.hash)),
  };
}

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

/** A row, whichever of the three kinds — the state does not care which, only
 * whether a batch row's head has a pick to touch. */
function row(r: Rng): Row {
  switch (r.int(3)) {
    case 0:
      return batchOf(r);
    case 1:
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
  switch (r.int(7)) {
    case 0:
      return arrived(r);
    case 1:
      return frame.read(lo.error(r.string("abc ", 8)));
    case 2: {
      const objs = objects(r, 1 + r.int(4));
      const hash = r.pick(objs).hash;
      return frame.own({ type: "Toggled", head: r.string("0123456789abcdef", 8), hash, on: r.bool() });
    }
    case 3:
      return frame.own({ type: "Settled", head: r.string("0123456789abcdef", 8) });
    case 4:
      return frame.own({ type: "SettledOpen", open: r.bool() });
    case 5:
      return frame.own({ type: "Expanded", key: r.pick(["k1", "k2"]), open: r.bool() });
    default:
      return frame.own({ type: "Writes", me: r.bool() ? null : r.string("abc", 4), writes: r.bool() });
  }
}

test("arrived: loads the payload, and the FIRST answer starts with no ticks and a closed section", () => {
  const next = step(lo.loading, arrived(rng(1)));
  const value = lo.withDefault(next, null);
  assert.ok(value !== null);
  assert.equal(value?.picks.size, 0);
  assert.equal(value?.settledOpen, false);
});

test("arrived: a later answer carries the ticks and the openness forward", () => {
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

/** A fixed `ProposedObject`, for tests that care about `hash` and `needs`
 * only — the rest of the fields are unread by `closure`/`pickedFor`. */
function fixed(hash: string, needs: string[]): ProposedObject {
  return { hash, act: "added", todo: "t1", body: "", at: "", actor: "", changes: [], needs, pending_todo: false };
}

/** One `batch`-kind row, carrying `b` — `toggled`'s own guard reads the
 * state's `rows` for a matching head before it touches `picks`, so a walk
 * that never arrived at a batch this head names is a no-op and not a crash. */
function batchRow(b: ProposedBatch): Row & { kind: "batch" } {
  const e = entryOf(rng(1));
  return { kind: "batch", w: waiting.decision("life", e), entry: e, chain: "life", batch: b, reviewed: new Set(b.objects.map((o) => o.hash)) };
}

test("toggled: ticking takes everything a row rests on, unticking drops what rests on it", () => {
  // a <- b <- c (c needs b, b needs a)
  const objs: ProposedObject[] = [fixed("a", []), fixed("b", ["a"]), fixed("c", ["b"])];
  const b: ProposedBatch = { head: "batch1", at: "", actor: "", objects: objs };
  const start = step(lo.loading, answer([batchRow(b)], [], null));
  const ticked = step(start, frame.own({ type: "Toggled", head: "batch1", hash: "c", on: true }));
  assert.deepEqual([...(lo.withDefault(ticked, null)?.picks.get("batch1") ?? [])].sort(), ["a", "b", "c"]);
  const unticked = step(ticked, frame.own({ type: "Toggled", head: "batch1", hash: "a", on: false }));
  assert.deepEqual([...(lo.withDefault(unticked, null)?.picks.get("batch1") ?? [])], []);
});

test("toggled: a head no row carries is a no-op", () => {
  const start = step(lo.loading, answer([], [], null));
  const same = step(start, frame.own({ type: "Toggled", head: "nosuchbatch", hash: "a", on: true }));
  assert.equal(same, start);
});

test("pickedFor: a batch nobody touched reads as what its decision reviewed", () => {
  const b: ProposedBatch = {
    head: "h1",
    at: "",
    actor: "",
    objects: [fixed("x", []), fixed("y", ["x"])],
  };
  assert.deepEqual([...pickedFor(new Map(), batchRow(b))].sort(), ["x", "y"]);
  const held = { ...batchRow(b), reviewed: new Set(["x"]) };
  assert.deepEqual([...pickedFor(new Map(), held)], ["x"]);
  // Ticking what came after the hold takes it, and keeps what was reviewed.
  const start = step(lo.loading, answer([held], [], null));
  const ticked = step(start, frame.own({ type: "Toggled", head: "h1", hash: "y", on: true }));
  assert.deepEqual([...(lo.withDefault(ticked, null)?.picks.get("h1") ?? [])].sort(), ["x", "y"]);
});

test("settled: clears a batch's ticks, and settling an untouched batch is a no-op", () => {
  const objs: ProposedObject[] = [fixed("x", [])];
  const b: ProposedBatch = { head: "h1", at: "", actor: "", objects: objs };
  const start = step(lo.loading, answer([batchRow(b)], [], null));
  const ticked = step(start, frame.own({ type: "Toggled", head: "h1", hash: "x", on: true }));
  assert.ok(lo.withDefault(ticked, null)?.picks.has("h1"));
  const cleared = step(ticked, frame.own({ type: "Settled", head: "h1" }));
  assert.ok(!lo.withDefault(cleared, null)?.picks.has("h1"));
  assert.equal(step(cleared, frame.own({ type: "Settled", head: "h1" })), cleared);
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
  const opened: Event = frame.read(lo.val({ rows: [], settled: [], unreadable: [], unplaced: [], me: null, writes: false, picks: new Map(), expanded: new Set(), settledOpen: false, opening: { key: "k", stage: "Pending" }, problem: null, box: "Answered", intents: intents.EMPTY, titles: new Map() }));
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
    },
  };
  const page = (rows: Row[]): Loaded => ({ rows, settled: [], unreadable: [], unplaced: [], me: null, writes: false, picks: new Map(), expanded: new Set(), settledOpen: false, opening: { key: "p9", stage: "Pending" }, problem: null, box: "Answered", intents: intents.EMPTY, titles: new Map() });
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
