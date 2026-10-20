/**
 * The write side, as functions.
 *
 * Three things are pinned here and each of them is a mechanism the app rests
 * on. The REQUEST carries the act header — that is the CSRF half of "a browser
 * tab is a trusted actor", and `gate::check_csrf` refuses without it,
 * so a build that dropped it would produce an app whose every button 403s. The
 * FORM's rule is a total function from state to either a payload or a sentence,
 * so an invalid draft is a message rather than a request the server has to
 * refuse. And APPLICABILITY is per state, so the page never offers an act that
 * cannot apply.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import payload from "../src/wire/generated/payload";
import d from "../src/wire/decoder";
import http from "../src/wire/http";
import type { Entry } from "../src/wire/generated/wire";
import type { Draft } from "../src/todo/draft";
import { draft, emptyDraft, preselect } from "../src/todo/draft";
import parsed from "../src/algebra/parsed";
import share from "../src/todo/share";
import form from "../src/view/form";
import price from "../src/todo/price";
import { applicable } from "../src/todo/todos";

const VOCABULARY = ["work", "study", "project", "home", "other"];

// --- the request -------------------------------------------------------------

test("a write carries the act header, the JSON content type and the POST method", () => {
  const request = http.actRequest({ todo: "file83b" });
  assert.equal(request.method, "POST");
  const headers = request.headers as Record<string, string>;
  assert.equal(headers[http.ACT_HEADER], "1");
  assert.equal(headers["content-type"], "application/json");
  assert.equal(request.body, '{"todo":"file83b"}');
});

test("the server's own words are what a refusal says", () => {
  assert.equal(http.errorOf({ error: "ada.login@example.com may read this app but not write to it" }), "ada.login@example.com may read this app but not write to it");
  assert.equal(http.errorOf({}), null);
  assert.equal(http.errorOf(null), null);
});

test("a post's receipt is guarded like every other payload", () => {
  const receipt = { heads: ["ab12"], adopted: ["ab12"], already: [], refused: [] };
  assert.equal(d.is(payload.ReceiptPayload)(receipt), true);
  // All the lists are ALWAYS sent, so an answer missing one is a server this
  // build does not know rather than an empty list to be assumed.
  assert.equal(d.is(payload.ReceiptPayload)({ ...receipt, already: undefined }), false);
  assert.equal(d.is(payload.ReceiptPayload)({ ...receipt, heads: "ab12" }), false, "the heads are a set, never one");
  assert.equal(
    d.is(payload.ReceiptPayload)({ ...receipt, refused: [{ hash: "cd", reason: "rests on … which this box does not hold", kind: "final" }] }),
    true,
  );
  // Whether to post it again is the box's to say: a refusal with no kind, or another, is no answer this build reads.
  assert.equal(d.is(payload.ReceiptPayload)({ ...receipt, refused: [{ hash: "cd", reason: "unsigned" }] }), false);
  assert.equal(d.is(payload.ReceiptPayload)({ ...receipt, refused: [{ hash: "cd", reason: "unsigned", kind: "later" }] }), false);
  // A refusal with no reason is a refusal that says nothing, which is the one
  // thing this app must never render.
  assert.equal(d.is(payload.ReceiptPayload)({ ...receipt, refused: [{ hash: "cd" }] }), false);
});

test("the authoring's answer is an event print and the todo it is about", () => {
  assert.equal(
    d.is(payload.AuthoredPayload)({ todo: "renewthelease", event: "Authored(todo='renewthelease', …)", revision: null }),
    true,
  );
  assert.equal(
    d.is(payload.AuthoredPayload)({ todo: "boardingpass", event: "Authored(todo='boardingpass', …)", revision: "Authored(…)" }),
    true,
    "a subtodo carries its parent's revision",
  );
  assert.equal(d.is(payload.AuthoredPayload)({ todo: "renewthelease", revision: null }), false, "an answer with no event authored nothing");
  assert.equal(d.is(payload.AuthoredPayload)({ event: "Authored(…)", revision: null }), false);
  assert.equal(d.is(payload.AuthoredPayload)({ todo: "renewthelease", event: "Authored(…)" }), false, "revision is null, never absent");
});

// --- the form ----------------------------------------------------------------

const body = "Renew the lease";

test("a flat draft becomes a whole-percentage ratio", () => {
  const drafted = draft(form.fill(emptyDraft(VOCABULARY), { body, category: "home", price: price.flat(0.3512) }));
  assert.equal(drafted.type, "Ok");
  if (drafted.type !== "Ok") return;
  assert.deepEqual(drafted.v.priority, { how: "flat", percent: 35 });
  assert.equal(drafted.v.body, body);
  assert.equal(drafted.v.category, "home");
  assert.equal(drafted.v.rationale, "");
});

test("a deadline draft carries the date, the lead-up and the start", () => {
  const drafted = draft(form.fill(emptyDraft(VOCABULARY), { body, category: "study", price: price.deadline("2026-09-20", 10) }));
  assert.equal(drafted.type, "Ok");
  if (drafted.type !== "Ok") return;
  assert.deepEqual(drafted.v.priority, {
    how: "deadline",
    end_date: "2026-09-20",
    start_percent: price.DEADLINE_START_PERCENT,
    end_percent: price.DEADLINE_END_PERCENT,
    lead_up_weeks: null,
    lead_up_days: 10,
    start_date: null,
  });
});

test("a recur or periodic draft carries the whole cycle, tagged by its own kind", () => {
  const cycle = { days: 60, from: 1, to: 0.3 };
  const recurring = draft(form.fill(emptyDraft(VOCABULARY), { body, category: "home", price: price.recur(cycle) }));
  assert.equal(recurring.type, "Ok");
  if (recurring.type === "Ok") assert.deepEqual(recurring.v.priority, { how: "recur", days: 60, from_percent: 100, to_percent: 30 });

  const periodic = draft(form.fill(emptyDraft(VOCABULARY), { body, category: "home", price: price.periodic(cycle) }));
  assert.equal(periodic.type, "Ok");
  if (periodic.type === "Ok") assert.deepEqual(periodic.v.priority, { how: "periodic", days: 60, from_percent: 100, to_percent: 30 });
});

test("an incomplete draft is a sentence about the field it names, never a request", () => {
  const cases: [form.Values<Draft>, RegExp, keyof Draft][] = [
    [{}, /what the todo is/i, "body"],
    [{ body: "   ", category: "home" }, /what the todo is/i, "body"],
    [{ body }, /category/i, "category"],
    [{ body, category: "home", price: price.deadline("", 14) }, /deadline date/i, "price"],
    [{ body, category: "home", price: price.deadline("20/09/2026", 14) }, /deadline date/i, "price"],
    [{ body, category: "home", price: price.deadline("2026-09-20", 0) }, /lead-up/i, "price"],
    [{ body, category: "home", price: price.deadline("2026-09-20", 2.5) }, /lead-up/i, "price"],
    [{ body, category: "home", price: price.recur({ days: 0, from: 1, to: 0.3 }) }, /1 to 3650 days/i, "price"],
    [{ body, category: "home", price: price.periodic({ days: 3651, from: 1, to: 0.3 }) }, /1 to 3650 days/i, "price"],
  ];
  for (const [over, problem, about] of cases) {
    const drafted = draft(form.fill(emptyDraft(VOCABULARY), over));
    assert.equal(drafted.type, "Refused", JSON.stringify(over));
    if (drafted.type !== "Refused") continue;
    assert.match(drafted.reason, problem);
    assert.deepEqual(drafted.about, parsed.field(about), JSON.stringify(over));
  }
});

test("a new todo starts in the category the list is narrowed to, else the last one used, else none", () => {
  assert.equal(preselect(VOCABULARY, "study", "home"), "study");
  assert.equal(preselect(VOCABULARY, "", "home"), "home");
  assert.equal(preselect(VOCABULARY, "", ""), "");
  // A category the deployment no longer has is never chosen.
  assert.equal(preselect(VOCABULARY, "gone", "home"), "home");
  assert.equal(preselect(VOCABULARY, "", "gone"), "");
  assert.equal(emptyDraft(VOCABULARY, "home").category.v, "home");
  assert.equal(emptyDraft(VOCABULARY).category.v, "");
});

test("the slider's value never leaves [0, 1] or a whole percent", () => {
  assert.equal(price.wholePercent(0.3512), 0.35);
  assert.equal(price.wholePercent(-1), 0);
  assert.equal(price.wholePercent(9), 1);
});

// --- switching kind (docs/TS-STYLE.md's tagged sum) ---------------------------

test("switching between the two cycles keeps the numbers typed so far", () => {
  const cycle = { days: 45, from: 0.9, to: 0.2 };
  const recurring = price.recur(cycle);
  assert.deepEqual(price.withKind(recurring, "Periodic"), price.periodic(cycle));
  assert.deepEqual(price.withKind(price.withKind(recurring, "Periodic"), "Recur"), recurring);
});

test("switching to a kind that shares nothing starts that kind fresh", () => {
  const recurring = price.recur({ days: 45, from: 0.9, to: 0.2 });
  assert.deepEqual(price.withKind(recurring, "Flat"), price.flat(price.DEFAULT_FLAT));
  assert.deepEqual(price.withKind(recurring, "Deadline"), price.deadline("", price.DEFAULT_LEAD_UP_DAYS));
});

test("switching to the kind a form is already in is the identity", () => {
  for (const form of [
    price.flat(0.42),
    price.deadline("2026-09-20", 21),
    price.recur({ days: 10, from: 1, to: 0.5 }),
    price.periodic({ days: 10, from: 1, to: 0.5 }),
  ]) {
    assert.deepEqual(price.withKind(form, form.type), form);
  }
});

// --- what a share means ------------------------------------------------------

/**
 * The four shapes Android's share sheet actually produces, and the fifth that
 * is the reason this is a function rather than a template: Chrome shares a
 * page as ("https://…", "The title"), a text selection as (the text, nothing),
 * some readers as (nothing, a heading), and at least one as ("Title —
 * https://…", "Title") — where naively joining says the title twice.
 */
