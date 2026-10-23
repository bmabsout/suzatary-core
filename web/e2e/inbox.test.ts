/**
 * THE INBOX, END TO END (`docs/INBOXPRODROME-PLAN-2026-09-17.md` §5 commit 4).
 *
 * A DECISION IS A RECORD, and this file is the browser's own check of that:
 * the real `suzatary serve`, the real fold, a real held batch and a real
 * scripted proposal — so what these scenes read is `view::entries` kept to
 * `kind === "decision"`, exactly the page's own claim.
 *
 * THE SCENES:
 *
 * 1. A held batch appears in the Inbox as an open decision, drawn as its
 *    diff, with the secretary's own reason above it.
 * 2. A viewer reads the Inbox and is offered nothing to press.
 * 3. A standing email proposal appears in the Inbox as an open decision, and
 *    tapping a CHANGESET proposal Confirms it and drops the count — a
 *    changeset, not an email, so it never touches the shared SMTP recorder
 *    `secretary.test.ts` asserts an exact count against.
 * 4. Withdrawing a proposal leaves the Inbox with nothing waiting on it.
 * 5. A revised draft supersedes the one before it: the OLD card shows a quiet
 *    line, never a dismissal in the owner's words, and the Inbox counts only the
 *    new one.
 * 6. A send that failed with nothing sent stays in the Inbox with Try again,
 *    and trying again sends it once and completes it.
 * 7. A send that may have gone through stays in the Inbox with one answer,
 *    "I checked Sent"; a tap asked anyway answers with the record, and after
 *    the check it leaves and still sends nothing.
 * 8. A held batch and a changeset open on their one-line summary, the diff
 *    collapsed; expanded, a pick made in the diff survives the redraw and is
 *    exactly what Accept takes.
 */

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";

import { fold, PROPOSED_BASE, SIGNED_POST, suite, WRITE_BASE } from "./harness";
import cast from "../test/cast";

const { proposed } = cast.world;

const app = await suite("inbox");

/** The todo the fixture's run proposes adding, and the one it proposes closing. */
const ADDED = proposed.id;
const CLOSED = proposed.id;

/**
 * Open the Inbox and wait until it has drawn something — a card, or the empty
 * note.
 */
// THE PAGE DRAWN AGAIN FROM NOTHING, IN THE SAME TAB: away and back by the hash, never a reload, so whatever a card
// shows after it came from what the tab holds outside the drawing (a card's register), not from the old nodes.
async function drawnAgain(base: string, card: string): Promise<void> {
  await app.page.navigate(`${base}/#/secretary`);
  await app.page.waitFor(`document.querySelector(".inboxpage") === null`, { timeout: 8000 });
  await app.page.navigate(`${base}/#/inbox`);
  await app.page.waitFor(`document.querySelector(${JSON.stringify(card)}) !== null`, { timeout: 15000 });
}

const valueOf = (selector: string): Promise<string> =>
  app.page.evaluate<string>(`document.querySelector(${JSON.stringify(selector)})?.value ?? "(none)"`);

async function openInbox(base: string): Promise<void> {
  await app.page.navigate(`${base}/#/inbox`);
  await app.page.reload();
  await app.page.waitFor(
    `document.querySelector(".inboxpage .batch, .inboxpage .proposal, .inboxpage .empty") !== null && document.querySelector('.inboxpage .proposals[data-box="Answered"]') !== null`,
    { timeout: 15000 },
  );
  // The ticks and the buttons arrive with `GET /api/me`'s answer, which lands
  // AFTER the mount: a click before it would land on a page about to redraw.
  // A proposal card needs no such wait — its Send button is drawn with the
  // card itself, unlike a batch's ticks.
  await app.page.waitFor(
    `document.querySelector(".inboxpage .empty, .inboxpage .proposal") !== null || document.querySelector(".batchacts .act, .batchacts .readonly") !== null`,
    { timeout: 10000 },
  );
}

/** The waiting count the tab bar's own badge reports — `""` for none waiting,
 * never a number this page computed twice. */
const badge = (): Promise<string> =>
  app.page.evaluate<string | null>(`document.querySelector(".tabbar .tabbadge")?.textContent ?? null`).then(
    (found) => found ?? "",
  );

/**
 * A FRESH DATED CONVERSATION, minted over the route rather than the UI — the
 * same shape `secretary.test.ts`'s own `freshConversation` is, kept here so
 * this file's scenes do not depend on that one's DOM.
 */
