/**
 * The one "I need something" popup's pure half: a proposal or a decision pops
 * up when it starts waiting after the baseline, never a todo, never one
 * already waiting, never one that is settled; and it links to the Inbox
 * opened at it.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import type { Entry } from "../src/wire/generated/wire";
import type { InboxProposal } from "../src/wire/generated/wire";
import { cardKey, hrefOfDecision, newlyWaiting, onScreen, waitingIds } from "../src/inbox/needsyou";
import waiting from "../src/inbox/waiting";
import { hrefOf, parseRoute } from "../src/shell/route";
import cast from "./cast";
import { authoredAs } from "./gen";

const asked = `Send the ${cast.world.counsel.name} reply`;

function entry(todo: string, over: Partial<Entry> = {}): Entry {
  return {
    todo,
    state: "open",
    states: ["open"],
    at: "",
    claimed: "",
    unconfirmed: false,
    recurring: false,
    value: 0.5,
    conflicts: {},
    authored: authoredAs({ kind: "decision" }),
    candidates: [],
    stream: [],
    ...over,
  };
}

function proposal(id: string, over: Partial<InboxProposal> = {}): InboxProposal {
  return {
    id,
    asked: "proposal:default/3",
    serves: null,
    headline: asked,
    by: { kind: "secretary" },
    why: "he asked",
    state: { kind: "open" },
    value: null,
    at: "2026-09-24T09:00:00",
    ...over,
  };
}

const all = (proposals: InboxProposal[], entries: Entry[]): waiting.Waiting[] =>
  waiting.all(proposals, [{ chain: "life", entries }]);

test("only what starts waiting after the baseline is new", () => {
  const before = waitingIds(all([proposal("pold")], [entry("old"), entry("atodo", { authored: authoredAs({ kind: "todo", name: "todo" }) })]));
  assert.deepEqual([...before].sort(), ["old", "pold"], "a todo is never waiting on him this way");
  const after = all(
    [proposal("pold"), proposal("pfresh"), proposal("psent", { state: { kind: "completed", at: "2026-09-24T10:00:00" } })],
    [entry("old"), entry("fresh"), entry("done", { state: "completed" }), entry("atodo", { authored: authoredAs({ kind: "todo", name: "todo" }) })],
  );
  assert.deepEqual(
    newlyWaiting(before, after, new Set()).map(waiting.key).sort(),
    ["fresh", "pfresh"],
    "not the one already waiting, not a settled one, not a todo",
  );
});

test("a decision links to the Inbox opened at it, and the route round-trips", () => {
  assert.equal(hrefOfDecision("web forms"), "#/inbox?open=web%20forms");
  assert.deepEqual(parseRoute("#/inbox?open=webforms"), { type: "inbox", open: "webforms" });
  assert.deepEqual(parseRoute(hrefOf({ type: "inbox", open: "a b" })), { type: "inbox", open: "a b" });
  assert.deepEqual(parseRoute("#/inbox?open="), { type: "inbox" }, "an empty open is the plain Inbox");
});

test("the popup says what is asked in plain words, or names the decision", () => {
  assert.equal(waiting.asked(waiting.decision("life", entry("x", { authored: authoredAs({ kind: "decision" }, "*_*") }))), "x", "a body of marks alone: the id");
  assert.equal(waiting.asked(waiting.proposal(proposal("p1"))), asked);
});

test("a proposal already on screen does not pop up there", () => {
  const shown = waiting.proposal(proposal("sendreply"));
  assert.equal(onScreen(shown, parseRoute("#/inbox")), true, "the Inbox shows every one");
  assert.equal(onScreen(shown, parseRoute("#/secretary/default")), true, "its own conversation shows it");
  assert.equal(onScreen(shown, parseRoute("#/secretary/other")), false, "another conversation does not");
  assert.equal(onScreen(shown, parseRoute("#/")), false, "the list does not");
});

test("a proposal whose card was already drawn is not new when the inbox names it", () => {
  const after = all(
    [
      proposal("shown", { asked: "proposal:talk/0" }),
      proposal("unseen", { asked: "proposal:talk/1" }),
    ],
    [],
  );
  assert.deepEqual(
    newlyWaiting(new Set(), after, new Set([cardKey("talk", 0)])).map(waiting.key),
    ["unseen"],
    "the card drawn in its conversation already told him",
  );
});
