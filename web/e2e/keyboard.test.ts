/**
 * The keyboard, in a browser rather than as a function.
 *
 * `web/test/keys.test.ts` already pins what a key MEANS — including the
 * negative property that matters most, that a focused field yields nothing but
 * Escape — and `web/test/row.test.ts` what l and h do where the cursor stands.
 * What they cannot check is that the meaning is WIRED: that `j` reaches the
 * list's cursor, that the cursor is focus so a screen reader follows it, that
 * `/` lands the caret in the search field, and that ⌘K is the one chord this
 * app claims from the browser.
 *
 * The cursor is asserted through `document.activeElement`, because that is
 * what the app decided the cursor IS (`pages/todos.ts`: focus is the mark, so
 * Tab and `j` cannot disagree). A test that read a private variable would pass
 * while the ring was somewhere else.
 */

import assert from "node:assert/strict";

import { BASE, SIGNED_POST, suite, WRITE_BASE } from "./harness";
import cast from "../test/cast";

const { todos, mail, counsel } = cast.world;
/** One word that finds the agent's todo AND the first seeded message. */
const query = counsel.name.toLowerCase();

const app = await suite("keyboard");

const cursor = (): Promise<string | null> =>
  app.page.evaluate<string | null>(`document.activeElement?.getAttribute("data-key") ?? null`);

app.scene("j and k move the cursor down and up the list, group heads and lines alike", async () => {
  await app.open(BASE);
  await app.page.press("j");
  assert.equal(await cursor(), "c:work", "j from nowhere should take the first group's head");
  await app.page.press("j");
  assert.equal(await cursor(), `c:work/t:${todos.urgent.id}`);
  await app.page.press("k");
  assert.equal(await cursor(), "c:work");
  await app.page.press("k");
  assert.equal(await cursor(), "c:work", "k at the top stays at the top rather than wrapping");
});

app.scene("l opens a line in place and then the todo; h closes it and climbs to its group", async () => {
  await app.open(BASE);
  // The scene above left the cursor at the top; Escape lets it go, so j starts from nowhere again.
  await app.page.press("Escape");
  await app.page.press("j");
  await app.page.press("j");
  assert.equal(await cursor(), `c:work/t:${todos.urgent.id}`);
  await app.page.press("l");
  await app.page.waitFor(`document.querySelector('.rowopen[data-todo="${todos.urgent.id}"]') !== null`);
  await app.page.press("h");
  await app.page.waitFor(`document.querySelector('.rowopen[data-todo="${todos.urgent.id}"]') === null`);
  await app.page.press("h");
  assert.equal(await cursor(), "c:work", "h on a closed line climbs to its group");
  await app.page.press("h");
  await app.page.waitFor(`document.querySelector('.tree .row[data-todo="${todos.urgent.id}"]') === null`);
  await app.page.press("l");
  await app.page.waitFor(`document.querySelector('.tree .row[data-todo="${todos.urgent.id}"]') !== null`);
  await app.page.press("j");
  await app.page.press("Enter");
  await app.page.waitFor(`document.querySelector('.rowopen[data-todo="${todos.urgent.id}"]') !== null`);
  await app.page.press("Enter");
  await app.page.waitFor(`window.location.hash === "#/todo/${todos.urgent.id}"`);
});

app.scene("slash puts the caret in the search field, and the field then owns the keyboard", async () => {
  await app.open(BASE);
  await app.page.press("/");
  assert.equal(await app.page.evaluate<string | null>(`document.activeElement?.className ?? null`), "search");
  // The negative property, live: `j` in a field is the letter j, not a move.
  await app.page.press("j");
  assert.equal(await app.page.evaluate<string>(`document.querySelector(".search").value`), "j");
  await app.page.waitFor(`document.querySelectorAll(".tree .row").length === 0`);
  await app.page.press("Escape");
  // AND PUT IT BACK: the search is part of the remembered filter, and the
  // scenes below share this browser profile.
  await app.page.evaluate(`(() => {
    const field = document.querySelector(".search");
    field.value = "";
    field.dispatchEvent(new Event("input", { bubbles: true }));
  })()`);
  await app.page.waitFor(`document.querySelectorAll(".tree .row").length > 0`);
});

