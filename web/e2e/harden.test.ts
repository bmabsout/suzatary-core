/**
 * THE ONE DOOR'S HARDENING PASS, in a real browser.
 *
 * `rich.ts` is the single place in this app that turns a string into nodes,
 * and `harden()` is what it does to those nodes before anything sees them:
 * every anchor `typst` emitted is rewritten, and the rewrite has two arms.
 * A `todo:<id>` becomes an in-app hash — the one explicit way a todo names
 * another, and the rule CLAUDE.md states by name in the Layout paragraph.
 * Everything else is OUTBOUND, and leaves with `rel="noopener noreferrer"`
 * and `target="_blank"`.
 *
 * ⚠️ IT WAS THE LEAST-TESTED SAFETY-RELEVANT CODE IN THE TREE, and the reason
 * is structural: `harden` needs a DOM, so it cannot be a node test, and no
 * browser scene mentioned `todolink` or `noopener`. Every other rule of this
 * weight is pinned by name — `stale.ts`'s two sentences, `nav.ts`'s five-slot
 * rule, `keys.ts`'s negative properties — so this one stood out by being the
 * exception.
 *
 * It is asserted on ONE rendered body carrying BOTH kinds of anchor
 * (`pack-bags`' detail, `app/cli/src/commands/fixture.rs`), because the claim
 * is not "links are rewritten" but that the two kinds are told apart — and a
 * fixture with only one kind can only pin half of that.
 *
 * The chain is the READ base: nothing here writes, so it shares the server
 * every read-only file shares.
 */

import assert from "node:assert/strict";

import { BASE, DESKTOP, suite } from "./harness";
import cast from "../test/cast";

const { todos } = cast.world;

const app = await suite("harden");

/** Everything the rendered detail says about one anchor, by its visible text.
 * Read off the LIVE DOM rather than off the payload: `harden` runs on nodes
 * after parsing, so the payload's HTML is what it was handed and not what it
 * produced. */
const anchorBy = async (text: string): Promise<{ href: string; rel: string; target: string; cls: string } | null> =>
  app.page.evaluate<{ href: string; rel: string; target: string; cls: string } | null>(`(() => {
    const anchor = [...document.querySelectorAll(".todoview .rich a")].find(
      (candidate) => candidate.textContent?.includes(${JSON.stringify(text)}),
    );
    if (anchor === undefined) return null;
    return {
      href: anchor.getAttribute("href") ?? "",
      rel: anchor.getAttribute("rel") ?? "",
      target: anchor.getAttribute("target") ?? "",
      cls: anchor.getAttribute("class") ?? "",
    };
  })()`);

const openPackBags = async (): Promise<void> => {
  await app.open(BASE, `#/todo/${todos.checklist.id}`);
  // The detail is a disclosure inside the view; wait for the RENDERED body,
  // because a cold typst cache serves source and there are no anchors in
  // source at all.
  await app.page.waitFor(`document.querySelectorAll(".todoview .rich a").length >= 2`, { timeout: 20000 });
};

app.scene("a todo: link becomes a hash into this app, and carries no target", async () => {
  await app.resize(DESKTOP);
  await openPackBags();
  const link = await anchorBy("the flight");
  assert.ok(link !== null, "the rendered detail should carry the todo: anchor");
  assert.equal(link.href, `#/todo/${todos.flat.id}`, "a todo: link must be rewritten to the in-app hash");
  // A new tab for a link INTO this app would leave the reader with two copies
  // of the same tab and a Back button that does nothing.
  assert.equal(link.target, "", "an in-app link must not open a new tab");
  assert.ok(link.cls.split(/\s+/).includes("todolink"), "the rewritten anchor is marked, so the stylesheet can say so");
});

app.scene("that hash is real: following it opens the todo it names", async () => {
  await app.resize(DESKTOP);
  await openPackBags();
  await app.page.evaluate(
    `[...document.querySelectorAll(".todoview .rich a")].find((a) => a.getAttribute("href") === "#/todo/${todos.flat.id}")?.click()`,
  );
  await app.page.waitFor(`window.location.hash === "#/todo/${todos.flat.id}"`);
  await app.page.waitFor(`document.querySelector(".todoview .todoid")?.textContent === "${todos.flat.id}"`);
});

app.scene("an outbound link leaves with noopener noreferrer, in a new tab", async () => {
  await app.resize(DESKTOP);
  await openPackBags();
  const link = await anchorBy("airline");
  assert.ok(link !== null, "the rendered detail should carry the outbound anchor");
  assert.equal(link.href, "https://example.invalid/baggage", "an outbound href is left exactly as typst wrote it");
  // `noopener` is the one that matters: without it the opened page holds a
  // `window.opener` handle onto this one and can navigate it. `noreferrer`
  // rides along because every string on this page came out of mail.
  assert.equal(link.rel, "noopener noreferrer");
  assert.equal(link.target, "_blank");
  assert.ok(!link.cls.split(/\s+/).includes("todolink"), "an outbound link is not an in-app one");
});
