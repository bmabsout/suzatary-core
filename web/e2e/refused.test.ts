/**
 * A LOGIN THAT MAY NOT WRITE, and an object the box will not take.
 *
 * These are the two refusals a replica meets, and they are different in kind.
 * The first is answered BEFORE anything is written: the box names no actor for
 * this login (`GET /api/me`), so the app offers no act controls at all — a
 * button that promised a write the box will refuse is worse than an absent one,
 * because the reader only finds out after tapping. The second is answered
 * AFTER: an object this replica sealed, folded and is showing, which the box
 * will not adopt. That one is kept and NAMED, never silently retried and never
 * quietly dropped, because the object is real and this browser holds it.
 *
 * `E2E_BASE` is gated to a login this deployment knows only as a VIEWER, so
 * both refusals are the box's own and nothing here had to invent an error to
 * catch. It is also why the read-only files can share this server: its chain
 * cannot move.
 */

import assert from "node:assert/strict";

import { BASE, fold, openInPlace, openTodo, shownTodos, suite, WRITE_BASE, WRITE_HEADERS } from "./harness";
import cast from "../test/cast";

const { todos } = cast.world;

const app = await suite("refused");

app.scene("a read-only login gets no act controls at all", async () => {
  await app.open(BASE);
  const before = await shownTodos(app.page);
  assert.ok(before.includes(todos.high.id), "the list still reads fine without a writer");

  // The box named no actor for this login.
  const who = await app.page.evaluate<string>(
    `fetch("/api/me").then((r) => r.json()).then((b) => JSON.stringify(b))`,
  );
  assert.match(who, /"actor":null/, `a viewer has no actor, /api/me said ${who}`);

  // So no line opened in place carries a Done, on the list…
  await openInPlace(app.page, todos.high.id);
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll(".rowopen .act-done, .rowopen .addsub").length`),
    0,
    "a login that cannot seal is not offered a button that seals",
  );
  // …and the todo view offers none either, where every verb normally lives.
  //
  // ⚠️ `.todoview` EXISTS TWICE: `todoview.ts`'s `skeleton()` (the "loading…"
  // placeholder `run()` draws synchronously on the click, before the fetch)
  // carries the same `column todoview` class as `draw()`'s real content, so
  // waiting on `.todoview` alone is satisfied by the SKELETON and races the
  // `/api/todo/water-plants` round trip that fills in `.actrow` — caught by
  // looping this scene (intermittent, 1 failure in 15 runs: `0 !== 1` on
  // `.actrow.readonly`, the skeleton has no actrow of either shape at all).
  // `.actrow` is what `draw()` adds and `skeleton()` never does, so waiting
  // on IT is waiting on the thing the assertions below are actually about,
  // the way `list.test.ts` waits on `.todoview .todoid` for the same reason.
  await openTodo(app.page, todos.high.id);
  await app.page.waitFor(`document.querySelector(".todoview .actrow") !== null`, { timeout: 4000 });
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll(".actrow .act").length`), 0);
  assert.equal(
    await app.page.evaluate<boolean>(`document.querySelector(".todoview .actrow").hidden`),
    true,
    "a row with nothing in it is not drawn: no empty box under the header",
  );

  assert.equal(
    (await fold(app.page)).find((entry) => entry.todo === todos.high.id)?.state,
    "open",
    "and nothing reached the chain",
  );
});

app.scene("an object the box refuses comes back named, with the box's reason", async () => {
  // The WRITE server, because this login has an actor and so gets past the
  // gate — which is the only way to reach the PER-OBJECT refusals at all. A
  // viewer is refused one layer earlier, for the whole request, which is the
  // scene above.
  await app.open(WRITE_BASE);

  // A REAL REFUSED OBJECT, appended by the core in this tab exactly as an act is
  // — but naming an actor this login is not. `sync::receive` refuses an event
  // whose actor is not the poster's: the rule that stops a page writing as
  // somebody else, and the reason the app itself can never produce one of these
  // (it seals as the actor `/api/me` named). Nothing is stubbed: the bytes are
  // canonical, the name is the core's hash of them, and the sentence is the
  // box's.
  const answer = await app.page.evaluate<string>(`(async () => {
    const glue = await import("/b/dev/prodrome.js");
    await glue.default({ module_or_path: new URL("/b/dev/prodrome_bg.wasm", location.href).href });
    const held = await fetch("/api/objects").then((r) => r.json());
    const replica = new glue.Todos(JSON.stringify(held.objects.map((o) => ({ hash: o.hash, text: o.literal }))));
    const event = glue.lifecycle("Completed", "${todos.high.id}", "2026-09-02T11:00:00", "triage", "");
    const appended = JSON.parse(replica.append(event, null));
    replica.free();
    const receipt = await fetch("/api/objects", {
      method: "POST",
      headers: ${WRITE_HEADERS},
      body: JSON.stringify({ objects: appended.objects.map((o) => o.text) }),
    }).then((r) => r.json());
    return JSON.stringify({ name: appended.hash, receipt });
  })()`);
  const { name, receipt } = JSON.parse(answer) as {
    name: string;
    receipt: { adopted: string[]; refused: { hash: string; reason: string }[] };
  };

  assert.deepEqual(receipt.adopted, [], "nothing was adopted");
  assert.equal(receipt.refused.length, 1);
  assert.equal(receipt.refused[0]?.hash, name, "the refusal names the object BY THE HASH THE CORE COMPUTED");
  assert.match(
    receipt.refused[0]?.reason ?? "",
    /triage/,
    `the box says whose event it is, it said ${receipt.refused[0]?.reason ?? ""}`,
  );

  assert.equal(
    (await fold(app.page)).find((entry) => entry.todo === todos.high.id)?.state,
    "open",
    "and the chain is exactly where it was",
  );
});
