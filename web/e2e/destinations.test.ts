/**
 * THE INFORMATION ARCHITECTURE, in a real browser (2026-09-10).
 *
 * `web/test/nav.test.ts` pins the arithmetic — five slots, which four, what
 * lights up — as pure data under node. What it cannot check is whether any of
 * that reaches a page: a bar built from the right list and never rendered, a
 * destination in `DESTINATIONS` with no arm in `main.ts`'s switch, a chain
 * pinned by a route that then mounts twice. Those are the failures this file is
 * for, and they are exactly the ones a reading of the diff would pass.
 *
 * The claims, each one a way the naive implementation goes wrong:
 *
 *   1. The bar IS Todos · Inbox · Secretary · Briefing · More, on a phone
 *      (docs/REDESIGN-PLAN-2026-09-25.md: words, not icons).
 *   2. Every one of those five opens something, and a hash with no arm lands
 *      on "No such page" quietly.
 *   3. `#/project` shows the OTHER chain — not the same list under a new title,
 *      which is what a switcher that changed a label and nothing else would do.
 *      The fixture's second chain is EMPTY, so this is checkable.
 *   4. Coming back to `#/` is the first chain again. A pin that only ever moved
 *      one way would pass claim 3 and strand the reader on Project.
 *   5. At 62rem the same words sit across the top, and More still holds the rest.
 *   6. The theme override, picked in the More sheet, wins over the machine's answer, in both directions.
 *   7. `#/add?text=…` is the list with the add sheet up and the text in it —
 *      the far end of the Android share target, whose Java half has no test
 *      harness at all. What the browser can prove is everything after the
 *      fragment is handed over, which is all of the behaviour.
 */

import assert from "node:assert/strict";

import { BASE, DESKTOP, PHONE, shownTodos, suite } from "./harness";

const app = await suite("destinations");

/** The bar's labels, in order. The More button is a `<button>` and the rest are
 * links, so this reads the text rather than the tag. */
const barLabels = async (): Promise<string[]> =>
  app.page.evaluate<string[]>(
    `[...document.querySelectorAll(".tabbar > .tab")].map((tab) => tab.querySelector(".tablabel")?.textContent ?? "")`,
  );

app.scene("the bar is the five a phone opens the app for", async () => {
  await app.open(BASE);
  assert.deepEqual(await barLabels(), ["Todos", "Inbox", "Secretary", "Briefing", "More"]);
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll(".tabbar svg").length`),
    0,
    "words, not icons",
  );
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll(".tabbar .tab.active .diamond").length`),
    1,
    "the page in view is marked with a diamond",
  );
});

app.scene("every slot in the bar opens a page the router knows", async () => {
  await app.open(BASE);
  for (const [label, hash] of [
    ["Todos", "#/"],
    ["Inbox", "#/inbox"],
    ["Secretary", "#/secretary/default"],
    ["Briefing", "#/briefings"],
  ] as const) {
    await app.page.navigate(`${BASE}/${hash}`);
    await app.page.waitFor(`document.querySelector("#view .pagetitle, #view h1") !== null`);
    const title = await app.page.evaluate<string>(
      `document.querySelector("#view .pagetitle, #view h1").textContent`,
    );
    assert.notEqual(title, "No such page", `${label} at ${hash} is not routed`);
    const lit = await app.page.evaluate<string>(
      `document.querySelector('.tabbar .tab.active .tablabel')?.textContent ?? ""`,
    );
    assert.equal(lit, label, `${hash} should light ${label}`);
  }
});

