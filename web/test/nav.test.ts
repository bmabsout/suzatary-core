/**
 * The bar's arithmetic, and what lights up.
 *
 * A thumb has room for five words, so the bar keeps four of its own plus a
 * "More" sheet (docs/REDESIGN-PLAN-2026-09-25.md: Todos, Inbox, Secretary,
 * Briefing). That split is DATA here rather than a layout somebody eyeballed.
 *
 * `nav.ts` touches the DOM only inside its render functions, so importing it
 * under node is safe — these tests call neither.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import nav from "../src/shell/nav";
const { DESTINATIONS, OVERFLOW, TABS, activeFor, inOverflow } = nav;
import app from "../src/app/app";
import time from "../src/app/time";
import { DEFAULT_CHAIN, PROJECT_CHAIN } from "../src/app/chain";
import { parseRoute } from "../src/shell/route";

test("five slots, eleven destinations, and the sheet holds the difference", () => {
  assert.equal(TABS.length + 1, 5);
  assert.equal(TABS.length + OVERFLOW.length, DESTINATIONS.length);
  assert.deepEqual(
    TABS.map((tab) => tab.label),
    ["Todos", "Inbox", "Secretary", "Briefing"],
  );
  assert.deepEqual(
    OVERFLOW.map((tab) => tab.label),
    ["Project", "Mail", "Calendar", "Settings", "About", "Chain", "Usage"],
  );
});

test("the sheet sets this app's own pages apart from the owner's places", () => {
  assert.deepEqual(nav.PLACES.map((tab) => tab.label), ["Project", "Mail", "Calendar"]);
  assert.deepEqual(nav.APP.map((tab) => tab.label), ["Settings", "About", "Chain", "Usage"]);
  const more = nav.of(app.initial({ type: "todos" }, false, DEFAULT_CHAIN), "auto").more;
  assert.deepEqual(more.places.map((l) => l.label), ["Mail", "Calendar"], "Project only where its chain is served");
  assert.deepEqual(more.app.map((l) => l.label), ["Settings", "About", "Chain", "Usage"]);
});

test("the theme is offered in the sheet, the one this device shows marked", () => {
  for (const theme of ["auto", "light", "dark"] as const) {
    assert.equal(nav.of(app.initial({ type: "about" }, false, DEFAULT_CHAIN), theme).more.theme, theme);
  }
});

test("only the second list depends on what the deployment holds", () => {
  const conditional = DESTINATIONS.filter((tab) => tab.needsChain !== undefined);
  assert.deepEqual(conditional.map((tab) => tab.label), ["Project"]);
  assert.equal(conditional[0]?.needsChain, PROJECT_CHAIN);
});

test("a tab answers where you are in the app, not what the hash is", () => {
  assert.equal(activeFor(parseRoute("#/todo/harrowreply")), "todos");
  assert.equal(activeFor(parseRoute("#/inbox")), "inbox");
  assert.equal(activeFor(parseRoute("#/project")), "project");
  assert.equal(activeFor(parseRoute("#/mail")), "mail");
  assert.equal(activeFor(parseRoute("#/mail?q=deed")), "mail");
  assert.equal(activeFor(parseRoute("#/calendar/2026-10-05")), "calendar");
  assert.equal(activeFor(parseRoute("#/mail/m1%40example.com")), "mail");
  assert.equal(activeFor(parseRoute("#/mail/thread/000000000000c3fb")), "mail");
  // A conversation is reached FROM the secretary's list, so it lights that tab
  // — the same rule a message and a briefing already follow.
  assert.equal(activeFor(parseRoute("#/secretary/2026-09-09-1")), "secretary");
  // A SHARE IS NOT A SIXTH DESTINATION. `#/add` is the list with the sheet up,
  // so it lights the list — a bar with nothing lit would tell the reader they
  // had left the app's map on the way in from Android's share sheet.
  assert.equal(activeFor(parseRoute("#/add?text=x")), "todos");
  assert.equal(activeFor(parseRoute("#/add?text=x"), PROJECT_CHAIN), "project");
});

test("a todo lights the LIST it came from, and the two lists are two chains", () => {
  // The hash cannot say which — a chain is a persisted selection, not part of
  // the URL (`chain.ts`) — so the selection is the second argument.
  const todo = parseRoute("#/todo/ci-red");
  assert.equal(activeFor(todo, PROJECT_CHAIN), "project");
  assert.equal(activeFor(todo, DEFAULT_CHAIN), "todos");
  // And an unknown chain, or none at all, is the ordinary list.
  assert.equal(activeFor(todo, ""), "todos");
  assert.equal(activeFor(todo, "somewhere-else"), "todos");
});

test("the More button lights up for the pages behind it", () => {
  for (const hash of ["#/project", "#/mail", "#/chain/cc324f2c", "#/about", "#/usage", "#/settings"]) {
    assert.equal(inOverflow(parseRoute(hash)), true, hash);
  }
  // A todo opened from the second list lights Project, which is behind More.
  assert.equal(inOverflow(parseRoute("#/todo/ci-red"), PROJECT_CHAIN), true);
  // The four with words of their own do not, nor what they lead to.
  for (const hash of ["#/", "#/inbox", "#/secretary", "#/secretary/2026-09-09-1", "#/briefings", "#/briefings/2026-09-01"]) {
    assert.equal(inOverflow(parseRoute(hash)), false, hash);
  }
  assert.equal(inOverflow(parseRoute("#/todo/ci-red"), DEFAULT_CHAIN), false);
});

test("a day's briefing lights Briefing", () => {
  assert.equal(activeFor(parseRoute("#/briefings")), "briefings");
  assert.equal(activeFor(parseRoute("#/briefings/2026-09-01")), "briefings");
});

test("every destination is reachable from the bar or the sheet", () => {
  const reachable = new Set([...TABS, ...OVERFLOW].map((tab) => tab.route.type));
  for (const destination of DESTINATIONS) {
    assert.equal(reachable.has(destination.route.type), true, `${destination.label} is in neither`);
  }
});

test("the inbox badge counts what is waiting, and zero draws nothing", () => {
  const at = app.initial({ type: "todos" }, false, DEFAULT_CHAIN);
  const inbox = (count: number) => nav.of(app.step(at, { type: "Waiting", count }), "auto").tabs.find((l) => l.label === "Inbox")?.count;
  assert.equal(inbox(0), 0);
  assert.equal(inbox(3), 3);
  // A negative or fractional count is somebody's arithmetic, not a number to
  // draw: it floors at zero, which is the value that draws no badge at all.
  assert.equal(inbox(-2), 0);
  assert.equal(inbox(2.7), 2);
});

test("the switcher is offered where more than one chain is served and the page is no one chain's", () => {
  const served = app.step(app.initial({ type: "todos" }, false, DEFAULT_CHAIN), { type: "Met", me: { type: "Reads", login: "reader@example.invalid" }, served: [DEFAULT_CHAIN, PROJECT_CHAIN], categories: [], zone: time.UTC });
  assert.deepEqual(
    nav.of(served, "auto").more.picks,
    [
      { name: DEFAULT_CHAIN, label: "My todos", here: true, to: { type: "todos" } },
      { name: PROJECT_CHAIN, label: "This app's work", here: false, to: { type: "project" } },
    ],
    "on a list a pick is the other list",
  );
  const inbox = app.step(served, { type: "Went", route: { type: "inbox" } });
  assert.deepEqual(nav.of(inbox, "auto").more.picks.map((p) => p.to), [null, null], "elsewhere a pick only pins");
  const project = app.step(served, { type: "Went", route: { type: "project" } });
  assert.deepEqual(nav.of(project, "auto").more.picks, [], "the project list is one chain's");
  assert.deepEqual(nav.of(app.initial({ type: "todos" }, false, DEFAULT_CHAIN), "auto").more.picks, [], "one chain, no switcher");
});