async function freshConversation(base: string): Promise<string> {
  await app.page.navigate(`${base}/#/secretary/default`);
  await app.page.waitFor(`document.querySelector(".pagetitle") !== null`, { timeout: 15000 });
  return app.page.evaluate<string>(
    `${SIGNED_POST}("/api/secretary/conversations", JSON.stringify({title: "Untitled"})).then((r) => r.json()).then((j) => j.id)`,
  );
}

/** Open a fresh conversation and say something, waiting for the proposal at
 * `index` to land. */
async function propose(base: string, said: string, index: number): Promise<string> {
  const id = await freshConversation(base);
  await app.page.navigate(`${base}/#/secretary/${id}`);
  await app.page.waitFor(`document.querySelector(".composebox") !== null`, { timeout: 8000 });
  await app.page.type(".composebox", said);
  await app.page.click(".composesend");
  await app.page.waitFor(`document.querySelector('.proposal[data-proposal="${index}"]') !== null`, {
    timeout: 20000,
  });
  return id;
}

/** Say something more in a conversation already open, waiting for a NEW
 * proposal at `index` to land — the same "type, click, wait" `propose` uses,
 * without minting a second conversation. */
async function sayMore(said: string, index: number): Promise<void> {
  await app.page.type(".composebox", said);
  await app.page.click(".composesend");
  await app.page.waitFor(`document.querySelector('.proposal[data-proposal="${index}"]') !== null`, {
    timeout: 20000,
  });
}

/** The object hashes a batch card is showing, in the order it shows them. */
const shownObjects = (): Promise<string[]> =>
  app.page.evaluate<string[]>(
    `[...document.querySelectorAll(".proposedobject")].map((row) => row.getAttribute("data-object"))`,
  );

/** RUN THE AGENT'S WRITER AGAIN, from node. `scripts/web-e2e.sh` names the
 * binary and the fixture's instance file; there is nothing here to fake. */
function rerunTriage(...flags: string[]): { ran: boolean; refused: string } {
  const bin = process.env["E2E_TRIAGE_BIN"];
  const instance = process.env["E2E_TRIAGE_INSTANCE"];
  if (bin === undefined || instance === undefined) {
    throw new Error("E2E_TRIAGE_BIN/E2E_TRIAGE_INSTANCE are unset — scripts/web-e2e.sh sets them");
  }
  const said = execFileSync(bin, ["fixture-triage", "--instance", instance, ...flags], { encoding: "utf8" });
  return JSON.parse(said.trim()) as { ran: boolean; refused: string };
}

/** A route call over `fetch`, in the page — every route-level assertion below
 * asks the box directly rather than through a card, per this file's own note
 * on why scenes 3-5 do. */
async function post(base: string, path: string, body: unknown): Promise<{ status: number; body: unknown }> {
  await app.page.navigate(`${base}/#/`);
  const said = await app.page.evaluate<string>(`(async () => {
    const answer = await ${SIGNED_POST}(${JSON.stringify(path)}, ${JSON.stringify(JSON.stringify(body))});
    return JSON.stringify({ status: answer.status, body: await answer.json() });
  })()`);
  return JSON.parse(said) as { status: number; body: unknown };
}

