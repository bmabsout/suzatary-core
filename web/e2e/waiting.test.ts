/**
 * WHAT A PERSON SEES WHILE THE APP WAITS, AND A CONVERSATION AT REST
 * (`docs/AUDIT-EXPERIENCE-2026-10-13.md` X5, X8, X14, X17, X21).
 *
 * Each scene holds what the page waits for and watches what the page draws: a
 * placeholder and never a sentence, a clock that moves while the box thinks, an
 * answer with its words, Reply and one "…", and one field above the
 * conversations. Every wait is on the page's own drawing.
 */

import assert from "node:assert/strict";

import { BASE, DESKTOP, PHONE, releaseModel, SIGNED_POST, suite, WRITE_BASE } from "./harness";
import cast from "../test/cast";

const { todos } = cast.world;

const app = await suite("waiting");

const WIDE: readonly [number, number] = [1440, 900];

/** The words the page shows a reader, its controls' labels included, outside the app's own chrome. */
const words = `document.getElementById("view").innerText`;

/** The nodes on screen: laid out, not see-through, and not inside a closed disclosure. */
const visible = (selector: string): string =>
  `[...document.querySelectorAll(${JSON.stringify(selector)})].filter((n) => n.checkVisibility({ opacityProperty: true, visibilityProperty: true }))`;

/** Every placeholder of a visible field whose text is wider than the room the field gives it. */
const cutPlaceholders = `(async () => {
  await document.fonts.ready;
  const ctx = document.createElement("canvas").getContext("2d");
  return [...document.querySelectorAll("#view input[placeholder], #view textarea[placeholder]")]
    .filter((i) => i.checkVisibility())
    .filter((i) => {
      const s = getComputedStyle(i);
      ctx.font = s.fontStyle + " " + s.fontWeight + " " + s.fontSize + " " + s.fontFamily;
      return ctx.measureText(i.placeholder).width > i.clientWidth - parseFloat(s.paddingLeft) - parseFloat(s.paddingRight) + 0.5;
    })
    .map((i) => i.placeholder);
})()`;

async function freshConversation(): Promise<string> {
  await app.open(WRITE_BASE);
  return app.page.evaluate<string>(
    `${SIGNED_POST}("/api/secretary/conversations", JSON.stringify({title: "Waiting"})).then((r) => r.json()).then((j) => j.id)`,
  );
}

async function converse(said: string): Promise<void> {
  const id = await freshConversation();
  await app.page.navigate(`${WRITE_BASE}/#/secretary/${id}`);
  await app.page.waitFor(`document.querySelector(".composebox") !== null`, { timeout: 8000 });
  await app.page.type(".composebox", said);
  await app.page.click(".composesend");
}

/** Hold what the Mail page asks the box, open it, and wait for the placeholder to show; the answer lets the box through. */
async function mailWaiting(): Promise<() => Promise<void>> {
  const release = await app.page.holdRequests(["/api/mail/"]);
  await app.page.evaluate(`location.hash = "#/mail"`);
  await app.page.waitFor(`document.querySelector(".mailpage .pagewait .placeholder.ph-rows") !== null`, { timeout: 8000 });
  await app.page.waitFor(`getComputedStyle(document.querySelector(".mailpage .placeholder")).visibility === "visible"`, { timeout: 8000 });
  return async () => {
    await release();
    await app.page.waitFor(`document.querySelector(".mailpage .maillist") !== null && document.querySelector(".mailpage .placeholder") === null`, { timeout: 15000 });
  };
}

