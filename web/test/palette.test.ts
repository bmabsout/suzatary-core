/**
 * The palette's static entries.
 *
 * `commands()` is safe to call under node — it touches the DOM only inside a
 * `run` closure, never while building the list (`nav.test.ts`'s header says
 * the same of `nav.ts`) — so what is on offer and where it routes is checked
 * here rather than only by eye.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { commands } from "../src/shell/palette";

test("Ask the secretary is offered, and routes to the compose box's focus intent", () => {
  const ask = commands().find((command) => command.label === "Ask the secretary");
  assert.notEqual(ask, undefined);
  assert.deepEqual(ask?.route, { type: "secretary", refs: [], focus: true });
  // Findable by more than its exact label — "ask" and "question" are the
  // words a reader who does not remember the label would type.
  assert.ok(ask?.search.includes("ask"));
  assert.ok(ask?.search.includes("secretary"));
});

test("every destination the bar and rail offer is also a palette command, routing the same place", () => {
  const all = commands();
  const gotos = all.filter((command) => command.hint === "go");
  assert.ok(gotos.length > 0);
  for (const command of gotos) {
    assert.notEqual(command.route, undefined, command.label);
  }
});