app.scene("Ctrl-K opens the palette and typing filters it", async () => {
  await app.open(BASE);
  await app.page.press("k", { ctrl: true });
  await app.page.waitFor(`document.querySelector(".overlaybox .palette") !== null`);
  assert.equal(
    await app.page.evaluate<string | null>(`document.activeElement?.className ?? null`),
    "draftfield palettefield",
    "the field is the point of the palette; it takes the keyboard immediately",
  );
  const before = await app.page.evaluate<number>(`document.querySelectorAll(".palrow").length`);
  assert.ok(before > 1, `the palette should offer the destinations and the todos, it offered ${before}`);

  await app.page.type(".palettefield", query);
  await app.page.waitFor(`document.querySelectorAll(".palrow").length === 1`);
  assert.equal(
    await app.page.evaluate<string | null>(`document.querySelector(".palrow .palhint")?.textContent ?? null`),
    todos.agents.id,
  );

  await app.page.press("Enter");
  await app.page.waitFor(`window.location.hash === "#/todo/${todos.agents.id}"`);
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll(".overlay").length`),
    0,
    "going somewhere closes the palette",
  );
});

/**
 * SEARCH EVERYTHING (project chain `searcheverything`, docs/APP.md §4 item 6):
 * the palette's mail source, in a real browser.
 *
 * SINCE `fixturemailisolation` THE FIXTURE HAS ITS OWN MAIL. `suzatary-dev
 * fixture-chain` (`app/cli/src/commands/fixture.rs::mail_fixture`) builds a
 * notmuch database in the fixture's own temp directory and seeds it with two
 * synthetic messages, so `/api/mail/search` here answers from THAT database
 * and never this box's — the content is known ahead of time, so this scene
 * asserts on it directly rather than only on shape. `MAIL_ONE_SUBJECT`
 * (`cast.world.mail[0]`, from counsel) is deliberately about the same deed
 * the agent's todo is, so one query
 * matches a local row and a mail row together, exactly as it would on a real
 * corpus.
 */
app.scene("the mail group appears below the local rows without disturbing them", async () => {
  await app.open(BASE);
  await app.page.press("k", { ctrl: true });
  await app.page.waitFor(`document.querySelector(".overlaybox .palette") !== null`);
  await app.page.type(".palettefield", query);
  await app.page.waitFor(`document.querySelectorAll(".palrow").length === 1`);
  assert.equal(
    await app.page.evaluate<string | null>(`document.querySelector(".palrow .palhint")?.textContent ?? null`),
    todos.agents.id,
    "the local row is on screen at once, before any async answer could have arrived",
  );

  // The debounce (250ms) plus the round trip to the fixture's own database.
  await app.page.waitFor(
    `[...document.querySelectorAll(".palgroup")].some((p) => p.textContent === "Mail")`,
    { timeout: 8000 },
  );
  // The mail row is the SEEDED message, by name — not just "some label".
  assert.equal(
    await app.page.evaluate<string | null>(
      `[...document.querySelectorAll(".palrow")]
        .find((r) => r.querySelector(".palhint")?.textContent !== "${todos.agents.id}")
        ?.querySelector(".pallabel")?.textContent ?? null`,
    ),
    mail[0].subject,
  );
  assert.equal(
    await app.page.evaluate<string | null>(
      `[...document.querySelectorAll(".palrow")]
        .find((r) => r.querySelector(".palhint")?.textContent !== "${todos.agents.id}")
        ?.querySelector(".palhint")?.textContent ?? null`,
    ),
    counsel.email,
  );
  // The local row is UNTOUCHED — the group is appended below it, not instead.
  assert.equal(
    await app.page.evaluate<string | null>(`document.querySelector(".palrow .palhint")?.textContent ?? null`),
    todos.agents.id,
  );
  await app.page.press("Escape");
});

/** `MAIL_TWO_SUBJECT` matches nothing local, so this is one row, unambiguous
 * to open: the whole point is proving a mail row is a REAL destination, not
 * only a label the palette can draw. */
app.scene("a seeded mail result opens on Enter", async () => {
  await app.open(BASE);
  await app.page.press("k", { ctrl: true });
  await app.page.waitFor(`document.querySelector(".overlaybox .palette") !== null`);
  await app.page.type(".palettefield", "ferret census");
  await app.page.waitFor(
    `[...document.querySelectorAll(".palgroup")].some((p) => p.textContent === "Mail")`,
    { timeout: 8000 },
  );
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll(".palrow").length`),
    1,
    "the seeded subject matches nothing else, local or mail",
  );
  assert.equal(
    await app.page.evaluate<string | null>(`document.querySelector(".palrow .pallabel")?.textContent ?? null`),
    mail[1].subject,
  );
  await app.page.press("Enter");
  await app.page.waitFor(`window.location.hash.startsWith("#/mail/")`);
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll(".overlay").length`),
    0,
    "opening the message closes the palette",
  );
});

/** Typing a second query before the first settles leaves no trace of the
 * first's async answer once things settle — `hits.accept`'s rule, live. Reads
 * only on WHETHER a row mentions the superseded query, never on the settled
 * query's actual content: the fixture's own mail is known and asserted on
 * above, so this scene stays about the RACE rather than repeating that. */
app.scene("a superseded query's async answer does not linger once a new one settles", async () => {
  await app.open(BASE);
  await app.page.press("k", { ctrl: true });
  await app.page.waitFor(`document.querySelector(".overlaybox .palette") !== null`);
  await app.page.type(".palettefield", query);
  // Cleared and retyped — a real field's own way of changing, so the "input"
  // listener `scheduleMail` hangs off fires exactly as a person's backspacing
  // would, whether or not the first query's own request had already gone out.
  await app.page.evaluate<void>(`(() => {
    const field = document.querySelector(".palettefield");
    field.value = "";
    field.dispatchEvent(new Event("input", { bubbles: true }));
  })()`);
  await app.page.type(".palettefield", "book");
  await app.page.waitFor(`document.querySelectorAll(".palrow").length === 1`, { timeout: 8000 });
  assert.equal(
    await app.page.evaluate<string | null>(`document.querySelector(".palrow .palhint")?.textContent ?? null`),
    todos.flat.id,
  );
  // Give the settled query's own debounce and round trip time to land.
  await new Promise((resolve) => setTimeout(resolve, 1500));
  const leaked = await app.page.evaluate<boolean>(
    `[...document.querySelectorAll(".palrow")].some((r) => /${query}/i.test(r.textContent ?? ""))`,
  );
  assert.equal(leaked, false, "no trace of the superseded query's async answer survives");
  await app.page.press("Escape");
});

/**
 * Every conversation the server holds is reachable the same way. `WRITE_BASE`,
 * not `BASE`, because starting one is a write (`openConversation`, the same
 * POST `start` always made) — this deployment's own conversation, so the
 * title this scene creates is exactly the one it looks for, with nothing else
 * on the chain to collide with it.
 *
 * MINTED DIRECTLY, over `fetch`, rather than through a "Start a conversation"
 * button: `pages/secretary.ts` no longer offers one
 * (`docs/ONECHAT-PLAN-2026-09-13.md` §4, §6 commit 7 — `#/secretary` forwards
 * to the default conversation now), and `start` itself is untouched, so this
 * is still the same write the button used to make.
 */