app.scene("Project is the OTHER chain, not the same list under a new name", async () => {
  await app.open(BASE);
  const life = await shownTodos(app.page);
  assert.ok(life.length > 0, "the first chain has todos");

  await app.page.navigate(`${BASE}/#/project`);
  await app.page.waitFor(`document.querySelector("#view .pagetitle")?.textContent === "Project"`);
  // ⚠️ WAIT FOR THE EMPTY NOTE, NOT FOR ZERO CARDS. "No cards" is also true of
  // a list that has not drawn yet, so a scene waiting on the count would pass
  // before the fold arrived and prove nothing about which chain it is. The note
  // is drawn only by a list that HAS drawn and found nothing.
  await app.page.waitFor(`document.querySelector(".todospane .empty:not([hidden])") !== null`);
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll(".tree .row").length`), 0);

  // AND IT SAYS THE RIGHT ONE OF THE THREE. The fixture's second chain is not
  // empty — it holds one RESOLVED todo, deliberately (a replica cannot seal the
  // first object of an empty chain), so the honest sentence here is "nothing is
  // OPEN", not "nothing here yet" and certainly not "widen your filter", which
  // is what a single empty string would have said for all three.
  const said = await app.page.evaluate<string>(`document.querySelector(".todospane .empty:not([hidden])").textContent`);
  assert.match(said, /Nothing is open/, `the empty note should name the reason this list is empty: ${said}`);
  assert.equal(said.includes("filter"), false, "nobody narrowed a filter; do not tell them to widen one");
});

/** Open the More sheet on whatever hash the reader is standing on. */
const openMore = async (): Promise<void> => {
  await app.page.evaluate(
    `[...document.querySelectorAll(".tabbar .tab")].find((tab) => tab.textContent?.includes("More"))?.click()`,
  );
  await app.page.waitFor(`document.querySelector(".moresheet")?.hidden === false`);
};

/**
 * THE SWITCHER AGREES WITH THE PAGE, OR IT IS NOT THERE (chain item
 * `chainswitcherlies`, `docs/AUDIT-UX-2026-09-12.md` finding 5). Three
 * scenes: hidden on the destination that is already about one chain, present
 * and honest everywhere else, and honest again once the reader has LEFT that
 * destination — the case a switch that only ever moved one way would fail.
 */
app.scene("on Project the More sheet has no chain switcher", async () => {
  await app.page.navigate(`${BASE}/#/project`);
  await app.page.waitFor(`document.querySelector("#view .pagetitle")?.textContent === "Project"`);
  await openMore();
  assert.equal(
    await app.page.evaluate<boolean>(`document.querySelector(".moresheet .chainrow") !== null`),
    false,
    "a control that disagrees with the page is worse than no control",
  );
  await app.page.evaluate(`document.body.click()`);
  await app.page.waitFor(`document.querySelector(".moresheet")?.hidden === true`);
});

app.scene("leaving Project, Todos is the first chain again: its list, its lit word and the switcher", async () => {
  await app.page.navigate(`${BASE}/#/project`);
  await app.page.waitFor(`document.querySelector("#view .pagetitle")?.textContent === "Project"`);
  await app.page.navigate(`${BASE}/#/`);
  await app.page.waitFor(`document.querySelector("#view .pagetitle")?.textContent === "Todos"`);
  await app.page.waitFor(`document.querySelectorAll(".tree .row").length > 0`);
  assert.equal(
    await app.page.evaluate<string>(`document.querySelector('.tabbar .tab.active .tablabel')?.textContent ?? ""`),
    "Todos",
  );
  await openMore();
  const active = await app.page.evaluate<string>(
    `document.querySelector(".moresheet .chainpick.active")?.textContent ?? ""`,
  );
  assert.equal(active, "My todos", "back on Todos the switcher should read the first chain, not the one it left on");
  await app.page.evaluate(`document.body.click()`);
  await app.page.waitFor(`document.querySelector(".moresheet")?.hidden === true`);
});

/**
 * THE MORE SHEET IN THE OWNER'S WORDS (the 2026-10-13 audit, X13 and X15): the chains are "My todos" and "This app's
 * work", never their names; Settings, About, Chain and Usage stand apart under "This app", away from Mail and Calendar;
 * and the theme is picked there, at the foot, where people look for it, and no longer on About.
 */
