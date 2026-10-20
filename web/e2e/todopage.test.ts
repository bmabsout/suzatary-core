/**
 * A todo's page on a phone: nothing on it is wider than the screen, and
 * "Why this number" is sentences, not the term's fields.
 *
 * The history shows each `Authored` record's fields as the chain printed them
 * — a spec is a constructor literal hundreds of characters long, with no
 * space to break at — and it is evidence, so it stays. What it may not do is
 * widen the page: a long value wraps, a literal scrolls in its own box.
 */

import assert from "node:assert/strict";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { BASE, PHONE, suite } from "./harness";
import cast from "../test/cast";

const { todos } = cast.world;

const app = await suite("todopage");

const SHOTS = process.env["SUZATARY_SHOTS"] ?? join(tmpdir(), "suzatary-shots");
mkdirSync(SHOTS, { recursive: true });

/** Every field and note name the core's explanation carries. */
const RAW = [
  "leadUpHours", "endDate", "startDate", "windowHours", "deltaHours", "periodHours", "needsHours",
  "peakAt", "peakShare", "certifies", "shares", "agoHours", "cycleStart", "anchor", "=",
];

async function openPage(): Promise<void> {
  await app.page.navigate(`${BASE}/#/todo/${todos.urgent.id}`);
  await app.page.waitFor(
    `document.querySelector(".todoview .whytree") !== null && document.querySelector(".todoview .history .timeline .diff") !== null`,
    { timeout: 20000 },
  );
}

/** The page with its history opened, as a person opens it. */
async function openHistory(): Promise<void> {
  await openPage();
  if (!(await app.page.evaluate<boolean>(`document.querySelector(".todoview .history").open`))) await app.page.click(".todoview .history > summary");
  await app.page.waitFor(`document.querySelector(".todoview .history").open === true`);
}

app.scene("under the title: no note on how it was drawn, no empty box, and who wrote it in one line", async () => {
  await openPage();
  const said = await app.page.evaluate<string>(`document.querySelector(".todoview .todohead").textContent`);
  assert.ok(!said.includes("Rendered by typst"), said);
  assert.equal(
    await app.page.evaluate<boolean>(`document.querySelector(".todoview .actrow").hidden`),
    true,
    "a viewer is offered no act, and the empty row is not drawn",
  );
  const line = await app.page.evaluate<{ summary: string; open: boolean; body: string }>(`(() => {
    const d = document.querySelector(".todoview details.provline");
    return { summary: d.querySelector("summary").textContent, open: d.open, body: d.querySelector("p").textContent };
  })()`);
  assert.match(line.summary, /^Written by \S+ on \S+ \S+$/, line.summary);
  assert.equal(line.open, false, "why it counts waits behind the line");
  assert.match(line.body, /^Confirmed/, line.body);
  for (const word of ["fold", "trusted actor", "chain"]) assert.ok(!`${line.summary} ${line.body}`.includes(word), word);
  assert.equal(await app.page.evaluate<boolean>(`document.querySelector(".todoview .history").open`), false, "the history is closed");
});

app.scene("at 390px the todo page is no wider than the screen", async () => {
  await openHistory();
  const [scroll, inner] = await app.page.evaluate<[number, number]>(
    `[document.documentElement.scrollWidth, window.innerWidth]`,
  );
  assert.ok(scroll <= inner, `the document is ${scroll}px wide on a ${inner}px screen`);
  const wider = await app.page.evaluate<string[]>(
    `[...document.querySelectorAll(".todoview *")].filter((e) => e.getBoundingClientRect().right > window.innerWidth + 0.5 && e.closest(".literal") === null).map((e) => e.tagName + "." + e.className)`,
  );
  assert.deepEqual(wider, [], "every box ends inside the screen, a literal's scroll box aside");
  const literal = await app.page.evaluate<boolean>(
    `[...document.querySelectorAll(".timeline .literal")].some((e) => e.scrollWidth > e.clientWidth)`,
  );
  assert.ok(literal, "a spec's literal is shown whole, in a box that scrolls");
});

app.scene("why this number says the price in words", async () => {
  await openPage();
  const said = await app.page.evaluate<string>(`document.querySelector(".todoview .whytree").textContent`);
  for (const raw of RAW) assert.ok(!said.includes(raw), `"${raw}" in: ${said}`);
  assert.ok(!/\d{4}-\d{2}-\d{2}T/.test(said), `an ISO instant in: ${said}`);
  assert.match(
    await app.page.evaluate<string>(`document.querySelector(".todoview .whytree .says").textContent`),
    /^\d+\.\d% /,
  );
});

app.scene("a photograph of the whole page at 390px", async () => {
  await openHistory();
  const height = await app.page.evaluate<number>(`document.documentElement.scrollHeight`);
  await app.page.viewport(PHONE[0], height);
  await app.page.waitFor(`window.innerHeight === ${height}`);
  await app.page.screenshot(join(SHOTS, "todo-page-390.png"));
  await app.page.viewport(PHONE[0], PHONE[1]);
});
