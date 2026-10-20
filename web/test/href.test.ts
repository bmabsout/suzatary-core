/**
 * `view/href.ts` — what an anchor typst emitted may lead to, read as the browser reads it.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import href from "../src/view/href";
import { forAll } from "./gen";

const BASE = "https://box.example.test/#/todo/life/x";

/** Every C0 control, the space, and the whitespace a browser's URL parser strips or skips. */
const STRAY = [...Array.from({ length: 0x20 }, (_, c) => String.fromCharCode(c)), " ", "\u00a0", "\u2028", "\ufeff"];

const EXECUTABLE = ["javascript:alert(1)", "vbscript:msgbox(1)", "data:text/html,<p>x"];

/** Never a link the browser would follow into a scheme that runs: inert, or outbound to one that navigates. */
const runsNothing = (raw: string): void => {
  const h = href.of(raw, BASE);
  if (h.type === "Outbound") assert.ok(href.OUTBOUND.has(new URL(h.href, BASE).protocol), JSON.stringify(raw));
  else assert.notEqual(h.type, "Todo", JSON.stringify(raw));
};

/** `text` with `stray` at `at`, a case of each letter flipped as the seed says. */
const salted = (text: string, at: number, stray: string, flips: boolean[]): string =>
  [...text.slice(0, at), stray, ...text.slice(at)].map((ch, i) => (flips[i] === true ? ch.toUpperCase() : ch)).join("");

test("law: an executable scheme never leads anywhere that runs, whatever control or space sits inside it", () => {
  for (const url of EXECUTABLE) {
    const scheme = url.indexOf(":") + 1;
    for (const stray of STRAY) {
      for (let at = 0; at <= scheme; at += 1) {
        runsNothing(salted(url, at, stray, []));
      }
    }
  }
});

test("law: any number of strays, in any case, still never lead anywhere that runs", () => {
  forAll(500, (r) => {
    let raw = r.pick(EXECUTABLE);
    for (let n = r.int(4); n >= 0; n -= 1) raw = salted(raw, r.int(raw.indexOf(":") + 2), r.pick(STRAY), r.array(raw.length + 1, () => r.bool()));
    runsNothing(raw);
  });
});

test("an outbound link of a scheme that navigates is kept as written", () => {
  for (const raw of ["https://example.invalid/baggage", "http://example.invalid", "mailto:a@example.invalid", "tel:+1-555-555-0100"]) {
    assert.deepEqual(href.of(raw, BASE), { type: "Outbound", href: raw });
  }
});

test("a todo: link names its todo, a bare one and any other scheme name nothing", () => {
  assert.deepEqual(href.of("todo:life/flight", BASE), { type: "Todo", id: "life/flight" });
  for (const raw of ["todo:", "ftp://example.invalid/x", "blob:https://box.example.test/x", "file:///etc/passwd", "http://[x"]) {
    assert.deepEqual(href.of(raw, BASE), { type: "Inert" }, raw);
  }
});