app.scene("More names the chains as the owner does, sets this app's pages apart, and holds the theme", async () => {
  await app.page.navigate(`${BASE}/#/`);
  await app.page.waitFor(`document.querySelector("#view .pagetitle")?.textContent === "Todos"`);
  await openMore();
  assert.deepEqual(
    await app.page.evaluate<string[]>(`[...document.querySelectorAll(".moresheet .chainpick")].map((n) => n.textContent)`),
    ["My todos", "This app's work"],
  );
  assert.deepEqual(
    await app.page.evaluate<string[]>(`[...document.querySelectorAll(".moresheet > .morelink .tablabel")].map((n) => n.textContent)`),
    ["Project", "Mail", "Calendar"],
  );
  assert.deepEqual(
    await app.page.evaluate<string[]>(`[...document.querySelectorAll(".moresheet .moreapp .morelink .tablabel")].map((n) => n.textContent)`),
    ["Settings", "About", "Chain", "Usage"],
  );
  await app.page.clickText(".moresheet .themepick", "Dark");
  await app.page.waitFor(`document.documentElement.getAttribute("data-theme") === "dark"`);
  await app.page.waitFor(`document.querySelector(".moresheet .themepick.active")?.textContent === "Dark"`);
  assert.equal(await app.page.evaluate<boolean>(`document.querySelector(".moresheet").hidden`), false, "picking a theme keeps the sheet open");
  await app.page.clickText(".moresheet .themepick", "System");
  await app.page.waitFor(`document.documentElement.hasAttribute("data-theme") === false`);
  await app.page.evaluate(`document.body.click()`);
  await app.page.waitFor(`document.querySelector(".moresheet")?.hidden === true`);
  await app.page.navigate(`${BASE}/#/about`);
  await app.page.waitFor(`document.querySelector(".aboutversion") !== null`);
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll("#view .themerow").length`), 0, "About keeps its prose and no setting");
});

/** A deployment that declares nothing and has proposed nothing: the shell's server, whose Inbox
 * holds no card of the box's either, since it declares no secret. */
const BARE = process.env["E2E_BASE_SHELL"] ?? BASE;

app.scene("the Inbox is a queue, and it says so when there is nothing in it", async () => {
  await app.page.navigate(`${BARE}/#/inbox`);
  // The EMPTY NOTE, not the legend: when the queue is empty the legend gets out
  // of the note's way (one sentence, not two), so waiting on the legend would
  // be waiting on a line the page has deliberately taken down.
  await app.page.waitFor(`document.querySelector(".inboxpage .empty") !== null`);
  const said = await app.page.evaluate<string>(`document.querySelector(".inboxpage .empty").textContent`);
  // That fixture has made no proposals, so this is the empty case — and the
  // empty case is the one that must not read as an error.
  assert.match(said, /Nothing is waiting on you/, said);
  assert.equal(
    await app.page.evaluate<boolean>(`document.querySelector(".inboxpage .legend").hidden`),
    true,
    "the page should not say there is nothing twice",
  );
});

app.scene("at 62rem the same words sit across the top, and More holds the rest", async () => {
  await app.resize(DESKTOP);
  await app.open(BASE);
  assert.deepEqual(await barLabels(), ["Todos", "Inbox", "Secretary", "Briefing", "More"]);
  const bar = await app.page.evaluate<{ top: number; position: string }>(
    `(() => { const bar = document.querySelector("header .tabbar"); return { top: bar.getBoundingClientRect().top, position: getComputedStyle(bar).position }; })()`,
  );
  assert.ok(bar.top < 80, `the bar is at the top of a wide screen, not its foot (top ${bar.top})`);
  assert.notEqual(bar.position, "fixed");
  await openMore();
  const rest = await app.page.evaluate<string[]>(
    `[...document.querySelectorAll(".moresheet .morelink .tablabel")].map((n) => n.textContent)`,
  );
  assert.deepEqual(rest, ["Project", "Mail", "Calendar", "Settings", "About", "Chain", "Usage"]);
  await app.page.evaluate(`document.body.click()`);
  await app.page.waitFor(`document.querySelector(".moresheet")?.hidden === true`);
  await app.resize(PHONE);
});

/**
 * WHAT ANDROID'S SHARE SHEET OPENS (2026-09-10).
 *
 * `MainActivity.targetOf` maps an `ACTION_SEND` intent onto this exact URL and
 * hands it to the WebView whole; from there it is the web app's, and this is
 * the whole of what happens. Four claims, and the naive implementation misses
 * the last two: the sheet is UP (not merely the list), the body is the two
 * shared fields composed, the bar lights Todos rather than nothing, and the
 * hash has gone back to the list's — because a `#/add` left in the URL bar
 * re-opens over an already-added todo on reload, and makes sharing the same
 * link twice in a row do nothing the second time (no `hashchange`).
 */