app.scene("a page on its way shows its placeholder after 300 ms with no words, still with reduced motion, in both themes", async () => {
  await app.resize(PHONE);
  await app.open(WRITE_BASE);
  const done = await mailWaiting();
  try {
    assert.equal(
      await app.page.evaluate<string>(`getComputedStyle(document.querySelector(".mailpage .placeholder")).animationDelay`),
      "0.3s",
      "nothing shows for the first 300 ms",
    );
    assert.ok(!/load|reading|asking|fetching/i.test(await app.page.evaluate<string>(words)), "no sentence says it is loading");
    assert.notEqual(await app.page.evaluate<string>(`getComputedStyle(document.querySelector(".mailpage .loadfill")).animationName`), "none", "the mark moves");
    for (const theme of ["light", "dark"] as const) {
      await app.page.readerMedia(theme, "reduce");
      await app.page.waitFor(`matchMedia("(prefers-color-scheme: ${theme})").matches && matchMedia("(prefers-reduced-motion: reduce)").matches`);
      const seen = await app.page.evaluate<{ fill: string; bar: string; ground: string }>(`(() => {
        const s = (q) => getComputedStyle(document.querySelector(q));
        return { fill: s(".mailpage .loadfill").animationName, bar: s(".mailpage .phbar").backgroundColor, ground: getComputedStyle(document.body).backgroundColor };
      })()`);
      assert.equal(seen.fill, "none", `${theme}: with reduced motion the mark holds still`);
      assert.notEqual(seen.bar, seen.ground, `${theme}: the bars stand out from the ground`);
    }
  } finally {
    await done();
    await app.page.colorScheme(null);
  }
});

// The fixture's Inbox always owes its setup cards, so the page's wait hands over to cards here; the same node kept
// through a value that still waits is islands.test.ts's law.
app.scene("the Inbox never starts a second placeholder: its wait is the frame's one node, and its cards replace it while the box is asked", async () => {
  await app.resize(PHONE);
  await app.open(BASE);
  const release = await app.page.holdRequests(["/fold"]);
  try {
    await app.page.evaluate(`location.hash = "#/inbox"`);
    await app.page.waitFor(`document.querySelector(".inboxpage .placeholder") !== null || document.querySelector('.inboxpage .proposals[data-box="Asking"]') !== null`, { timeout: 8000 });
    await app.page.evaluate(`(document.querySelectorAll(".inboxpage .placeholder").forEach((n) => (n.dataset.seen = "first")), true)`);
    await app.page.waitFor(`document.querySelector('.inboxpage .proposals[data-box="Asking"]') !== null`, { timeout: 8000 });
    assert.deepEqual(
      await app.page.evaluate<string[]>(`[...document.querySelectorAll(".inboxpage .placeholder")].map((n) => n.dataset.seen ?? "new")`),
      [],
      "the cards the tab holds stand at once, and no new placeholder starts while the box is asked",
    );
    assert.ok((await app.page.evaluate<number>(`document.querySelectorAll(".inboxpage .proposal").length`)) > 0);
  } finally {
    await release();
  }
  await app.page.waitFor(`document.querySelector('.inboxpage .proposals[data-box="Answered"]') !== null`, { timeout: 15000 });
});

app.scene("a todo's title is never its id, however its page arrives", async () => {
  await app.resize(PHONE);
  await app.open(WRITE_BASE);
  await app.page.evaluate(`(() => {
    window.__titles = [];
    new MutationObserver(() => {
      const t = document.querySelector(".todoview .pagetitle");
      if (t !== null) window.__titles.push(t.textContent);
    }).observe(document.body, { subtree: true, childList: true, characterData: true });
    return true;
  })()`);
  const release = await app.page.holdRequests([`/todo/${todos.urgent.id}`]);
  await app.page.evaluate(`location.hash = "#/todo/${todos.urgent.id}"`);
  await app.page.waitFor(`document.querySelector(".todoview") !== null`, { timeout: 8000 });
  await release();
  await app.page.waitFor(`document.querySelector(".todoview .todobody") !== null`, { timeout: 15000 });
  const titles = await app.page.evaluate<string[]>(`window.__titles`);
  assert.ok(!titles.includes(todos.urgent.id), titles.join(" | "));
});