app.scene("a held batch appears in the Inbox as an open decision, drawn as its diff", async () => {
  await openInbox(PROPOSED_BASE);

  // ITS SUMMARY FIRST, the rows folded: a viewer reads the overview and may open the diff.
  assert.equal(
    await app.page.evaluate<string>(`document.querySelector(".batch .diffline").textContent`),
    `add ${proposed.id} · complete ${CLOSED}`,
  );
  assert.equal(await app.page.evaluate<boolean>(`document.querySelector(".batch .batchobjects").hidden`), true);
  // ITS LINE IS THE COUNT AND THE DATE; the chain, the writer and the batch's name are in the diff.
  assert.deepEqual(
    await app.page.evaluate<string[]>(`[...document.querySelector(".batch .batchhead").children].map((c) => c.className)`),
    ["batchsize", "batchwhen"],
  );
  assert.equal(await app.page.evaluate<string>(`document.querySelector(".batch .batchsize").textContent`), "2 changes");
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll(".batch .batchobjects .batchmeta :is(.batchchain, .batchwho, .batchhash)").length`),
    3,
  );
  await app.page.click(".batch .diffbtn");
  await app.page.waitFor(`document.querySelector(".batch .batchobjects").hidden === false`, { timeout: 4000 });

  const objects = await shownObjects();
  assert.equal(objects.length, 2, "one run, two writes");

  const text = await app.page.evaluate<string>(`document.querySelector(".inboxpage").textContent`);
  assert.match(text, new RegExp(`add ${proposed.id}`), `the added todo is named as an addition: ${text}`);
  assert.match(text, new RegExp(`complete ${CLOSED}`), `the completion names its todo: ${text}`);
  assert.match(text, new RegExp(proposed.body), "the body is shown in full");
  assert.match(text, /the rain did it/, "and so is the note on the completion");

  // THE SECRETARY'S OWN REASON IS THE DECISION'S rationale — the record it
  // authored when it held the batch, drawn above the diff exactly as it drew
  // the OLD review line.
  const held = await app.page.evaluate<string | null>(
    `document.querySelector(".inboxpage .batch .batchheld")?.textContent ?? null`,
  );
  assert.notEqual(held, null, "a held batch carries the secretary's reason");
  assert.match(held ?? "", /^Held for you: /, `${held ?? ""}`);
  assert.match(held ?? "", /adds a todo and completes it/, `${held ?? ""}`);

  // AND THE CHAIN HAS NOT MOVED. Nothing here is on it.
  const entries = await fold(app.page);
  assert.equal(entries.find((entry) => entry.todo === ADDED), undefined, "the added todo is not on the chain");

  // The badge counts the one open decision.
  assert.equal(await badge(), "1", "one decision is waiting, and the bar says so");
});

app.scene("a viewer reads the Inbox and is offered nothing to press", async () => {
  await openInbox(PROPOSED_BASE);
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll(".batchacts .act").length`),
    0,
    "a login that cannot settle is not offered a button that settles",
  );
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll(".batchacts .readonly").length`),
    1,
    "the row is there and says why, so the page's shape does not change under a viewer",
  );
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll(".proposedpick").length`),
    0,
    "and there are no ticks either — a selection it cannot send is furniture",
  );

  // …and the ROUTE refuses it, whatever the page drew.
  const refused = await post(PROPOSED_BASE, "/api/life/proposals/accept", { objects: [] });
  assert.equal(refused.status, 403);
  assert.match((refused.body as { error: string }).error, /may read this app but not write to it/);
});

// EVERY SCENE FROM HERE SCOPES ITS QUERIES BY `[data-conversation="<id>"]`,
// NEVER BY A BARE COUNT OF `.inboxpage .proposal`. Each `propose` mints its
// own conversation, but WRITE_BASE is shared across this whole file's scenes
// in the order node runs them (`--test-concurrency=1`, one server) — so a
// scene that leaves its own decision standing (this file has one, on
// purpose: the next scene needs a fresh one to tap) must not be read by a
// LATER scene's raw total, only by the row that names its own conversation.

