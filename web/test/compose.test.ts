/**
 * The composer's Enter table (docs/CHATSHAPE-PLAN-2026-09-15.md §4.5): every
 * row, both widths, both modifier keys that mean "send".
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { composeKey } from "../src/chat/compose";
import type { Box, ComposeDecision, ComposeKeyEvent } from "../src/chat/compose";

const WIDE = true;
const PHONE = false;

const key = (overrides: Partial<ComposeKeyEvent> = {}): ComposeKeyEvent => ({
  key: "Enter",
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  ...overrides,
});

type Row = [Partial<ComposeKeyEvent>, boolean, Box, ComposeDecision];

test("every row of the table, at both widths, empty or written", () => {
  const rows: Row[] = [
    // Ctrl-Enter and Cmd-Enter send at every width; Shift held alongside the chord changes nothing.
    [{ ctrlKey: true }, WIDE, "written", "send"],
    [{ ctrlKey: true }, PHONE, "written", "send"],
    [{ metaKey: true }, WIDE, "written", "send"],
    [{ metaKey: true }, PHONE, "written", "send"],
    [{ ctrlKey: true, shiftKey: true }, WIDE, "written", "send"],
    // At 62rem and up a bare Enter sends and Shift-Enter breaks a line; under it, a bare Enter is a newline.
    [{}, WIDE, "written", "send"],
    [{ shiftKey: true }, WIDE, "written", "newline"],
    [{}, PHONE, "written", "newline"],
    [{ shiftKey: true }, PHONE, "written", "newline"],
    ...[WIDE, PHONE].flatMap((wide): Row[] => [
      [{ key: "Escape" }, wide, "written", "stop"],
      // ↑ in an empty box is edit; written, or with a modifier, it is none.
      [{ key: "ArrowUp" }, wide, "empty", "edit"],
      [{ key: "ArrowUp" }, wide, "written", "none"],
      [{ key: "ArrowUp", ctrlKey: true }, wide, "empty", "none"],
      [{ key: "ArrowUp", metaKey: true }, wide, "empty", "none"],
      [{ key: "ArrowUp", shiftKey: true }, wide, "empty", "none"],
      [{ key: "a" }, wide, "written", "none"],
      [{ key: "Tab" }, wide, "written", "none"],
    ]),
  ];
  for (const [over, wide, box, expected] of rows) {
    assert.equal(composeKey(key(over), wide, box), expected, JSON.stringify([over, wide, box]));
  }
  // Every key but ↑ decides the same in an empty box.
  for (const wide of [WIDE, PHONE]) {
    for (const k of ["Enter", "Escape", "a", "Tab", "ArrowDown"]) {
      for (const held of [{}, { ctrlKey: true }, { shiftKey: true }]) {
        const event = key({ key: k, ...held });
        assert.equal(composeKey(event, wide, "empty"), composeKey(event, wide, "written"), k);
      }
    }
  }
});
