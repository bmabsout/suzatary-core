/**
 * THE WIDE LAYOUT, AT DESKTOP SIZE: what the owner's browser at a desk sees, where the phone's tests do not look.
 *
 * More drops a sheet whose links a press reaches. The secretary panel's toggle keeps one place, open or closed, and
 * a closed panel takes no width. The Todos page fits the window, so only its panes scroll. A login that may read and
 * not write, or a writer whose browser key the chain does not trust, is told so, in a status line where the composer
 * would be. The untrusted key's scene is last: it puts a key nobody trusted in this browser's place.
 */

import assert from "node:assert/strict";

import { BASE, DESKTOP, MIDDLE, suite, WIDE, WRITE_BASE } from "./harness";
import cast from "../test/cast";

const { todos } = cast.world;

const app = await suite("desktop", DESKTOP);

const more = `[...document.querySelectorAll(".tabbar .tab")].find((t) => t.textContent.trim() === "More")`;

type Box = { x: number; y: number; width: number; height: number };
const box = (selector: string): Promise<Box> =>
  app.page.evaluate<Box>(
    `(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; })()`,
  );

app.scene("More opens a sheet whose links are visible and take a press", async () => {
  await app.open(BASE);
  await app.page.clickNode(more);
  await app.page.waitFor(`document.querySelector(".moresheet") !== null && !document.querySelector(".moresheet").hidden`);
  assert.equal(await app.page.evaluate<string>(`${more}.getAttribute("aria-expanded")`), "true", "More says it is open");
  const links = await app.page.evaluate<{ text: string; shown: boolean }[]>(`[...document.querySelectorAll(".moresheet a")].map((a) => {
    const r = a.getBoundingClientRect();
    const at = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2);
    return { text: a.textContent.trim(), shown: r.height > 0 && r.bottom <= innerHeight && at !== null && a.contains(at) };
  })`);
  assert.ok(links.length > 0, "the sheet holds links");
  assert.deepEqual(links.filter((l) => !l.shown).map((l) => l.text), [], "every link is drawn where a press reaches it");
  await app.page.clickText(".moresheet a", "Mail");
  await app.page.waitFor(`location.hash.startsWith("#/mail")`);
  assert.equal(await app.page.evaluate<string | null>(`${more}?.getAttribute("aria-expanded") ?? null`), "false", "the sheet closes on the place it opened");
});

/**
 * ONE GRID FOR EVERY PAGE (the 2026-10-13 audit, X4): the brand and the words stand in the same place on every page,
 * a todo beside the list is the wider of the two, and a secretary panel with nothing about the page in it is closed to
 * its toggle until the person opens it. First in the file that touches the panel, so the person has said nothing yet.
 */
app.scene("at 1440 px the header holds still across pages, the todo read is the widest column, and an unused panel is closed", async () => {
  await app.resize(WIDE);
  const brands: Box[] = [];
  for (const [hash, ready] of [
    ["#/", `document.querySelectorAll(".tree .row").length > 0`],
    ["#/inbox", `document.querySelector("#view h1") !== null`],
    ["#/secretary/default", `document.querySelector("#view .pagetitle") !== null`],
    ["#/about", `document.querySelector(".aboutversion") !== null`],
  ] as const) {
    await app.page.navigate(`${BASE}/${hash}`);
    await app.page.waitFor(ready);
    brands.push(await box("header .brand"));
  }
  assert.deepEqual(brands.slice(1), brands.slice(0, -1), "the brand is where it was on every page");
  await app.page.navigate(`${BASE}/#/todo/${todos.flat.id}`);
  await app.page.waitFor(`document.querySelector(".detailpane .todoview .todobody") !== null`);
  await app.page.waitFor(`document.querySelector(".secretarypanel .frameheader")?.textContent.includes("Nothing about") === true`);
  assert.equal(await app.page.evaluate<string>(`document.querySelector(".panelcollapse").getAttribute("aria-expanded")`), "false", "nothing in it is about this todo");
  assert.equal((await box(".secretarypanel")).width, 0);
  const list = await box(".listpane");
  const pane = await box(".detailpane");
  assert.ok(pane.width > list.width, `the todo (${pane.width} px) is wider than the list (${list.width} px)`);
  assert.equal(list.x, brands[0]?.x, "the list starts under the brand");
  await app.resize(DESKTOP);
});

/**
 * AT A MIDDLE WIDTH WITH THE PANEL OPEN the todo is still the wider column: the list gives way first, down to 16rem,
 * so the thing being read keeps its room.
 */
app.scene("at 1100 px with the panel open, the todo is wider than the list", async () => {
  await app.resize(MIDDLE);
  await app.page.navigate(`${BASE}/#/todo/${todos.flat.id}`);
  await app.page.waitFor(`document.querySelector(".detailpane .todoview .todobody") !== null`);
  await app.page.waitFor(`document.querySelector(".secretarypanel .panelcollapse")?.textContent !== ""`);
  if ((await app.page.evaluate<string>(`document.querySelector(".panelcollapse").getAttribute("aria-expanded")`)) === "false") {
    await app.page.click(".panelcollapse");
  }
  await app.page.waitFor(`document.querySelector(".panelcollapse").getAttribute("aria-expanded") === "true"`);
  const list = await box(".listpane");
  const pane = await box(".detailpane");
  assert.ok(pane.width > list.width, `the todo (${pane.width} px) is wider than the list (${list.width} px)`);
  await app.page.click(".panelcollapse");
  await app.page.waitFor(`document.querySelector(".panelcollapse").getAttribute("aria-expanded") === "false"`);
  await app.resize(DESKTOP);
});

