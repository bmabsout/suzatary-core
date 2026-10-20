/**
 * A CODING SESSION'S CARD (`docs/CODINGSESSION-PLAN-2026-09-27.md` §2 rules
 * 3 to 5), over the conversation `app/cli/src/commands/fixture.rs`'s
 * `a_coding_session` seeds on the WRITE server: a tapped coding proposal whose
 * session was fired and then reported. Nothing is fired here; the fixture's
 * routine is unreachable by construction.
 *
 * ONE SCENE, in order, because each answer changes what the next can see: the
 * card shows the branch, the diff stat's total and the report whole; "Hand to
 * the main session" writes its todo on the project chain and the card then
 * links it; dropping needs a reason and ends the session. The Settings page
 * lists the routine's token because this deployment declares `[coding]`.
 */

import assert from "node:assert/strict";

import { WRITE_BASE, suite } from "./harness";

const app = await suite("coding");

const SESSION = ".proposal .codingsession";

// The secretary has answered the hand-over: a model turn after the act that records it, and nothing running.
const HEARD = `(() => {
  const turns = [...document.querySelectorAll(".turn")];
  const act = turns.findIndex((t) => t.dataset.who === "act" && /handed it:/.test(t.textContent));
  return act >= 0 && turns.slice(act + 1).some((t) => t.dataset.who === "model") && document.querySelector(".answerplace")?.hidden === true;
})()`;

// What a failure writes down of the card: its register and intent as it draws them, and its DOM whole.
const PROBE = `(() => { const n = document.querySelector('${SESSION}'); return n === null ? null : { ui: n.dataset.ui, asked: n.dataset.asked, hash: location.hash, focus: document.activeElement?.className ?? null, place: document.querySelector(".answerplace")?.hidden, turns: [...document.querySelectorAll(".turn")].map((t) => t.dataset.who + ": " + t.textContent.slice(0, 60)), html: new XMLSerializer().serializeToString(n) }; })()`;

app.scene("a reported session shows its report, is handed over once, then dropped", async () => {
  const { mark } = await app.journal();
  mark("open");
  await app.page.navigate(`${WRITE_BASE}/#/secretary/codingsession`);
  await app.page.reload();
  await app.page.waitFor(`document.querySelector('${SESSION}.codingsession-reported') !== null`, {
    timeout: 15000,
  });

  const text = (selector: string): Promise<string> =>
    app.page.evaluate<string>(`document.querySelector('${selector}')?.textContent ?? ""`);
  assert.equal(await text(`${SESSION} .codinghead`), "Reported on claude/retitle");
  assert.equal(await text(`${SESSION} .codingtotal`), "2 files, +6 −1");
  assert.match(await text(`${SESSION} .codingreport`), /Gates: cargo test and web-test green\./);
  assert.match(await text(".proposal .proposaloutcome"), /started a coding session/);

  mark("hand");
  await app.page.click(`${SESSION} .act-hand`);
  await app.page.waitFor(`document.querySelector('${SESSION} .codinghanded .reflink') !== null`, {
    timeout: 10000,
  });
  assert.match(await text(`${SESSION} .codinghanded`), /todo:suzatary\/merge[0-9a-f]{12}/);
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll('${SESSION} .act-hand').length`),
    0,
    "a handed session offers no second hand-over",
  );
  // THE HAND-OVER WAKES THE SECRETARY (2026-10-04: under load its round landed while Drop was being tapped, and the
  // stick-to-bottom scroll moved the card from under the tap): the next tap waits until it has answered.
  mark("heard");
  await app.page.waitFor(HEARD, { timeout: 20000 });

  mark("drop");
  await app.page.click(`${SESSION} .act-drop`);
  mark("drop with no reason");
  await app.page.click(`${SESSION} .act-drop.confirming`);
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll('${SESSION}.codingsession-dropped').length`),
    0,
    "no reason, no drop",
  );
  // THE DROP STEP AND ITS HALF-TYPED REASON OUTLIVE A REDRAW (2026-10-03: the hand-over's settle redrew the card
  // between "Drop" and its confirmation, and the step was gone): the page drawn again from nothing finds both.
  mark("type done");
  await app.page.type(`${SESSION} .dismissreason`, "done");
  mark("leave");
  await app.page.navigate(`${WRITE_BASE}/#/secretary`);
  await app.page.waitFor(`document.querySelector('${SESSION}') === null`, { timeout: 8000 });
  mark("back");
  await app.page.navigate(`${WRITE_BASE}/#/secretary/codingsession`);
  await app.page.waitFor(`document.querySelector('${SESSION} .act-drop.confirming') !== null`, { timeout: 15000 });
  assert.equal(await app.page.evaluate<string>(`document.querySelector('${SESSION} .dismissreason').value`), "done");
  await app.page.type(`${SESSION} .dismissreason`, " by hand");
  assert.equal(await app.page.evaluate<string>(`document.querySelector('${SESSION} .dismissreason').value`), "done by hand");
  mark("drop done by hand");
  await app.page.click(`${SESSION} .act-drop.confirming`);
  await app.page.waitFor(`document.querySelector('${SESSION}.codingsession-dropped') !== null`, {
    timeout: 10000,
  });
  assert.equal(await text(`${SESSION} .codinghead`), "Dropped: done by hand");
}, PROBE);

app.scene("Settings lists the routine once [coding] declares one, in words", async () => {
  await app.page.navigate(`${WRITE_BASE}/#/settings`);
  await app.page.reload();
  const routine = '.settingsgroup[data-group="coding"] .settingsline[data-line="routine"]';
  await app.page.waitFor(`document.querySelector('${routine}') !== null`, { timeout: 15000 });
  // The line's own words; a card drawn on it keeps the stored name in its details fold.
  const said = await app.page.evaluate<string>(
    `[...document.querySelector('${routine}').children].filter((n) => !n.classList.contains("proposal")).map((n) => n.textContent).join(" ")`,
  );
  assert.match(said, /Coding sessions.*fires the routine/, said);
  assert.equal(said.includes("SUZATARY_ROUTINE_TOKEN"), false, "the token's name is not drawn");
});
