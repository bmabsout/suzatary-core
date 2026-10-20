/**
 * THE MODEL BOOTSTRAP, END TO END (the guided setup's stage 0): connecting the first model is
 * the one step no model guides, so the box asks for it itself.
 *
 * `scripts/web-e2e.sh` starts this file's server with `--model unanswered`: the register says
 * its model's last call failed, so the box files the "Connect a model" card at start, and its
 * `PATH` holds a fake `claude` that draws what `claude setup-token` draws. No real sign-in is
 * ever reached.
 *
 * THE PLAN'S SCENE 9 FIRST, while no model answers: the server's calendar has synced and its
 * service has since refused the link its own store holds (`fixture-chain --calendar revoked`), and
 * its one provider is ordered by no job, so the box files their fix cards itself: the calendar's
 * line says what failed and draws the failing link's one card, and the provider's line draws its
 * removal, a Configure card; both are the box's.
 *
 * THEN: Settings says no model answers and draws the card first under Models; Connect
 * starts the sign-in, whose link (the box's own reading of the CLI) is the code card's button;
 * the code typed into its field keeps the subscription's token, and the CDP log shows the code
 * in the tap's request alone and the token in no request or response.
 *
 * LAST, the calendar still failing, the first screen says setup is unfinished and opens Settings,
 * Models first.
 */

import assert from "node:assert/strict";

import { suite, trustThisDevice } from "./harness";

const app = await suite("bootstrap");

const BOOTSTRAP = process.env["E2E_BASE_BOOTSTRAP"] ?? "";
const INSTANCE = process.env["E2E_BOOTSTRAP_INSTANCE"];

/** What the fake `claude` accepts, and the token it prints for it: never a real one. */
const CODE = "e2e-code#e2e-state";
const TOKEN = "sk-ant-oat01-E2Efake-token-never-real";

const connect = '.settingsbody .proposal.proposal-act-claude_sign_in[data-conversation="setup"]';
const code = '.settingsbody .proposal.proposal-act-sign_in_code[data-conversation="setup"]';

const calendar = '.settingsline[data-line="credential:ics/work"]';
const provider = '.settingsline[data-line="provider:phala"]';

app.scene("with no model answering, the box still files each failing connection's fix card", async () => {
  await app.page.navigate(`${BOOTSTRAP}/#/settings`);
  await app.page.waitFor(`document.querySelector('${connect}') !== null`, { timeout: 15000 });
  assert.match(
    await app.page.evaluate<string>(`document.querySelector(".settingsunanswered").textContent`),
    /No model answers/,
  );
  const failing = await app.page.evaluate<string>(`document.querySelector('${calendar} .settingshealth').textContent`);
  assert.match(failing, /^Failing since .*401.* Its link was changed or revoked, so store the new one\.$/, "a stored link its service refused says so, and what to do");
  assert.equal(failing.match(/401/g)?.length, 1, `the cause is said once: ${failing}`);
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll('${calendar} .proposal.proposal-act-secret').length`),
    1,
    "the failing link is one card",
  );
  assert.match(
    await app.page.evaluate<string>(`document.querySelector('${calendar} .proposal.proposal-act-secret .proposalwhy').textContent`),
    /^Your Work calendar has failed since \d{4}-\d{2}-\d{2} \d{2}:\d{2} UTC: /,
    "the card is the failing rule's, not a missing secret's",
  );
  // The box's own: its Secret card in the setup conversation is a setup card, its others the app's.
  for (const [line, kind, tag] of [
    [calendar, "secret", "Setup"],
    [provider, "configure", "App"],
  ]) {
    const card = `${line} .proposal.proposal-act-${kind}[data-conversation="setup"]`;
    await app.page.waitFor(`document.querySelector('${card}') !== null`, { timeout: 15000 });
    assert.equal(
      await app.page.evaluate<string>(`document.querySelector('${card} .proposaltag').textContent`),
      tag,
      `${kind}: the box filed it, with no model asked`,
    );
  }
});

app.scene("with no model answering, Settings connects one by signing in", async () => {
  await trustThisDevice(app.page, BOOTSTRAP, INSTANCE);
  await app.page.navigate(`${BOOTSTRAP}/#/settings`);
  // The card draws before the page knows this device may act, and its button once it does.
  await app.page.waitFor(`document.querySelector('${connect} .act-connect') !== null`, { timeout: 15000 });
  assert.match(
    await app.page.evaluate<string>(`document.querySelector(".settingsunanswered").textContent`),
    /No model answers/,
  );
  assert.match(
    await app.page.evaluate<string>(`document.querySelector('${connect} .proposaltag').textContent`),
    /^App$/,
    "the box filed it, with no model asked",
  );

  await app.page.click(`${connect} .act-connect`);
  await app.page.click(`${connect} .act-connect.confirming`);
  await app.page.waitFor(`document.querySelector('${code} .proposallink') !== null`, { timeout: 30000 });
  assert.match(
    await app.page.evaluate<string>(`document.querySelector('${code} .proposallink').getAttribute("href")`),
    /^https:\/\/claude\.com\/cai\/oauth\/authorize\?/,
    "the button opens the link the CLI drew",
  );

  const stopCapture = await app.page.captureNetwork(["/tap", "/api/"]);
  await app.page.type(`${code} .proposalsecretfield input`, CODE);
  await app.page.click(`${code} .act-finish`);
  await app.page.waitFor(`document.querySelector(".settingsunanswered") === null`, { timeout: 30000 });

  const traffic = await stopCapture();
  const carrying = traffic.filter((exchange) => exchange.requestBody.includes(CODE));
  assert.equal(carrying.length, 1, `one request carried the code: ${JSON.stringify(traffic.map((e) => e.url))}`);
  assert.match(carrying[0]?.url ?? "", /\/tap$/, "the one that carried it is the code card's tap");
  for (const exchange of traffic) {
    assert.equal(exchange.requestBody.includes(TOKEN), false, `a request carried the token: ${exchange.url}`);
    assert.equal(exchange.responseBody.includes(TOKEN), false, `a response carried the token: ${exchange.url}`);
    assert.equal(exchange.responseBody.includes(CODE), false, `a response carried the code: ${exchange.url}`);
  }
  const pageText = await app.page.evaluate<string>(`document.body.textContent`);
  assert.equal(pageText.includes(CODE) || pageText.includes(TOKEN), false, "neither reached the page");
});

// THE FIRST SCREEN SAYS SETUP IS UNFINISHED (audit S11): the calendar's link still fails, so Todos leads with one line
// opening Settings, which draws Models first.
app.scene("while setup is unfinished, the first screen says so and opens Settings, Models first", async () => {
  await app.page.navigate(`${BOOTSTRAP}/#/`);
  await app.page.waitFor(`document.querySelector(".setupunfinished a") !== null`, { timeout: 20000 });
  const line = await app.page.evaluate<{ text: string; href: string | null }>(`(() => {
    const a = document.querySelector(".setupunfinished a");
    return { text: a.textContent, href: a.getAttribute("href") };
  })()`);
  assert.match(line.text, /^Finish setting up · \d+ of \d+ done$/);
  assert.equal(line.href, "#/settings");
  await app.page.click(".setupunfinished a");
  await app.page.waitFor(`document.querySelector(".settingsbody .settingsgroup") !== null`, { timeout: 20000 });
  assert.equal(
    await app.page.evaluate<string>(`document.querySelector(".settingsbody .settingsgroup").getAttribute("data-group")`),
    "models",
    "the step every other waits on is first",
  );
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll(".settingspage .releases").length`), 0, "the release plan is About's");
});
