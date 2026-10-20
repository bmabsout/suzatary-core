/**
 * THE REFERENCE GRAMMAR, IN THE BROWSER'S HALF OF IT.
 *
 * `suzatary_core::reference` is the authority: it refuses a reference on the
 * way in and it decides what the stored bytes mean. `web/src/chat/refs.ts` is the
 * SECOND reader, and it exists because the linking happens here, over prose
 * that arrives as one string with no offsets. Two readers of one grammar is
 * exactly the shape this repository normally refuses, so the ones that could
 * disagree are pinned:
 *
 * - THE SIX SPELLINGS are the same six `suzatary_core::reference`'s own test
 *   asserts, character for character. If either side changes one, one of the
 *   two files goes red.
 * - THE ROUND TRIP is a law here as it is there: whatever `print` writes,
 *   `parse` reads back as the same value.
 * - THE TOKENISER LOSES NOTHING: concatenating every token's source is the
 *   input. A scan that silently ate a character would be a page quietly
 *   misquoting the model, which is worse than a missed link.
 *
 * Nothing here touches the DOM: `anchor` and `textWithReferences` build nodes
 * and belong to the browser scenes (`web/e2e/secretary.test.ts`), which is
 * where a link that does not navigate would show up.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import type { Reference } from "../src/chat/refs";
import { askHref, hrefOf, parse, print, tokenise, urlTokens } from "../src/chat/refs";

/** The six, as `SECRETARY.md` shows them to the model and as
 * `suzatary_core::reference::tests` asserts them. */
const SPELLINGS = [
  "todo:life/book-flight",
  "mail:a1b2c3d4e5f6",
  "thread:0000000000001b2c",
  "briefing:2026-09-10",
  "proposal:2026-09-09-1/1",
  "batch:life/2fdf1691",
];

test("the six text forms are the ones the manual names, and they round-trip", () => {
  for (const spelling of SPELLINGS) {
    const reference = parse(spelling);
    assert.notEqual(reference, null, spelling);
    assert.equal(print(reference as Reference), spelling);
  }
});

test("a string that is not a reference is not one", () => {
  for (const bad of [
    "",
    "todo",
    "todo:",
    "todo:life",
    "todo:life/",
    "todo:/x",
    "todo:../x",
    "todo:life/../etc",
    "todo:Life/x",
    "mail:",
    "mail:abc",
    "mail:ABCDEF",
    "mail:zzzzzz",
    "thread:",
    "thread:a/b",
    "thread:a b",
    "briefing:",
    "briefing:2026-9-10",
    "briefing:2026-13-01",
    "briefing:tomorrow",
    "proposal:2026-09-09-1",
    "proposal:2026-09-09-1/x",
    "proposal:../etc/0",
    "batch:",
    "batch:life",
    "batch:life/",
    "batch:life/abcdef1",
    "batch:Life/abcdef1234",
    "batch:../etc/abcdef1234",
    "note:hello",
    "https://example.test",
  ]) {
    assert.equal(parse(bad), null, `${JSON.stringify(bad)} must not parse`);
  }
});

test("every reference has a destination in this app", () => {
  assert.equal(hrefOf({ kind: "todo", chain: "life", id: "x" }), "#/todo/x");
  // THE CHAIN IS PART OF THE NAME AND NOT PART OF THE LINK, and that is
  // `route.ts`'s decision: `#/todo/<id>` deliberately pins no chain, so opening
  // a todo from either list stays on the list's.
  assert.equal(hrefOf({ kind: "todo", chain: "suzatary", id: "x" }), "#/todo/x");
  assert.equal(hrefOf({ kind: "mail", hash: "a1b2c3" }), "#/mail/a1b2c3");
  assert.equal(hrefOf({ kind: "thread", id: "00ab" }), "#/mail/thread/00ab");
  assert.equal(hrefOf({ kind: "briefing", date: "2026-09-10" }), "#/briefings/2026-09-10");
  assert.equal(
    hrefOf({ kind: "proposal", conversation: "2026-09-09-1", index: 1 }),
    "#/secretary/2026-09-09-1",
  );
  // A HELD BATCH HAS NO PAGE OF ITS OWN EITHER — it is a card on the Inbox,
  // exactly like a decision about a proposal.
  assert.equal(hrefOf({ kind: "batch", chain: "life", head: "2fdf1691" }), "#/inbox");
});

test("ask-the-secretary carries the reference in the hash", () => {
  assert.equal(
    askHref({ kind: "todo", chain: "life", id: "permitfiling" }),
    "#/secretary?ref=todo%3Alife%2Fpermitfiling",
  );
});