app.scene("a standing proposal appears in the Inbox as an open decision", async () => {
  const id = await propose(WRITE_BASE, "propose the harrow reply", 0);
  await openInbox(WRITE_BASE);
  const row = `.inboxpage .proposal[data-conversation="${id}"]`;
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll('${row}').length`),
    1,
    "the standing proposal is drawn as the EXISTING proposal card",
  );
  assert.match(
    await app.page.evaluate<string>(`document.querySelector('${row} .proposalbody').textContent`),
    /the last one is scanned/,
    "the same card the conversation draws — one implementation, not two",
  );
});

// THE BOX'S OWN CARDS ARE ONE ROW (audit X2): the fixture declares secrets it does not hold, so the box files a
// setup card for each, and the Inbox folds them into one closed row after the work, counted apart from it, its
// cards untagged since the row names them (audit S21).
app.scene("the box's setup cards are one closed row after the work, opening in place to its cards and Settings", async () => {
  await openInbox(WRITE_BASE);
  await app.page.waitFor(`document.querySelector(".inboxpage .setupgroup") !== null`, { timeout: 15000 });
  const placed = await app.page.evaluate<{ open: boolean; summary: string; after: boolean; outside: number; inside: number; tagged: number; legend: string }>(`(() => {
    const group = document.querySelector(".inboxpage .setupgroup");
    const top = [...document.querySelector(".inboxpage .proposals").children];
    const at = top.indexOf(group);
    return {
      open: group.open,
      summary: group.querySelector("summary").textContent,
      after: top.every((n, i) => !(n.matches(".proposal, .batch")) || i < at),
      outside: [...document.querySelectorAll(".inboxpage .proposal .proposaltag")].filter((t) => t.textContent === "Setup" && t.closest(".setupgroup") === null).length,
      inside: group.querySelectorAll(".proposal").length,
      tagged: group.querySelectorAll(".proposal .proposaltag").length,
      legend: [...document.querySelectorAll(".inboxpage p.legend")].map((p) => p.textContent).join(" "),
    };
  })()`);
  assert.equal(placed.open, false, "the row is drawn closed");
  assert.match(placed.summary, /^Setup · \d+ things? to set up$/);
  assert.equal(placed.after, true, "every card of the work stands above it");
  assert.equal(placed.outside, 0, "no setup card stands outside it");
  assert.ok(placed.inside > 0);
  // One tag per card: the row says Setup, so no card in it says it again (audit S21).
  assert.equal(placed.tagged, 0, "a card in the Setup row carries no tag of its own");
  // The count is what is shown: the setup steps are counted apart from the work.
  assert.match(placed.legend, new RegExp(`${placed.inside} setup steps?`), placed.legend);
  await app.page.click(".inboxpage .setupgroup > summary");
  await app.page.waitFor(`document.querySelector(".inboxpage .setupgroup").open === true`, { timeout: 4000 });
  assert.equal(await app.page.evaluate<string>(`document.querySelector(".inboxpage .setupsettings").getAttribute("href")`), "#/settings");
  // Each card in it is drawn closed too, its field labelled in words and its Store visibly waiting for one.
  assert.equal(await app.page.evaluate<number>(`[...document.querySelectorAll(".inboxpage .setupgroup .proposaldetails")].filter((d) => d.open).length`), 0);
  const field = await app.page.evaluate<{ label: string; placeholder: string; disabled: boolean } | null>(`(() => {
    const f = document.querySelector(".inboxpage .setupgroup .proposalsecretfield");
    return f === null ? null : { label: f.querySelector(".secretlabel span").textContent, placeholder: f.querySelector("input").placeholder, disabled: f.querySelector("button").disabled };
  })()`);
  if (field !== null) {
    assert.notEqual(field.label, "");
    assert.equal(field.placeholder, "Paste it here");
    assert.equal(field.disabled, true);
  }
  // No card says how the box speaks of itself.
  const words = await app.page.evaluate<string>(`document.querySelector(".inboxpage .proposals").innerText`);
  assert.ok(!/proposal:setup|no model asked|on this box/.test(words), words);
  // Closed again by the reader, it stays closed.
  await app.page.click(".inboxpage .setupgroup > summary");
  await app.page.waitFor(`document.querySelector(".inboxpage .setupgroup").open === false`, { timeout: 4000 });
});

// ASKING YES OPENS THE CARD (audit X2): a card is drawn closed, and Send's question is asked with every field on screen.
app.scene("a card is drawn closed, and asking Send opens it whole before the question", async () => {
  const id = await propose(WRITE_BASE, "propose the reply opened by its question", 0);
  await openInbox(WRITE_BASE);
  const card = `.inboxpage .proposal[data-conversation="${id}"]`;
  await app.page.waitFor(`document.querySelector('${card} .act-send') !== null`, { timeout: 10000 });
  assert.equal(await app.page.evaluate<boolean>(`document.querySelector('${card} .proposaldetails').open`), false);
  assert.deepEqual(
    await app.page.evaluate<string[]>(`[...document.querySelectorAll('${card} .proposalact-row button')].map((b) => b.textContent)`),
    ["Send", "Dismiss", "Reply"],
  );
  await app.page.click(`${card} .act-send`);
  await app.page.waitFor(`document.querySelector('${card} .proposalconfirm') !== null`, { timeout: 4000 });
  assert.equal(await app.page.evaluate<boolean>(`document.querySelector('${card} .proposaldetails').open`), true, "the message is on screen");
  await app.page.click(`${card} .act-cancelsend`);
  await app.page.waitFor(`document.querySelector('${card} .proposalconfirm') === null`, { timeout: 4000 });
  assert.equal(await app.page.evaluate<boolean>(`document.querySelector('${card} .proposaldetails').open`), true, "No leaves it as it was");
  const gone = await app.page.evaluate<number>(
    `${SIGNED_POST}("/api/secretary/conversations/${id}/proposals/0/dismiss", JSON.stringify({ reason: "e2e cleanup" })).then((r) => r.status)`,
  );
  assert.equal(gone, 200);
});

// A CHANGESET, NOT AN EMAIL: tapping Confirm applies real chain acts and never
// touches `$SUZATARY_E2E_SENT`, which `secretary.test.ts` asserts an EXACT
// count against — a second sender there would fail a test this file did not
// touch. A tap is a tap whichever act it performs; this one just leaves the
// shared fixture's one recorded-mail count alone.
app.scene("tapping a proposal in the Inbox confirms it and drops the count", async () => {
  const id = await propose(WRITE_BASE, "propose an inbox tap test changeset", 0);
  await openInbox(WRITE_BASE);
  const row = `.inboxpage .proposal[data-conversation="${id}"]`;
  await app.page.click(`${row} .act-confirm`);
  await app.page.waitFor(`document.querySelector(".inboxpage .proposalconfirm") !== null`, { timeout: 4000 });
  // THE CONFIRM STEP AND ITS NOTE OUTLIVE A REDRAW: they are the card's register, not its nodes.
  await app.page.type(`${row} .noteline`, "applied from");
  await drawnAgain(WRITE_BASE, row);
  assert.equal(await app.page.evaluate<boolean>(`document.querySelector('${row} .proposalconfirm') !== null`), true, "the confirm step survives");
  await app.page.type(`${row} .noteline`, " the Inbox");
  assert.equal(await valueOf(`${row} .noteline`), "applied from the Inbox", "and so do the words, typed on from where they were");
  // The count the page drew just now: the shell's startup badge counts standing proposals only, and may be what
  // showed before the page was drawn again.
  const before = await badge();
  await app.page.click(`${row} .act-confirm.confirming`);
  await app.page.waitFor(`document.querySelector('${row}') === null`, { timeout: 15000 });
  // The card goes with the tap's own register; the badge is drawn when the box's answer lands, a later draw, so
  // the scene waits for the count to move rather than reading it at one instant.
  await app.page
    .waitFor(`(document.querySelector(".tabbar .tabbadge")?.textContent ?? "") !== ${JSON.stringify(before)}`, { timeout: 15000 })
    .catch(() => undefined);
  const after = await badge();
  assert.notEqual(after, before, `tapping settled the decision the proposal made: ${before} -> ${after}`);
  const entries = await fold(app.page);
  assert.ok(entries.some((entry) => entry.todo === "e2einboxtaptest"), "the tap really applied the changeset");
});

app.scene("withdrawing a proposal leaves the Inbox with nothing waiting on it", async () => {
  const id = await propose(WRITE_BASE, "propose the reply to withdraw", 0);
  await sayMore("withdraw the harrow reply", 0); // same index: withdrawn IN PLACE, not superseded, and drawn as withdrawn, never as the owner's no
  await app.page.waitFor(
    `document.querySelector('.proposal[data-proposal="0"]')?.className.includes("proposal-withdrawn") ?? false`,
    { timeout: 20000 },
  );
  await openInbox(WRITE_BASE);
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll('.inboxpage .proposal[data-conversation="${id}"]').length`),
    0,
    "a withdrawn proposal's decision is cancelled, so the fold no longer counts it open",
  );
});

