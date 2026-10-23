/**
 * What a mail row says — the pure half of the mail pages.
 *
 * The one that matters is `badgeOf`: the agent's disposition is a BADGE. The
 * wire carries it as `mail::Disposed`, a closed sum the decoder checks, so a
 * disposition a newer box adds fails this build's type check before it reaches a
 * row; an older page is refused by `gate::CLIENT_MIN` rather than misreading one.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import type { MailRow } from "../src/wire/generated/wire";
import { badgeOf, cursorOf, senderOf, subjectOf, todoOf } from "../src/pages/mailrow";
import cast from "./cast";

const row: MailRow = {
  hash: "aaaaaaaaaaaaaaaa",
  id: "m1@example.com",
  sender: cast.world.counsel.email,
  subject: "Deed signature for lot 4417",
  date: "2026-09-03T09:00:00",
  container: "inbox",
  account: cast.world.owner.email,
  disposition: { kind: "undecided" },
};

test("no decision is no badge, not a badge saying nothing", () => {
  assert.equal(badgeOf({ kind: "undecided" }), null);
});

test("the three dispositions get our words, not the wire's", () => {
  assert.deepEqual(badgeOf({ kind: "todo", todo: "renewcard" }), { label: "todo", kind: "ok" });
  assert.deepEqual(badgeOf({ kind: "noise" }), { label: "noise", kind: "" });
  // `skip` on the wire is "seen, not actionable"; "skip" on a page reads like
  // something was passed over by mistake.
  assert.deepEqual(badgeOf({ kind: "skip" }), { label: "seen", kind: "" });
});

test("only a todo decision names a todo", () => {
  assert.equal(todoOf({ kind: "todo", todo: "renewcard" }), "renewcard");
  assert.equal(todoOf({ kind: "noise" }), null);
  assert.equal(todoOf({ kind: "skip" }), null);
  assert.equal(todoOf({ kind: "undecided" }), null);
});

test("a missing subject says so rather than leaving a gap", () => {
  assert.equal(subjectOf(row), "Deed signature for lot 4417");
  assert.equal(subjectOf({ ...row, subject: "" }), "(no subject)");
  assert.equal(subjectOf({ ...row, subject: "   " }), "(no subject)");
});

test("a sender is shortened to its local part, never emptied", () => {
  assert.equal(senderOf(row), "counsel");
  assert.equal(senderOf({ ...row, sender: "@example.com" }), "@example.com");
  assert.equal(senderOf({ ...row, sender: "postmaster" }), "postmaster");
  assert.equal(senderOf({ ...row, sender: "" }), "");
});

test("the page cursor is the oldest row's instant, and nothing when there are none", () => {
  assert.equal(cursorOf([]), "");
  assert.equal(cursorOf([row, { ...row, date: "2026-09-01T00:00:00" }]), "2026-09-01T00:00:00");
});