app.scene("a conversation on its way shows turns above the composer, at the bottom, never an empty page", async () => {
  await app.resize(PHONE);
  const id = await freshConversation();
  // Whatever the conversation is read through, it asks the box first; held, the page stays on its way.
  const release = await app.page.holdRequests(["/api/"]);
  try {
    await app.page.evaluate(`location.hash = "#/secretary/${id}"`);
    await app.page.waitFor(`document.querySelector(".transcriptcontent .placeholder.ph-turns") !== null && document.querySelector(".composebox") !== null`, { timeout: 8000 });
    await app.page.waitFor(`getComputedStyle(document.querySelector(".transcriptcontent .placeholder")).visibility === "visible"`, { timeout: 8000 });
    const at = await app.page.evaluate<{ placeholder: number; composer: number; height: number }>(`({
      placeholder: document.querySelector(".transcriptcontent .placeholder").getBoundingClientRect().bottom,
      composer: document.querySelector(".compose").getBoundingClientRect().top,
      height: innerHeight,
    })`);
    assert.ok(at.placeholder <= at.composer + 1, "the turns stand above the composer");
    assert.ok(at.composer > at.height / 2, "the composer is at the bottom, where a thumb is");
  } finally {
    await release();
  }
  await app.page.waitFor(`document.querySelector(".transcriptcontent .placeholder") === null`, { timeout: 15000 });
});

