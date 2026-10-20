/**
 * The theme override's DECISION, which is the only part of it a browser is not
 * needed for: what a stored string means, and what goes on the root element.
 *
 * The two properties worth pinning are the ones a two-state toggle gets wrong:
 * `auto` writes nothing (so the media query still decides), and anything that
 * is not one of the three reads as `auto` rather than throwing a page away.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { THEMES, attributeFor, isTheme, labelFor, parseTheme } from "../src/app/theme";

test("a stored value that is not one of the three is the machine's answer", () => {
  assert.equal(parseTheme(null), "auto");
  assert.equal(parseTheme(""), "auto");
  assert.equal(parseTheme("Dark"), "auto");
  assert.equal(parseTheme("sepia"), "auto");
  assert.equal(parseTheme("auto"), "auto");
  assert.equal(parseTheme("light"), "light");
  assert.equal(parseTheme("dark"), "dark");
});

test("auto is an ABSENT attribute, not a value", () => {
  assert.equal(attributeFor("auto"), null);
  assert.equal(attributeFor("light"), "light");
  assert.equal(attributeFor("dark"), "dark");
});

test("every theme has a name, and the guard admits exactly those three", () => {
  assert.deepEqual(THEMES.map(labelFor), ["System", "Light", "Dark"]);
  for (const value of THEMES) assert.equal(isTheme(value), true);
  for (const value of [null, undefined, 0, "AUTO", {}]) assert.equal(isTheme(value), false);
});

test("a round trip through the store is the identity on all three", () => {
  for (const value of THEMES) assert.equal(parseTheme(value), value);
});