test("a sentence splits into prose and the things it names", () => {
  const tokens = tokenise("Look at todo:life/permitfiling and mail:a1b2c3d4e5f6, then reply.");
  assert.deepEqual(tokens, [
    { text: "Look at " },
    { reference: { kind: "todo", chain: "life", id: "permitfiling" } },
    { text: " and " },
    { reference: { kind: "mail", hash: "a1b2c3d4e5f6" } },
    { text: ", then reply." },
  ]);
});

/** NOTHING IS DROPPED AND NOTHING IS INVENTED. The one property that matters
 * for a tokeniser over somebody's prose. */
test("the tokens reassemble into exactly the input", () => {
  for (const text of [
    "",
    "no references here at all",
    "todo:life/x",
    "todo:life/x todo:life/y",
    "(mail:a1b2c3d4e5f6) and briefing:2026-09-10.",
    "notodo:life/x is a word, xmail:a1b2c3d4e5f6 is another",
    "thread:00ab. ends a sentence",
    SPELLINGS.join(" · "),
  ]) {
    const rebuilt = tokenise(text)
      .map((token) => ("text" in token ? token.text : print(token.reference)))
      .join("");
    assert.equal(rebuilt, text, JSON.stringify(text));
  }
});

/** A reference starts at a boundary, so a word that ENDS in one is a word.
 * Without this, "notodo:life/x" would link and the link would be wrong. */
test("a scheme in the middle of a word is not a reference", () => {
  const tokens = tokenise("notodo:life/x and xmail:a1b2c3d4e5f6");
  assert.deepEqual(tokens, [{ text: "notodo:life/x and xmail:a1b2c3d4e5f6" }]);
});

/** Prose puts punctuation next to things. A thread id may hold a dot, so the
 * scan refuses one at the end rather than swallowing the full stop. */
test("trailing punctuation stays prose", () => {
  assert.deepEqual(tokenise("see thread:00ab."), [
    { text: "see " },
    { reference: { kind: "thread", id: "00ab" } },
    { text: "." },
  ]);
});

/** A bare `todo:<id>` is DELIBERATELY not matched. It is the spelling typst's
 * `#link("todo:x")` uses, rewritten by `rich.ts`, and it names a todo on
 * whichever chain is open — the ambiguity `Reference::Todo` carries a chain to
 * avoid. Two grammars in one tokeniser would make the safer one unreachable. */
test("a chainless todo: is left as prose", () => {
  assert.deepEqual(tokenise("todo:permitfiling"), [{ text: "todo:permitfiling" }]);
});

/**
 * `urlTokens` IS THE DECISION `textWithReferences` LINKS ON, PURE OF THE DOM
 * (`docs/SECRETARYACTS-PLAN-2026-09-14.md` §3, §6 commit 6): an act turn's
 * own device-code URL becomes a link and a model turn's stays plain text —
 * `textWithReferences(text, who)` gates this call on `who === "act"`, which is
 * the browser scene's to prove (`web/e2e`), since that is where a link that
 * does not `target="_blank"`/`rel="noopener"` would show up. What is checked
 * here is the boundary this module owns: WHICH substring is a linkable URL.
 */
test("an https url is its own token, split out of the surrounding prose", () => {
  assert.deepEqual(
    urlTokens("Sign in at https://microsoft.com/devicelogin and enter the code."),
    [
      { text: "Sign in at " },
      { url: "https://microsoft.com/devicelogin" },
      { text: " and enter the code." },
    ],
  );
});

test("a bare http url is linked too, and a non-web scheme never is", () => {
  assert.deepEqual(urlTokens("try http://example.test/x"), [
    { text: "try " },
    { url: "http://example.test/x" },
  ]);
  assert.deepEqual(urlTokens("run javascript:alert(1) now"), [
    { text: "run javascript:alert(1) now" },
  ]);
  assert.deepEqual(urlTokens("open data:text/html,<script>1</script> now"), [
    { text: "open data:text/html,<script>1</script> now" },
  ]);
});

test("prose with no url at all is one text token", () => {
  assert.deepEqual(urlTokens("nothing to link here"), [{ text: "nothing to link here" }]);
});

test("urlTokens reassembles into exactly the input", () => {
  for (const text of [
    "",
    "no urls here",
    "https://a.test",
    "before https://a.test after",
    "two https://a.test and https://b.test/path?q=1",
  ]) {
    const rebuilt = urlTokens(text)
      .map((token) => ("text" in token ? token.text : token.url))
      .join("");
    assert.equal(rebuilt, text, JSON.stringify(text));
  }
});