app.scene("a sent question reads Thinking with its count, beside Stop, and is never marked queued", async () => {
  await app.resize(PHONE);
  // The script answers this one once the scene lets it; the count moving each second is answer.test.ts's.
  await converse("take your time");
  await app.page.waitFor(`/^Thinking · \\d+ s$/.test(document.querySelector(".answerplace .answerstate")?.textContent ?? "")`, { timeout: 8000 });
  assert.equal(await app.page.evaluate<number>(`${visible(".answerplace .stopanswer")}.length`), 1, "Stop beside it");
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll(".turns .queuedmark, .turns .turn.queued").length`), 0);
  assert.ok(!/queued/i.test(await app.page.evaluate<string>(words)), "nothing says queued");
  releaseModel();
  await app.page.waitFor(`document.querySelector(".answerplace .answerstate") === null`, { timeout: 20000 });
});

app.scene("an answer at rest shows Reply and one menu, and the menu holds the copies, the source, Erase and the calls", async () => {
  await app.resize(PHONE);
  await converse("propose the harrow reply");
  // The round's own answer, never a model turn among its calls, once nothing runs.
  const answer = ".round > .turn-model";
  await app.page.waitFor(`document.querySelector("${answer} .turncontrols") !== null && document.querySelector(".answerplace .answerstate") === null`, { timeout: 20000 });
  const atRest = await app.page.evaluate<string[]>(
    `${visible(`${answer} button, ${answer} summary, .round > .roundfold`)}.map((n) => n.textContent.trim())`,
  );
  assert.deepEqual(atRest, ["Reply", "…"], "two controls, and the calls are behind the answer");
  await app.page.click(`${answer} .turnmorebutton`);
  await app.page.waitFor(`document.querySelector("${answer} > .turncontrols > .turnmore").open`);
  const inMenu = await app.page.evaluate<string[]>(`${visible(`${answer} > .turncontrols > .turnmore > .turnmenu > *`)}.map((n) => n.className)`);
  assert.ok(inMenu.includes("copyanswer") && inMenu.includes("turnsource") && inMenu.includes("turnerase"), inMenu.join(" "));
  assert.ok(inMenu.includes("roundfold"), "the round's calls are in the answer's menu");
  const asked = await app.page.evaluate<string[]>(`${visible(".turns .turn-person button, .turns .turn-person summary")}.map((n) => n.textContent.trim())`);
  assert.deepEqual(asked, ["Reply", "…"]);
});

for (const [size, label] of [
  [PHONE, "390"],
  [WIDE, "1440"],
] as const) {
  app.scene(`the conversations have one field above them, and no placeholder is cut, at ${label} px`, async () => {
    await app.resize(size);
    await app.open(WRITE_BASE);
    await app.page.evaluate(`location.hash = "#/secretary"`);
    await app.page.waitFor(`document.querySelector(".chats .chatrow") !== null`, { timeout: 15000 });
    assert.equal(await app.page.evaluate<number>(`${visible(".chats .chathead input")}.length`), 1, "the search, and no name field");
    assert.equal(await app.page.evaluate<number>(`${visible(".chats .chathead .act-newtopic")}.length`), 1);
    assert.deepEqual(await app.page.evaluate<string[]>(cutPlaceholders), []);
    const id = await app.page.evaluate<string>(`document.querySelector(".chats .chatrow").getAttribute("href").split("/").pop()`);
    await app.page.evaluate(`location.hash = "#/secretary/${id}"`);
    await app.page.waitFor(`document.querySelector(".composebox") !== null`, { timeout: 8000 });
    if (size === WIDE) await app.page.waitFor(`document.querySelector(".chatsaside .chathead input") !== null`, { timeout: 8000 });
    assert.deepEqual(await app.page.evaluate<string[]>(cutPlaceholders), []);
  });
}

app.scene("a page waiting on a search keeps its whole column: nothing is clipped, the search field inside it", async () => {
  await app.resize(PHONE);
  await app.open(WRITE_BASE);
  await app.page.evaluate(`location.hash = "#/secretary"`);
  await app.page.waitFor(`document.querySelector(".chats .chatrow") !== null`, { timeout: 15000 });
  await app.page.type(".chats .chatsearch", "deed");
  // A search waits at least its debounce, a state the column says.
  await app.page.waitFor(`document.querySelector(".column.chats.pagewaiting") !== null`, { timeout: 8000 });
  const seen = await app.page.evaluate<{ overflow: string; clipped: boolean; inside: boolean }>(`(() => {
    const column = document.querySelector(".column.chats");
    const field = column.querySelector(".chatsearch").getBoundingClientRect();
    const box = column.getBoundingClientRect();
    return {
      overflow: getComputedStyle(column).overflow,
      clipped: column.scrollHeight > column.clientHeight + 1,
      inside: field.top >= box.top - 1 && field.bottom <= box.bottom + 1,
    };
  })()`);
  assert.equal(seen.overflow, "visible");
  assert.equal(seen.clipped, false, "the column shows all it holds");
  assert.equal(seen.inside, true, "the search field stands inside the column");
  await app.page.waitFor(`document.querySelector(".column.chats.pagewaiting") === null`, { timeout: 15000 });
});

app.scene("New topic opens a conversation at once, named as new until the box names it", async () => {
  await app.resize(PHONE);
  await app.open(WRITE_BASE);
  await app.page.evaluate(`location.hash = "#/secretary"`);
  await app.page.waitFor(`document.querySelector(".chats .act-newtopic") !== null`, { timeout: 15000 });
  const was = await app.page.evaluate<string>(`location.hash`);
  await app.page.click(".chats .act-newtopic");
  await app.page.waitFor(`location.hash !== ${JSON.stringify(was)} && location.hash.startsWith("#/secretary/")`, { timeout: 8000 });
  await app.page.waitFor(`document.querySelector(".convhead .pagetitle")?.textContent === "New topic"`, { timeout: 15000 });
  assert.equal(await app.page.evaluate<boolean>(`document.activeElement === document.querySelector(".composebox")`), true, "ready to ask");
});

app.scene("the Typst list shows the list's placeholder until Typst has drawn it", async () => {
  await app.resize(DESKTOP);
  await app.open(WRITE_BASE);
  const release = await app.page.holdRequests([".wasm"]);
  try {
    await app.page.evaluate(`location.hash = "#/typst"`);
    await app.page.waitFor(`document.querySelector(".typstframe .placeholder.ph-rows") !== null`, { timeout: 8000 });
    assert.ok(!/loading Typst/i.test(await app.page.evaluate<string>(words)));
  } finally {
    await release();
  }
  await app.page.waitFor(`document.querySelector(".typstframe .placeholder") === null`, { timeout: 30000 });
});
