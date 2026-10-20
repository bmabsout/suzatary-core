/**
 * The composer's Enter table (docs/CHATSHAPE-PLAN-2026-09-15.md §4.5): every
 * row, both widths, both modifier keys that mean "send".
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { composeKey } from "../src/chat/compose";
import type { ComposeKeyEvent } from "../src/chat/compose";

const WIDE = true;
const PHONE = false;

const key = (overrides: Partial<ComposeKeyEvent> = {}): ComposeKeyEvent => ({
  key: "Enter",
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  ...overrides,
});

test("Ctrl-Enter and Cmd-Enter send at every width", () => {
  assert.equal(composeKey(key({ ctrlKey: true }), WIDE, "written"), "send");
  assert.equal(composeKey(key({ ctrlKey: true }), PHONE, "written"), "send");
  assert.equal(composeKey(key({ metaKey: true }), WIDE, "written"), "send");
  assert.equal(composeKey(key({ metaKey: true }), PHONE, "written"), "send");
  // Shift held alongside the chord changes nothing — Ctrl/Cmd already decided.
  assert.equal(composeKey(key({ ctrlKey: true, shiftKey: true }), WIDE, "written"), "send");
});

test("at 62rem and up, a bare Enter sends and Shift-Enter breaks a line", () => {
  assert.equal(composeKey(key(), WIDE, "written"), "send");
  assert.equal(composeKey(key({ shiftKey: true }), WIDE, "written"), "newline");
});

test("under 62rem, a bare Enter stays a newline — the button is the send", () => {
  assert.equal(composeKey(key(), PHONE, "written"), "newline");
  assert.equal(composeKey(key({ shiftKey: true }), PHONE, "written"), "newline");
});

test("Escape is stop at either width", () => {
  for (const wide of [WIDE, PHONE]) assert.equal(composeKey(key({ key: "Escape" }), wide, "written"), "stop");
});

test("↑ in an empty box is edit; written, or with a modifier, it is none", () => {
  for (const wide of [WIDE, PHONE]) {
    assert.equal(composeKey(key({ key: "ArrowUp" }), wide, "empty"), "edit");
    assert.equal(composeKey(key({ key: "ArrowUp" }), wide, "written"), "none");
    for (const held of [{ ctrlKey: true }, { metaKey: true }, { shiftKey: true }]) {
      assert.equal(composeKey(key({ key: "ArrowUp", ...held }), wide, "empty"), "none");
    }
  }
});

test("every key but ↑ decides the same in an empty box", () => {
  for (const wide of [WIDE, PHONE]) {
    for (const k of ["Enter", "Escape", "a", "Tab", "ArrowDown"]) {
      for (const held of [{}, { ctrlKey: true }, { shiftKey: true }]) {
        const event = key({ key: k, ...held });
        assert.equal(composeKey(event, wide, "empty"), composeKey(event, wide, "written"), k);
      }
    }
  }
});

test("every other key is none, at either width", () => {
  for (const wide of [WIDE, PHONE]) {
    assert.equal(composeKey(key({ key: "a" }), wide, "written"), "none");
    assert.equal(composeKey(key({ key: "Tab" }), wide, "written"), "none");
  }
});
