/**
 * The router's parser.
 *
 * The reason this exists as a function at all is the home route: under the old
 * `hash.startsWith(prefix)` test, `#/` matched every page, so the list could
 * not be the app's front door. The cases below pin that, the aliases a
 * bookmark depends on, and totality — every string is a route, including one
 * with a percent-escape that `decodeURIComponent` throws on.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { hrefOf, parseRoute } from "../src/shell/route";
import type { Route } from "../src/shell/route";
import cast from "./cast";

const { counsel, owner } = cast.world;

test("the empty hash, #/ and #/todos are all home", () => {
  for (const hash of ["", "#", "#/", "#/todos"]) {
    assert.deepEqual(parseRoute(hash), { type: "todos" });
  }
});

test("#/ does not swallow the other pages", () => {
  assert.deepEqual(parseRoute("#/about"), { type: "about" });
  assert.deepEqual(parseRoute("#/chain"), { type: "chain", focus: "" });
});

test("a todo id is decoded", () => {
  assert.deepEqual(parseRoute("#/todo/permitrenewal"), { type: "todo", id: "permitrenewal" });
  assert.deepEqual(parseRoute(`#/todo/${encodeURIComponent("a b/c")}`), { type: "todo", id: "a b/c" });
});

test("a todo route with no id is not a todo", () => {
  assert.deepEqual(parseRoute("#/todo/"), { type: "unknown", hash: "#/todo/" });
  assert.deepEqual(parseRoute("#/todo"), { type: "unknown", hash: "#/todo" });
});

test("a malformed escape is a route, not an exception", () => {
  assert.deepEqual(parseRoute("#/todo/%"), { type: "unknown", hash: "#/todo/%" });
  assert.deepEqual(parseRoute("#/chain/%E0%A4%A"), { type: "chain", focus: "" });
});

test("#/chain/<hash> focuses an object", () => {
  assert.deepEqual(parseRoute("#/chain/cc324f2c"), { type: "chain", focus: "cc324f2c" });
});

test("an unrouted hash names itself", () => {
  assert.deepEqual(parseRoute("#/nowhere"), { type: "unknown", hash: "#/nowhere" });
});

test("#/mail is the inbox, and its query lives in the hash", () => {
  assert.deepEqual(parseRoute("#/mail"), { type: "mail", query: "", account: "" });
  assert.deepEqual(parseRoute(`#/mail?q=${encodeURIComponent(`from:${counsel.email}`)}`), {
    type: "mail",
    query: `from:${counsel.email}`,
    account: "",
  });
  assert.deepEqual(parseRoute(`#/mail?account=${encodeURIComponent(owner.email)}`), {
    type: "mail",
    query: "",
    account: owner.email,
  });
  assert.deepEqual(parseRoute(`#/mail?q=deed&account=${encodeURIComponent(owner.email)}`), {
    type: "mail",
    query: "deed",
    account: owner.email,
  });
});

test("a mail id and a thread id are different pages, and `thread/` is reserved", () => {
  // A message id reaches the hash percent-encoded, so its slashes are %2F and
  // it can never be mistaken for the thread prefix.
  assert.deepEqual(parseRoute("#/mail/m1%40example.com"), { type: "message", id: "m1@example.com" });
  assert.deepEqual(parseRoute("#/mail/thread/000000000000c3fb"), { type: "thread", id: "000000000000c3fb" });
  assert.deepEqual(parseRoute(`#/mail/${encodeURIComponent("thread/x")}`), { type: "message", id: "thread/x" });
  assert.deepEqual(parseRoute("#/mail/thread/"), { type: "unknown", hash: "#/mail/thread/" });
});

test("a malformed mail hash is a route, not an exception", () => {
  assert.deepEqual(parseRoute("#/mail/%"), { type: "unknown", hash: "#/mail/%" });
  assert.deepEqual(parseRoute("#/mail?q=%E0%A4%A"), { type: "mail", query: "", account: "" });
});

test("hrefOf round-trips every page through parseRoute", () => {
  const routes: Route[] = [
    { type: "todos" },
    { type: "todo", id: "a b/c" },
    { type: "mail", query: "", account: "" },
    { type: "mail", query: `from:${counsel.email}`, account: "" },
    { type: "mail", query: "subject:deed & more", account: owner.email },
    { type: "message", id: "m1@example.com" },
    { type: "message", id: "aaaaaaaaaaaa" },
    { type: "thread", id: "000000000000c3fb" },
    { type: "chain", focus: "" },
    { type: "chain", focus: "cc324f2c" },
    { type: "settings" },
    { type: "memory" },
    { type: "briefings" },
    { type: "briefing", date: "2026-09-01" },
    { type: "calendar", date: "" },
    { type: "calendar", date: "2026-10-05" },
    { type: "secretary", refs: [] },
    { type: "conversation", id: "2026-09-09-1", refs: [] },
  ];
  for (const route of routes) {
    assert.deepEqual(parseRoute(hrefOf(route)), route);
  }
});

test("calendar: today's week, or the week from a day", () => {
  assert.deepEqual(parseRoute("#/calendar"), { type: "calendar", date: "" });
  assert.deepEqual(parseRoute("#/calendar/2026-10-05"), { type: "calendar", date: "2026-10-05" });
  assert.equal(hrefOf({ type: "calendar", date: "" }), "#/calendar");
  assert.equal(hrefOf({ type: "calendar", date: "2026-10-05" }), "#/calendar/2026-10-05");
  assert.deepEqual(parseRoute("#/calendar/%E0"), { type: "unknown", hash: "#/calendar/%E0" });
});

test("briefings: the list and one day, and a bare prefix names the list", () => {
  assert.deepEqual(parseRoute("#/briefings"), { type: "briefings" });
  assert.deepEqual(parseRoute("#/briefings/2026-09-01"), { type: "briefing", date: "2026-09-01" });
  assert.equal(hrefOf({ type: "briefings" }), "#/briefings");
  assert.equal(hrefOf({ type: "briefing", date: "2026-09-01" }), "#/briefings/2026-09-01");
});

test("a trailing slash with no date still lists, like #/mail with none", () => {
  assert.deepEqual(parseRoute("#/briefings/"), { type: "briefings" });
});

test("the secretary: the list and one conversation", () => {
  assert.deepEqual(parseRoute("#/secretary"), { type: "secretary", refs: [] });
  assert.deepEqual(parseRoute("#/secretary/"), { type: "secretary", refs: [] });
  assert.deepEqual(parseRoute("#/secretary/2026-09-09-1"), {
    type: "conversation",
    id: "2026-09-09-1",
    refs: [],
  });
  assert.equal(
    hrefOf({ type: "conversation", id: "2026-09-09-1", refs: [] }),
    "#/secretary/2026-09-09-1",
  );
});

/**
 * "ASK THE SECRETARY" IS A LINK, so what is being asked about is IN THE HASH
 * (2026-09-10). That is what makes it a URL somebody can send, what survives a
 * reload, and what lets Back return to the todo the reader came from.
 *
 * The text forms ride as a comma-separated list because a text form holds no
 * comma. They are PARSED by `refs.ts`, never here, so this module stays free of
 * the grammar and a hand-edited hash is somebody else's refusal.
 */