app.scene("a shared link opens the list with the sheet prefilled, once", async () => {
  // A BARE `#/add` IS THE LIST, with nothing to fill in and the sheet shut —
  // and it is the one of these that can be reached with a real navigation. The
  // share hashes below cannot: consuming a share rewrites `location.href`, and
  // `navigate` waits for the href it asked for. Driving them as hash changes is
  // also the shape of the real thing, because the app is normally already open
  // when Android hands it a share and the intent lands on `onNewIntent`.
  // `#/` first, so this scene starts on the ordinary list whatever the scenes
  // above left selected — `#/add` names no chain, exactly like `#/todo/<id>`.
  await app.open(BASE);
  await app.page.navigate(`${BASE}/#/add`);
  assert.equal(
    await app.page.evaluate<string>(`document.querySelector("#view .pagetitle").textContent`),
    "Todos",
    "#/add is routed — an unrouted hash lands on No such page quietly",
  );
  assert.equal(await app.page.evaluate<boolean>(`document.querySelector(".addsheet.open") !== null`), false);

  const shared = "#/add?text=https%3A%2F%2Fexample.test%2Ffpl%3Fv%3D2&title=The%20FPL%20paper";
  const share = async (): Promise<void> => {
    await app.page.evaluate(`window.location.hash = ${JSON.stringify(shared)}`);
    await app.page.waitFor(`document.querySelector(".addsheet.open") !== null`, { timeout: 8000 });
  };

  await share();
  assert.equal(
    await app.page.evaluate<string>(`document.querySelector(".draftbody").value`),
    "The FPL paper\nhttps://example.test/fpl?v=2",
    "the two shared fields, composed by draft.sharedBody and nothing else",
  );
  assert.equal(
    await app.page.evaluate<string>(`document.querySelector(".tabbar .tab.active .tablabel")?.textContent ?? ""`),
    "Todos",
    "a share lights the list it lands on, not nothing",
  );
  assert.equal(
    await app.page.evaluate<string>(`window.location.hash`),
    "#/",
    "the share is consumed: the hash goes back to the list's",
  );

  // AND THE SAME LINK AGAIN, which is what the consumption is for. Without it
  // the second share would be a hash that did not change, no `hashchange`, and
  // a sheet that never opened.
  await app.page.evaluate(`document.querySelector(".scrim").click()`);
  await app.page.waitFor(`document.querySelector(".addsheet.open") === null`);
  await share();
  assert.equal(
    await app.page.evaluate<string>(`document.querySelector(".draftbody").value`),
    "The FPL paper\nhttps://example.test/fpl?v=2",
  );

  await app.page.evaluate(`document.querySelector(".scrim").click()`);
});

/** Open the More sheet and press one of the three theme buttons, the way a
 * reader does. Through the sheet rather than by writing `localStorage`, because
 * what is under test is that the control is REACHABLE — a preference nobody can
 * get to is not a preference. */
// The theme control is on the About page since 2026-09-13 (audit finding 6):
// it was pinned at the bottom of the sheet and the rail, permanent space for a
// setting touched about twice in a lifetime.
// The theme is picked in the More sheet, which closes again with a press elsewhere.
const pickTheme = async (label: string): Promise<void> => {
  await openMore();
  await app.page.clickText(".moresheet .themepick", label);
  await app.page.waitFor(`document.querySelector(".moresheet .themepick.active")?.textContent === ${JSON.stringify(label)}`);
  await app.page.evaluate(`document.body.click()`);
  await app.page.waitFor(`document.querySelector(".moresheet")?.hidden === true`);
};

app.scene("the theme override wins over the machine, in both directions", async () => {
  await app.page.colorScheme("dark");
  await app.open(BASE);
  const ground = async (): Promise<string> =>
    app.page.evaluate<string>(`getComputedStyle(document.body).backgroundColor`);
  const onDarkMachine = await ground();

  // Light, chosen, on a dark machine.
  await pickTheme("Light");
  await app.page.waitFor(`document.documentElement.getAttribute("data-theme") === "light"`);
  const forcedLight = await ground();
  assert.notEqual(forcedLight, onDarkMachine, "an override that changes nothing is not an override");

  // And it SURVIVES A RELOAD, because it is remembered — the whole point of a
  // preference over a media query.
  await app.page.reload();
  await app.page.waitFor(`document.documentElement.getAttribute("data-theme") === "light"`);
  assert.equal(await ground(), forcedLight);

  // Back to System: the machine decides again, which is the state a two-state
  // toggle cannot express — it would have had to guess an initial value and
  // then it has silently stopped following the machine.
  await pickTheme("System");
  await app.page.waitFor(`document.documentElement.getAttribute("data-theme") === null`);
  assert.equal(await ground(), onDarkMachine);

  await app.page.colorScheme(null);
});
