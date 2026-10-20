/**
 * A SETTING CHANGED BY ITS CARD, END TO END (the guided setup's stage 5, scene 7 of its §12).
 *
 * `scripts/web-e2e.sh` starts this file's server with `--settings --connected calendar --configuring order`:
 * a card stands in the `models` conversation, as the setup agent will file it, changing Review's
 * model order from nothing chosen to `anthropic/claude-e2e`.
 *
 * ONE SCENE. Settings draws the card on the Review line it changes, saying from and to, and the
 * page has no editor of its own; tapping Change writes the order into the instance file, and the
 * Review line then says the new model, with no card left on it.
 */

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import { suite, trustThisDevice } from "./harness";

const app = await suite("configure");

const CONNECT = process.env["E2E_BASE_CONNECT"] ?? "";
const INSTANCE = process.env["E2E_CONNECT_INSTANCE"] ?? "";

const review = '.settingsline[data-line="job:review"]';
const card = `${review} .proposal.proposal-act-configure[data-conversation="models"]`;

const text = (selector: string): Promise<string> => app.page.evaluate<string>(`document.querySelector('${selector}')?.textContent ?? ""`);

app.scene("a Configure card shows from and to on its line, and its tap moves the Models line", async () => {
  await trustThisDevice(app.page, CONNECT, INSTANCE);
  await app.page.navigate(`${CONNECT}/#/settings`);
  await app.page.reload();
  await app.page.waitFor(`document.querySelector('${card} .act-change') !== null`, { timeout: 20000 });

  assert.match(await text(`${review} .settingsmeta`), /^Review answers with the Claude subscription's \S+ model, since nothing is chosen\.$/);
  const said = await text(card);
  assert.match(said, /From\s*nothing chosen, so the fallback answers/, "the card says what the setting reads");
  assert.match(said, /To\s*anthropic\/claude-e2e/, "and what the tap writes");
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll('.settingsgroup[data-group="models"] input, .settingsgroup[data-group="models"] select').length`),
    0,
    "the Models group has no editor of its own",
  );
  assert.equal(readFileSync(INSTANCE, "utf8").includes("claude-e2e"), false, "nothing is written before the tap");

  await app.page.click(`${card} .act-change`);
  await app.page.click(`${card} .act-change.confirming`);
  await app.page.waitFor(`document.querySelector('${card}') === null`, { timeout: 20000 });
  assert.match(readFileSync(INSTANCE, "utf8"), /review = \["anthropic\/claude-e2e"\]/, "the tap wrote the order");

  await app.page.reload();
  await app.page.waitFor(`(document.querySelector('${review} .settingsmeta')?.textContent ?? "").includes("claude-e2e")`, { timeout: 20000 });
  assert.equal(await text(`${review} .settingsmeta`), "Review answers with the Anthropic API's claude-e2e model.");
});