test("a reference rides the hash into the secretary, and round-trips", () => {
  assert.deepEqual(parseRoute("#/secretary?ref=todo%3Alife%2Fpermitfiling"), {
    type: "secretary",
    refs: ["todo:life/permitfiling"],
  });
  assert.deepEqual(
    parseRoute("#/secretary/2026-09-09-1?ref=mail%3Aa1b2c3%2Cbriefing%3A2026-09-10"),
    { type: "conversation", id: "2026-09-09-1", refs: ["mail:a1b2c3", "briefing:2026-09-10"] },
  );
  // An empty `ref` is no chips rather than one empty chip.
  assert.deepEqual(parseRoute("#/secretary?ref="), { type: "secretary", refs: [] });
  for (const route of [
    { type: "secretary", refs: ["todo:life/x"] } as const,
    { type: "conversation", id: "2026-09-09-1", refs: ["mail:a1b2c3", "thread:00ab"] } as const,
  ]) {
    assert.deepEqual(parseRoute(hrefOf(route)), route);
  }
});

/**
 * `focus` IS THE OTHER DOOR (project chain `askfromanywhere`, docs/APP.md §4
 * item 8): "Ask the secretary" with no reference attached, which is a
 * compose-box focus intent riding the same query mechanism `ref` does.
 */