test("a share becomes one body, and never says the title twice", () => {
  assert.equal(share.body("https://example.test/a", "The title"), "The title\nhttps://example.test/a");
  assert.equal(share.body("Renew the lease", ""), "Renew the lease");
  assert.equal(share.body("", "Just a heading"), "Just a heading");
  assert.equal(share.body("The title — https://example.test/a", "The title"), "The title — https://example.test/a");
  // Whitespace is the share sheet's, not the author's.
  assert.equal(share.body("  https://example.test/a\n", "  The title "), "The title\nhttps://example.test/a");
  // Total: nothing shared is the empty body the sheet already refuses, with a
  // sentence rather than a request.
  assert.equal(share.body("", ""), "");
  assert.equal(draft(form.fill(emptyDraft(VOCABULARY), { body: share.body("", ""), category: "home" })).type, "Refused");
});

// --- what applies ------------------------------------------------------------

function entry(over: Partial<Entry> = {}): Entry {
  return {
    todo: "file83b",
    state: "open",
    states: ["open"],
    at: "",
    claimed: "",
    unconfirmed: false,
    recurring: false,
    value: 0.4,
    conflicts: {},
    authored: null,
    candidates: [],
    stream: [],
    ...over,
  };
}

test("an open todo can be finished or dropped; a resolved one can come back", () => {
  assert.deepEqual(applicable(entry()), ["done", "cancel"]);
  assert.deepEqual(applicable(entry({ state: "completed", at: "2026-09-03" })), ["reopen"]);
  assert.deepEqual(applicable(entry({ state: "cancelled", at: "2026-09-03" })), ["reopen"]);
});

test("confirm is offered exactly when the SERVER says the todo is unconfirmed", () => {
  assert.deepEqual(applicable(entry({ unconfirmed: true })), ["done", "cancel", "confirm"]);
  assert.deepEqual(applicable(entry({ state: "completed", unconfirmed: true })), ["reopen", "confirm"]);
  // The roster lives on the server; this side reads its verdict and no more.
  assert.equal(applicable(entry({ claimed: "completed" })).includes("confirm"), false);
});