app.scene("a superseded proposal shows a quiet line, never a dismissal in Ada's words", async () => {
  const id = await propose(WRITE_BASE, "propose the draft to correct", 0);
  await sayMore("send a corrected harrow reply", 1);

  // THE OLD CARD, in the conversation: not `dismissed`, a quiet line of its
  // own — SPEC §5 commit 4(d), and the ONE place the Rust derives it
  // (`secretary::routes::proposal`) is asserted by `app/core`'s own tests;
  // this is the page matching the arm it sent.
  assert.equal(
    await app.page.evaluate<string>(`document.querySelector('.proposal[data-proposal="0"]').className`),
    "proposal proposal-superseded proposal-act-email",
  );
  assert.equal(
    await app.page.evaluate<string>(
      `document.querySelector('.proposal[data-proposal="0"] .proposaloutcome').textContent`,
    ),
    "Replaced by a later draft.",
    "never his words — a superseded proposal is nobody's answer to report",
  );

  // AND THE INBOX COUNTS ONLY THE NEW ONE, from THIS conversation: the
  // superseded decision is cancelled, so this conversation's fold row is
  // exactly one, drawn as index 1's card — the revision, not the draft it
  // replaced. Tapped afterwards so this scene, like the others, leaves
  // nothing of its own standing behind it.
  await openInbox(WRITE_BASE);
  const rows = `.inboxpage .proposal[data-conversation="${id}"]`;
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll('${rows}').length`),
    1,
    "one open decision: the revision, not the draft it replaced",
  );
  assert.match(
    await app.page.evaluate<string>(`document.querySelector('${rows}').getAttribute("data-proposal")`),
    /^1$/,
    "the standing card is the REVISION",
  );
  await app.page.click(`${rows} .act-dismiss`);
  await app.page.waitFor(`document.querySelector('${rows} .dismissreason') !== null`, { timeout: 4000 });
  // A HALF-TYPED REASON OUTLIVES A REDRAW: the step and the words are read back, and typing goes on from them.
  await app.page.type(`${rows} .dismissreason`, "e2e");
  await drawnAgain(WRITE_BASE, rows);
  assert.equal(await valueOf(`${rows} .dismissreason`), "e2e", "the reason being typed survives");
  await app.page.type(`${rows} .dismissreason`, " cleanup");
  assert.equal(await valueOf(`${rows} .dismissreason`), "e2e cleanup");
  await app.page.click(`${rows} .act-dismiss.confirming`);
  await app.page.waitFor(`document.querySelector('${rows}') === null`, { timeout: 15000 });
});

app.scene("a reason being typed stays in its own card while the card above it goes", async () => {
  const above = await propose(WRITE_BASE, "propose the reply that moves away", 0);
  const below = await propose(WRITE_BASE, "propose the reply being answered", 0);
  await openInbox(WRITE_BASE);
  const card = (id: string): string => `.inboxpage .proposal[data-conversation="${id}"]`;
  await app.page.waitFor(`document.querySelector('${card(above)}') !== null && document.querySelector('${card(below)}') !== null`, { timeout: 15000 });
  await app.page.click(`${card(below)} .act-dismiss`);
  await app.page.waitFor(`document.querySelector('${card(below)} .dismissreason') !== null`, { timeout: 4000 });
  await app.page.type(`${card(below)} .dismissreason`, "not yet, the");
  // The other card goes from under the one being typed in: the rows the Inbox draws move.
  const gone = await app.page.evaluate<number>(
    `${SIGNED_POST}("/api/secretary/conversations/${above}/proposals/0/dismiss", JSON.stringify({ reason: "e2e reorder" })).then((r) => r.status)`,
  );
  assert.equal(gone, 200);
  await app.page.waitFor(`document.querySelector('${card(above)}') === null`, { timeout: 15000 });
  await app.page.type(`${card(below)} .dismissreason`, " deadline");
  assert.equal(
    await app.page.evaluate<string>(`document.querySelector('${card(below)} .dismissreason').value`),
    "not yet, the deadline",
    "every character is in the field it was typed into",
  );
  assert.deepEqual(
    await app.page.evaluate<string[]>(`[...document.querySelectorAll('.inboxpage .dismissreason')].map((f) => f.value).filter((v) => v !== "")`),
    ["not yet, the deadline"],
    "and in no other",
  );
  await app.page.click(`${card(below)} .act-dismiss.confirming`);
  await app.page.waitFor(`document.querySelector('${card(below)}') === null`, { timeout: 15000 });
});

// THE RECORDER FAILS ON CUE: `--smtp-fixture <file>` reads `<file>.failures`
// before each send and fails the next one as the first line says (`not_sent`
// or `unknown`), recording nothing. The scenes below count their own subject
// in the recorder rather than its lines, because it is shared.
const SENT = process.env["SUZATARY_E2E_SENT"] ?? "";

const sentWith = (subject: string): number =>
  (existsSync(SENT) ? readFileSync(SENT, "utf8") : "")
    .split("\n")
    .filter(Boolean)
    .filter((line) => (JSON.parse(line) as { subject?: string }).subject === subject).length;

/** Where a conversation's first proposal stands, read as the tab reads it: the inbox's objects through the module. */
async function decisionOf(conversation: string): Promise<string | undefined> {
  const proposals = await app.page.evaluate<{ state: { kind: string }; asked: string }[]>(
    `(async () => {
      const core = await import("/b/dev/prodrome.js");
      await core.default({ module_or_path: "/b/dev/prodrome_bg.wasm" });
      const held = await (await fetch("/api/stores/inbox/objects")).json();
      const inbox = new core.Proposals(JSON.stringify(held.objects.map((o) => ({ hash: o.hash, text: o.literal }))));
      const chains = new core.Chains();
      try {
        return JSON.parse(inbox.inbox(new Date().toISOString().slice(0, 19), chains)).proposals;
      } finally {
        chains.free();
        inbox.free();
      }
    })()`,
  );
  return proposals.find((p) => p.asked === `proposal:${conversation}/0`)?.state.kind;
}

/** Tap an answer on this card and confirm it. */
async function answer(row: string, verb: string): Promise<void> {
  await app.page.click(`${row} .act-${verb}`);
  await app.page.waitFor(`document.querySelector('${row} .proposalconfirm') !== null`, { timeout: 4000 });
  await app.page.click(`${row} .act-${verb}.confirming`);
}

app.scene("a send that sent nothing offers Try again, and trying again sends it once", async () => {
  assert.notEqual(SENT, "", "the suite names a recorder file");
  const id = await propose(WRITE_BASE, "propose the failing reply", 0);
  writeFileSync(`${SENT}.failures`, "not_sent\n");
  await openInbox(WRITE_BASE);
  const row = `.inboxpage .proposal[data-conversation="${id}"]`;
  await answer(row, "send");
  await app.page.waitFor(`document.querySelector('${row}.proposal-failed') !== null`, { timeout: 15000 });

  // STILL WAITING, AND STILL ANSWERABLE: the card says what failed and offers
  // the same send again, and the decision behind it is open.
  assert.match(
    await app.page.evaluate<string>(`document.querySelector('${row} .proposaloutcome').textContent`),
    /^Not sent, tried once: /,
  );
  assert.equal(
    await app.page.evaluate<string>(`document.querySelector('${row} .act-send').textContent`),
    "Try again",
  );
  assert.equal(sentWith("The failing reply"), 0, "nothing was recorded");
  assert.equal(await decisionOf(id), "open");

  await answer(row, "send");
  await app.page.waitFor(`document.querySelector('${row}') === null`, { timeout: 15000 });
  assert.equal(sentWith("The failing reply"), 1, "the second tap sent it, once");
  assert.equal(await decisionOf(id), "completed", "sending it closed its decision");
});

app.scene("a send that may have gone stays until he checks Sent, and never sends again", async () => {
  const id = await propose(WRITE_BASE, "propose the uncertain reply", 0);
  writeFileSync(`${SENT}.failures`, "unknown\n");
  await openInbox(WRITE_BASE);
  const row = `.inboxpage .proposal[data-conversation="${id}"]`;
  await answer(row, "send");
  await app.page.waitFor(`document.querySelector('${row}.proposal-unknown .act-acknowledge') !== null`, {
    timeout: 15000,
  });

  // STILL IN FRONT OF HIM, with one answer: never Send again, never Dismiss.
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll('${row} .act-send, ${row} .act-dismiss').length`),
    0,
    "a send that may have gone is never offered again",
  );
  assert.match(
    await app.page.evaluate<string>(`document.querySelector('${row} .proposaloutcome').textContent`),
    /It may have been sent\. Check Sent; it is never sent again\./,
  );
  assert.equal(await decisionOf(id), "open", "it waits on his check");

  // A TAP ASKED ANYWAY ANSWERS WITH THE RECORD, AND THE RECORDER STAYS EMPTY.
  const tap = (): Promise<string> =>
    app.page.evaluate<string>(
      `${SIGNED_POST}("/api/secretary/conversations/${id}/proposals/0/tap", "{}").then(async (r) => r.status + " " + (await r.json()).proposals[0].state)`,
    );
  assert.equal(await tap(), "200 unknown");

  await app.page.click(`${row} .act-acknowledge`);
  await app.page.waitFor(`document.querySelector('${row}') === null`, { timeout: 15000 });
  assert.equal(await decisionOf(id), "completed", "checked, it leaves the Inbox");
  assert.equal(await tap(), "200 checked");
  assert.equal(sentWith("The uncertain reply"), 0);
});

