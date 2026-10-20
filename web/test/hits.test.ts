/**
 * The palette's async adapters, pure (`src/hits.ts`'s own header says why
 * they are separate from `palette.ts`'s DOM half).
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { hitOfConversation, hitOfMail, needsAsync } from "../src/shell/hits";
import type { ConversationRow } from "../src/wire/generated/wire";
import type { MailRow } from "../src/wire/generated/wire";
import cast from "./cast";

const { counsel } = cast.world;

const MAIL: MailRow = {
  hash: "a1b2c3",
  id: "notmuch-id-1",
  sender: counsel.email,
  subject: "The deed documents",
  date: "2026-09-10",
  container: "inbox",
  account: cast.world.owner.email,
  disposition: { kind: "undecided" },
};

const CONVERSATION: ConversationRow = {
  id: "2026-09-14a",
  title: "Untitled",
  kind: "topic",
  opened: "2026-09-14T08:00:00Z",
  turns: 3,
  standing: 0,
  unplaced: [],
  last: "",
  last_at: "2026-09-14T08:00:00Z",
};

test("a mail row becomes a message Hit routed at its own id", () => {
  const hit = hitOfMail(MAIL);
  assert.equal(hit.kind, "message");
  assert.equal(hit.title, "The deed documents");
  assert.equal(hit.subtitle, counsel.email);
  assert.deepEqual(hit.route, { type: "message", id: "notmuch-id-1" });
  // Findable by sender as well as subject.
  assert.ok(hit.search.includes("deed"));
  assert.ok(hit.search.includes(counsel.email));
});

test("a conversation row becomes a conversation Hit carrying its turn count", () => {
  const hit = hitOfConversation(CONVERSATION);
  assert.equal(hit.kind, "conversation");
  assert.equal(hit.title, "Untitled");
  assert.equal(hit.subtitle, "3 turns");
  assert.deepEqual(hit.route, { type: "conversation", id: "2026-09-14a", refs: [] });
});

test("a single turn is singular", () => {
  assert.equal(hitOfConversation({ ...CONVERSATION, turns: 1 }).subtitle, "1 turn");
});

test("a query under two characters asks no async source", () => {
  assert.equal(needsAsync(""), false);
  assert.equal(needsAsync("f"), false);
  assert.equal(needsAsync(" f "), false);
  assert.equal(needsAsync("fr"), true);
  assert.equal(needsAsync(counsel.name.toLowerCase()), true);
});