app.scene("the palette finds a conversation and Enter opens it", async () => {
  // Away from the secretary, so a match can only be the ASYNC group and not
  // a page-local `KeyTarget` command.
  await app.page.navigate(`${WRITE_BASE}/#/`);
  await app.page.press("k", { ctrl: true });
  await app.page.waitFor(`document.querySelector(".overlaybox .palette") !== null`);
  await app.page.type(".palettefield", "untitl");
  // Made once the palette is open and its query typed: the palette reads the replica as it moves, so the
  // conversation is found when the replica takes it in, with nothing typed again.
  await app.page.evaluate(
    `${SIGNED_POST}("/api/secretary/conversations", JSON.stringify({title: "Untitled"}))`,
  );
  await app.page.waitFor(
    `[...document.querySelectorAll(".palgroup")].some((p) => p.textContent === "Conversations")`,
    { timeout: 8000 },
  );
  assert.equal(
    await app.page.evaluate<string | null>(
      `[...document.querySelectorAll(".palrow")].map((r) => r.querySelector(".pallabel")?.textContent).find((t) => t === "Untitled") ?? null`,
    ),
    "Untitled",
  );

  await app.page.clickText(".pallabel", "Untitled");
  await app.page.waitFor(`window.location.hash.startsWith("#/secretary/")`);
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll(".overlay").length`),
    0,
    "going somewhere closes the palette",
  );
});

/**
 * `a`: "ask the secretary" with nothing on screen to point at (project chain
 * `askfromanywhere`, docs/APP.md §4 item 8). `WRITE_BASE`, not `BASE`, because
 * the compose box this scene is checking for is drawn only for a login that
 * may write (`conversation.ts`: "a read-only login gets the transcript and no
 * compose box") — `BASE`'s viewer would never show one to focus.
 */
app.scene("a opens the secretary with the compose box focused, from Todos", async () => {
  await app.open(WRITE_BASE);
  await app.page.press("a");
  await app.page.waitFor(`window.location.hash.startsWith("#/secretary")`);
  // The list forwards to the newest conversation (or opens one) once the login
  // is known to write — `secretary.ts`'s own `watchActor` race — so the wait
  // is for the compose box to exist before asking who has the keyboard.
  await app.page.waitFor(`document.querySelector(".composebox") !== null`);
  await app.page.waitFor(
    `document.activeElement !== null && document.activeElement.classList.contains("composebox")`,
  );
});

app.scene("a inside the search field does nothing — the field keeps the keyboard", async () => {
  await app.open(WRITE_BASE);
  const before = await app.page.evaluate<string>(`window.location.hash`);
  await app.page.press("/");
  assert.equal(await app.page.evaluate<string | null>(`document.activeElement?.className ?? null`), "search");
  await app.page.press("a");
  // The negative property, live: `a` in a field is the letter a, not the verb.
  assert.equal(await app.page.evaluate<string>(`document.querySelector(".search").value`), "a");
  assert.equal(await app.page.evaluate<string>(`window.location.hash`), before);
});