/** Where the phone-width pictures of a folded and an opened diff go, when a run asks for them. */
const SHOTS = process.env["SUZATARY_DIFF_SHOTS"];
const shoot = async (selector: string, name: string): Promise<void> => {
  if (SHOTS === undefined) return;
  await app.page.evaluate(`document.querySelector('${selector}').scrollIntoView({ block: "center" })`);
  await app.page.screenshot(`${SHOTS}/${name}.png`);
};

app.scene("a batch and a changeset open on their summary, and a pick made in the diff is what Accept takes", async () => {
  const HELD = "e2esummarybatch";
  const ran = rerunTriage("--hold", HELD);
  assert.equal(ran.ran, true, `the held run: ${ran.refused}`);
  const id = await propose(WRITE_BASE, "propose a summary changeset", 0);
  await openInbox(WRITE_BASE);

  const head = await app.page.evaluate<string>(
    `[...document.querySelectorAll(".inboxpage .batch")].find((b) => b.textContent.includes("${HELD}"))?.getAttribute("data-batch") ?? ""`,
  );
  assert.notEqual(head, "", "the held run is drawn as a batch");
  const batch = `.inboxpage .batch[data-batch="${head}"]`;
  const changeset = `.inboxpage .proposal[data-conversation="${id}"]`;
  await app.page.waitFor(`document.querySelector('${batch} .batchacts .act') !== null`, { timeout: 10000 });
  await shoot(batch, "batch-folded");
  await shoot(changeset, "changeset-folded");
  const text = (selector: string): Promise<string> =>
    app.page.evaluate<string>(`document.querySelector('${selector}')?.textContent ?? ""`);
  const hidden = (selector: string): Promise<boolean> =>
    app.page.evaluate<boolean>(`document.querySelector('${selector}').hidden`);

  // THE BATCH, FOLDED: the acts, the hold's reason in numbers, the picks counted, and its acts still offered.
  assert.equal(await text(`${batch} .diffline`), `add ${HELD} · complete ${HELD}`);
  assert.equal(await text(`${batch} .diffflag`), `At or under the 45% the review holds: ${HELD} at 40%`);
  assert.equal(await text(`${batch} .diffpicked`), "2 of 2 picked");
  assert.equal(await app.page.evaluate<string>(`document.querySelector('${batch} .diffbtn').getAttribute("aria-expanded")`), "false");
  assert.equal(await hidden(`${batch} .batchobjects`), true, "the rows and their ticks are folded");
  assert.equal(await text(`${batch} .act-accept`), "Accept all");

  // THE CHANGESET, FOLDED: grouped by act, the low price said, and Confirm still offered.
  assert.equal(await text(`${changeset} .diffline`), "add 2 todos · complete e2esummaryone");
  assert.equal(await text(`${changeset} .diffflag`), "At or under the 45% the review holds: e2esummaryone at 40%");
  assert.equal(await app.page.evaluate<boolean>(`document.querySelector('${changeset} .proposaldetails').open`), false, "a card is drawn closed");
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll('${changeset} .act-confirm').length`), 1);
  await app.page.click(`${changeset} .proposaldetails > summary`);
  await app.page.waitFor(`document.querySelector('${changeset} .proposaldetails').open === true`, { timeout: 4000 });
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll('${changeset} .proposedobject').length`), 3);
  await shoot(changeset, "changeset-open");

  // OPENED, a pick is made in the diff; the redraw it causes keeps the diff open.
  await app.page.click(`${batch} .diffbtn`);
  await app.page.waitFor(`document.querySelector('${batch} .batchobjects').hidden === false`, { timeout: 4000 });
  const completion = await app.page.evaluate<string>(
    `document.querySelector('${batch} .proposedobject.act-completed').getAttribute("data-object")`,
  );
  const addition = await app.page.evaluate<string>(
    `document.querySelector('${batch} .proposedobject.act-added').getAttribute("data-object")`,
  );
  const clicked = await app.page.evaluate<number>("performance.now()");
  await app.page.click(`${batch} .proposedpick[data-pick="${completion}"]`);
  await app.page.waitFor(`document.querySelector('${batch} .diffpicked')?.textContent === "1 of 2 picked"`, { timeout: 4000 });
  // Drawing the page again runs none of its handlers: one pick is one toggle and a draw or two, and it stays.
  await new Promise((r) => setTimeout(r, 1500));
  const draws = await app.page.evaluate<number>(`performance.getEntriesByName("page:draw").filter((e) => e.startTime >= ${clicked}).length`);
  assert.ok(draws <= 3, `a pick drew the page ${draws} times`);
  assert.equal(await text(`${batch} .diffpicked`), "1 of 2 picked", "the pick stays made");
  assert.equal(await hidden(`${batch} .batchobjects`), false, "a pick does not fold the diff it was made in");
  assert.equal(await text(`${batch} .act-accept`), "Accept 1");
  await shoot(batch, "batch-open");

  // ACCEPT TAKES EXACTLY THE PICK: the todo lands, its completion is still standing.
  await app.page.click(`${batch} .act-accept`);
  await app.page.waitFor(`document.querySelector('.inboxpage .batch .diffline')?.textContent === "complete ${HELD}"`, {
    timeout: 15000,
  });
  const entries = await fold(app.page);
  assert.equal(entries.find((entry) => entry.todo === HELD)?.state, "open", "the addition is on the chain, the completion is not");
  const standing = await app.page.evaluate<{ objects: { hash: string }[] }[]>(
    `fetch("/api/life/proposals").then((r) => r.json()).then((j) => j.batches)`,
  );
  const left = standing.flatMap((b) => b.objects.map((o) => o.hash));
  assert.ok(left.includes(completion), "the completion still stands");
  assert.ok(!left.includes(addition), "the addition it rests on was taken");

  // Nothing of this scene is left standing for a later file.
  const rejected = await post(WRITE_BASE, "/api/life/proposals/reject", { objects: [completion] });
  assert.equal(rejected.status, 200, JSON.stringify(rejected.body));
});