app.scene("the panel's toggle keeps its place, a closed panel takes no width, and the Todos page fits the window", async () => {
  await app.open(WRITE_BASE);
  await app.page.waitFor(`document.querySelector(".secretarypanel .panelcollapse") !== null`);
  if ((await app.page.evaluate<string>(`document.querySelector(".panelcollapse").getAttribute("aria-expanded")`)) === "false") {
    await app.page.click(".panelcollapse");
  }
  const open = await box(".panelcollapse");
  assert.ok((await box(".secretarypanel")).width > 0, "an open panel has its column");
  await app.page.click(".panelcollapse");
  await app.page.waitFor(`document.querySelector(".panelcollapse").getAttribute("aria-expanded") === "false"`);
  assert.deepEqual(await box(".panelcollapse"), open, "the toggle is where it was");
  assert.equal((await box(".secretarypanel")).width, 0, "a closed panel takes no width");
  assert.equal(await app.page.evaluate<boolean>(`document.activeElement === document.querySelector(".panelcollapse")`), true, "the toggle keeps the focus");
  const fits = `document.documentElement.scrollHeight <= innerHeight`;
  assert.equal(await app.page.evaluate<boolean>(fits), true, "closed, the page does not scroll; its panes do");
  await app.page.click(".panelcollapse");
  await app.page.waitFor(`document.querySelector(".panelcollapse").getAttribute("aria-expanded") === "true"`);
  assert.deepEqual(await box(".panelcollapse"), open, "and back where it was");
  assert.equal(await app.page.evaluate<boolean>(fits), true, "open, the page does not scroll either");
});

app.scene("a login that reads and does not write is told so where the composer would be, in the panel and on the page", async () => {
  await app.open(BASE);
  const line = (scope: string): string =>
    `document.querySelector(${JSON.stringify(`${scope} .readonly[role="status"]`)})?.textContent ?? ""`;
  await app.page.waitFor(`(${line(".secretarypanel")}) !== ""`);
  assert.match(await app.page.evaluate<string>(line(".secretarypanel")), /can read here and not write\. Write from/);
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll(".composebox").length`), 0, "no box that writes");
  await app.page.navigate(`${BASE}/#/secretary/default`);
  await app.page.waitFor(`(${line("#view")}) !== ""`);
  assert.match(await app.page.evaluate<string>(line("#view")), /e2e-reader@example\.invalid/);
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll(".composebox").length`), 0, "no box that writes");
});

app.scene("a writer whose browser key the chain does not trust is sent to Settings, in the panel and on the page", async () => {
  // A new key in this browser's place, as "Use a new key on this device" makes one, and never trusted at the terminal.
  await app.page.navigate(`${WRITE_BASE}/manifest.webmanifest`);
  await app.page.evaluate(`(async () => {
    const request = (r) => new Promise((ok, no) => { r.onsuccess = () => ok(r.result); r.onerror = () => no(r.error); });
    const opening = indexedDB.open("suzatary-keys", 1);
    opening.onupgradeneeded = () => opening.result.createObjectStore("keys");
    const db = await request(opening);
    const pair = await crypto.subtle.generateKey({ name: "Ed25519" }, false, ["sign", "verify"]);
    const raw = new Uint8Array(await crypto.subtle.exportKey("raw", pair.publicKey));
    const key = { public: Array.from(raw, (b) => b.toString(16).padStart(2, "0")).join(""), key: pair.privateKey };
    await request(db.transaction("keys", "readwrite").objectStore("keys").put(key, "device"));
    db.close();
    return true;
  })()`);
  await app.open(WRITE_BASE);
  const said = "This device is not trusted to write yet: trust it from Settings → This device with the code from the box, or ask there, then tap its card on a trusted device.";
  const line = (scope: string): string => `document.querySelector(${JSON.stringify(`${scope} .readonly[role="status"]`)})`;
  await app.page.waitFor(`(${line(".secretarypanel")})?.textContent.startsWith(${JSON.stringify(said)}) === true`, { timeout: 20000 });
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll(".secretarypanel .composebox").length`), 0, "no box whose every send the box refuses");
  assert.equal(await app.page.evaluate<string>(`${line(".secretarypanel")}.querySelector("a").getAttribute("href")`), "#/settings");
  await app.page.navigate(`${WRITE_BASE}/#/secretary/default`);
  await app.page.waitFor(`(${line("#view .compose")})?.textContent.startsWith(${JSON.stringify(said)}) === true`, { timeout: 20000 });
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll("#view .composebox").length`), 0, "nor on the page");
  await app.page.click(`#view .compose .readonly a`);
  await app.page.waitFor(`location.hash === "#/settings"`);
});
