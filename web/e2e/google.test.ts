/**
 * SIGN IN WITH GOOGLE, END TO END (the guided setup's stage 10, `authcode`).
 *
 * `scripts/web-e2e.sh` starts this file's server with `--settings --connected google` and
 * `--google-fixture` aimed at `suzatary-dev fake-google`: a Google account's Connect card stands
 * in the `connect` conversation as the setup agent files it, and every request the box makes to
 * Google reaches the fake, which checks the redirect, the S256 challenge and that a code is
 * redeemed once.
 *
 * ONE SCENE, from the card to a connected account. Connect writes the account; the box files
 * the client's card, which takes the downloaded client file in its write-only field; the box then
 * files the account's sign-in card, whose tap opens a consent and whose button goes to Google
 * through the box; the fake consents and sends the browser back to the box's callback, which
 * keeps the grant and lands on the Inbox. The CDP log shows the client's secret only in the tap
 * that carried it, and the grant in no answer.
 */

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";

import { suite, trustThisDevice } from "./harness";

const app = await suite("google");

const GOOGLE = process.env["E2E_BASE_GOOGLE"] ?? "";
const INSTANCE = process.env["E2E_GOOGLE_INSTANCE"] ?? "";
const STORE = process.env["E2E_GOOGLE_STORE"] ?? "";
const FAKE = process.env["E2E_FAKE_GOOGLE"] ?? "";

const ACCOUNT = "ada@example.test";
const SECRET = "GOCSPX-fake-client-secret";
const REFRESH = "1//fake-refresh-1";

const client = (redirect: string): string =>
  JSON.stringify({
    web: {
      client_id: "1234-fake.apps.googleusercontent.com",
      project_id: "suzatary-fake",
      client_secret: SECRET,
      redirect_uris: [redirect],
    },
  });

const stored = (): string => (existsSync(STORE) ? readFileSync(STORE, "utf8") : "");
const instance = (): string => readFileSync(INSTANCE, "utf8");

/** The selector of the first card of `kind` in `conversation`. */
const cardOf = (kind: string, conversation: string): string =>
  `.inboxpage .proposal.proposal-act-${kind}[data-conversation="${conversation}"]`;

app.scene("a Google account is signed in from its cards, and the box keeps its grant", async () => {
  await trustThisDevice(app.page, GOOGLE, INSTANCE);
  await app.page.navigate(`${GOOGLE}/#/inbox`);

  const connect = cardOf("connect", "connect");
  await app.page.waitFor(`document.querySelector('${connect} .act-connect') !== null`, { timeout: 20000 });
  await app.page.click(`${connect} .act-connect`);
  await app.page.click(`${connect} .act-connect.confirming`);
  // Asked of the box, since an answered card folds into the day's settled row as it lands.
  await app.page.waitFor(
    `fetch("/api/secretary/conversations/connect/drawn?indices=0").then((r) => r.json()).then((d) => d.proposals[0]?.state === "acted")`,
    { timeout: 20000 },
  );
  assert.match(instance(), /sync = "google"/, "the tap wrote the account");

  // The box's own cards are one Setup row in the Inbox, opened in place.
  const clientCard = cardOf("google_client", "setup");
  await app.page.waitFor(`document.querySelector('${clientCard}') !== null`, { timeout: 20000 });
  await app.page.click(".inboxpage .setupgroup > summary");
  await app.page.waitFor(`document.querySelector(".inboxpage .setupgroup").open === true`);
  const said = await app.page.evaluate<string>(`document.querySelector('${clientCard}').textContent`);
  const redirect = `${GOOGLE}/api/oauth/google/callback`;
  assert.ok(said.includes(redirect), `the card names the exact redirect: ${said}`);
  assert.match(said, /In production/, "the card says to publish the app");
  assert.match(said, /7 days/, "and what Testing costs");

  const stopCapture = await app.page.captureNetwork(["/tap", "/api/"]);
  await app.page.type(`${clientCard} .proposalsecretfield input`, client(redirect));
  await app.page.click(`${clientCard} .act-store`);
  const signIn = cardOf("google_sign_in", "setup");
  await app.page.waitFor(`document.querySelector('${signIn} .act-sign_in') !== null`, { timeout: 20000 });
  const traffic = await stopCapture();
  const carrying = traffic.filter((exchange) => exchange.requestBody.includes(SECRET));
  assert.equal(carrying.length, 1, `one request carried the client: ${JSON.stringify(traffic.map((e) => e.url))}`);
  assert.match(carrying[0]?.url ?? "", /\/tap$/, "the one that carried it is the card's tap");
  for (const exchange of traffic) assert.equal(exchange.responseBody.includes(SECRET), false, `an answer carried it: ${exchange.url}`);
  assert.ok(stored().includes(SECRET), "the client's secret is kept");

  await app.page.click(`${signIn} .act-sign_in`);
  await app.page.click(`${signIn} .act-sign_in.confirming`);
  const button = `${signIn} a.proposallink`;
  await app.page.waitFor(`document.querySelector('${button}') !== null`, { timeout: 20000 });
  const href = await app.page.evaluate<string>(`document.querySelector('${button}').href`);
  const index = await app.page.evaluate<string>(`document.querySelector('${signIn}').getAttribute('data-proposal')`);
  assert.ok(href.startsWith(`${GOOGLE}/api/oauth/google/consent/`), `the button is the box's own path: ${href}`);

  // The fake consents at once and sends the browser back to the box's callback, which keeps the
  // grant and lands on the Inbox.
  const back = await app.page.captureNetwork(["/api/oauth/google/", "/api/"]);
  await app.page.evaluate(`(() => { location.href = ${JSON.stringify(href)}; return true; })()`);
  await app.page.waitFor(`location.origin === ${JSON.stringify(GOOGLE)} && location.hash === "#/inbox" && document.readyState === "complete"`, {
    timeout: 20000,
  });
  const followed = await back();
  for (const exchange of followed) assert.equal(exchange.responseBody.includes(REFRESH), false, `an answer carried the grant: ${exchange.url}`);
  assert.ok(stored().includes(`GOOGLE_GRANT_ADA_AT_EXAMPLE_TEST=mail,calendar ${REFRESH}`), "the grant is kept under the account's name, with the parts Google granted");
  assert.ok(FAKE !== "" && !href.startsWith(FAKE), "the page never held Google's own link");

  // The card is answered, and says how it ended.
  const drawn = await app.page.evaluate<string>(
    `fetch("/api/secretary/conversations/setup/drawn?indices=${index}").then((r) => r.text())`,
  );
  const row = (JSON.parse(drawn) as { proposals: { state: string; outcome: string }[] }).proposals[0];
  assert.equal(row?.state, "acted", drawn);
  assert.ok(row?.outcome.includes(`signed ${ACCOUNT} in with Google`), row?.outcome);
});
