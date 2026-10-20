/**
 * The verb-phrase table, in isolation — pure string mapping, no DOM.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { verbPhrase } from "../src/chat/verbphrase";

test("the listed verbs get their short phrase", () => {
  assert.equal(verbPhrase("search"), "searching mail");
  assert.equal(verbPhrase("show"), "reading a message");
  assert.equal(verbPhrase("todo_show"), "reading todos");
  assert.equal(verbPhrase("list_open_todos"), "reading todos");
  assert.equal(verbPhrase("briefing_delta"), "reading what changed");
  assert.equal(verbPhrase("context"), "reading notes");
  assert.equal(verbPhrase("propose_email"), "drafting an email");
});

test("an unlisted verb falls back to its own name", () => {
  assert.equal(verbPhrase("run_verification"), "run_verification");
  assert.equal(verbPhrase("todo_add"), "todo_add");
  assert.equal(verbPhrase(""), "");
});