test("focus rides the hash into the secretary, absent by default, and round-trips", () => {
  assert.deepEqual(parseRoute("#/secretary?focus=1"), { type: "secretary", refs: [], focus: true });
  assert.deepEqual(
    parseRoute("#/secretary/2026-09-09-1?focus=1"),
    { type: "conversation", id: "2026-09-09-1", refs: [], focus: true },
  );
  // A bare `#/secretary` carries no `focus` key at all — absent, not `false`
  // — so it still `deepEqual`s every literal written before this existed.
  assert.deepEqual(parseRoute("#/secretary"), { type: "secretary", refs: [] });
  assert.equal(hrefOf({ type: "secretary", refs: [], focus: true }), "#/secretary?focus=1");
  assert.equal(hrefOf({ type: "secretary", refs: [] }), "#/secretary");
  // A reference and the focus intent can ride together.
  assert.deepEqual(parseRoute("#/secretary?ref=todo%3Alife%2Fx&focus=1"), {
    type: "secretary",
    refs: ["todo:life/x"],
    focus: true,
  });
  for (const route of [
    { type: "secretary", refs: [], focus: true } as const,
    { type: "conversation", id: "2026-09-09-1", refs: ["mail:a1b2c3"], focus: true } as const,
  ]) {
    assert.deepEqual(parseRoute(hrefOf(route)), route);
  }
});

/**
 * THE SHARE TARGET'S HASH (2026-09-10). Android's share sheet hands the app two
 * strings and `MainActivity` spells them into this fragment; it is the only
 * protocol between the two halves, so both directions are pinned here.
 *
 * The URL a browser shares is the interesting case: it holds `?`, `&`, `#`,
 * `=` and `/`, every one of which is structure in a hash that already has a
 * query in it. A round trip through `hrefOf` is the proof the escaping holds.
 */
test("a shared text and title ride the hash, and round-trip", () => {
  assert.deepEqual(parseRoute("#/add"), { type: "add", text: "", title: "" });
  assert.deepEqual(parseRoute("#/add?text=Renew%20the%20lease"), {
    type: "add",
    text: "Renew the lease",
    title: "",
  });
  assert.deepEqual(
    parseRoute("#/add?text=https%3A%2F%2Fexample.test%2Fa%3Fb%3D1%26c%3D2%23top&title=A%20title"),
    { type: "add", text: "https://example.test/a?b=1&c=2#top", title: "A title" },
  );
  // A title with no text is still a share — a note app that names a heading.
  assert.deepEqual(parseRoute("#/add?title=Just%20a%20name"), {
    type: "add",
    text: "",
    title: "Just a name",
  });
  // Malformed, like every other route here: absent rather than an exception.
  assert.deepEqual(parseRoute("#/add?text=%E0%A4%A"), { type: "add", text: "", title: "" });
  for (const route of [
    { type: "add", text: "", title: "" } as const,
    { type: "add", text: "https://example.test/a?b=1&c=2#top", title: "A title" } as const,
    { type: "add", text: "a + b = c, 100%", title: "" } as const,
  ]) {
    assert.deepEqual(parseRoute(hrefOf(route)), route);
  }
});

test("settings is one destination, and its hash round-trips", () => {
  assert.deepEqual(parseRoute("#/settings"), { type: "settings" });
  assert.equal(hrefOf({ type: "settings" }), "#/settings");
  assert.deepEqual(parseRoute(hrefOf({ type: "settings" })), { type: "settings" });
  // Like every other argumentless page, the head decides and a trailing
  // segment is ignored rather than being a second rule for one route.
  assert.deepEqual(parseRoute("#/settings/anything"), { type: "unknown", hash: "#/settings/anything" });
});

test("the inbox is one destination, and its hash round-trips", () => {
  assert.deepEqual(parseRoute("#/inbox"), { type: "inbox" });
  assert.equal(hrefOf({ type: "inbox" }), "#/inbox");
  assert.deepEqual(parseRoute(hrefOf({ type: "inbox" })), { type: "inbox" });
  // Which batch is open is page-local state, so a trailing segment is ignored
  // rather than being a second rule for one route.
  assert.deepEqual(parseRoute("#/inbox/anything"), { type: "inbox" });
});

test("#/typst is the Typst list, and #/typst/<id> the same list with a todo open", () => {
  assert.deepEqual(parseRoute("#/typst"), { type: "typst", open: "" });
  assert.deepEqual(parseRoute("#/typst/file-permit"), { type: "typst", open: "file-permit" });
  assert.equal(hrefOf({ type: "typst", open: "file-permit" }), "#/typst/file-permit");
  assert.deepEqual(parseRoute("#/typst/%"), { type: "unknown", hash: "#/typst/%" });
});
