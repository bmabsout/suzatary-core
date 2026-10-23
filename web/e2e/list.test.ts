/**
 * The list, and what a line opens.
 *
 * This is the app's front door and the one screen that has to be right: the
 * open todos as one tree of lines, grouped by category, ASCENDING in value at
 * every level because low means urgent, the agent's todo marked where it
 * stands rather than sunk to the bottom, and a line that opens in place and
 * then the todo — a pushed screen on a phone, the pane beside the list on a
 * desktop, from the same `todoview.ts`.
 *
 * The ordering and every group's value are checked against `/api/tree`
 * FETCHED BY THE PAGE rather than against numbers written down here: the page
 * builds its tree from the store with the core in the tab, and the claim is
 * that it builds the SAME tree the box does.
 */

import assert from "node:assert/strict";

import { BASE, DESKTOP, PHONE, WRITE_BASE, fold, openInPlace, openTodo, row, shownTodos, suite, pickCategory, openFinder } from "./harness";
import type { Worth } from "../src/wire/generated/wire";
import cast from "../test/cast";
import fulfillment from "../src/todo/fulfillment";

const { todos } = cast.world;

const app = await suite("list");

interface Node {
  kind: "group" | "todo";
  label?: string;
  todo?: string;
  worth: Worth;
  children: Node[];
}

/** A worth's number, or null for `∅` and for a term that does not link: the page prints the same. */
const numberOf = (worth: Worth): number | null => (typeof worth === "object" && "value" in worth ? worth.value : null);

/** `/api/tree` at the instant the page priced its own, fetched by the page. */
async function served(by: string): Promise<Node> {
  await app.page.waitFor(`document.querySelector(".tree")?.getAttribute("data-at") !== ""`, { timeout: 30000 });
  const answer = await app.page.evaluate<string>(
    `fetch("/api/tree?by=${by}&t=" + document.querySelector(".tree").getAttribute("data-at")).then((r) => r.text())`,
  );
  return (JSON.parse(answer) as { root: Node }).root;
}

const todosIn = (n: Node): string[] => [...(n.todo === undefined ? [] : [n.todo]), ...n.children.flatMap(todosIn)];

app.scene("the page's groups are the box's tree: the same groups, order and values, in all three groupings", async () => {
  await app.open(BASE);
  for (const by of ["category", "urgency", "flat"]) {
    await openFinder(app.page);
    await app.page.click(`.pill[data-by="${by}"]`);
    await app.page.waitFor(`document.querySelector('.pill[data-by="${by}"]').getAttribute("aria-pressed") === "true"`);
    const root = await served(by);
    const cards = by === "flat" ? [{ ...root, label: "all" }] : root.children;
    const shown = await app.page.evaluate<[string, number | null][]>(
      `[...document.querySelectorAll(".tree section.group")].map((s) => [s.getAttribute("data-group"), JSON.parse(s.getAttribute("data-worth"))])`,
    );
    assert.equal(shown.length, cards.length, `${by}: one card per group`);
    cards.forEach((card, i) => {
      const [key, value] = shown[i] ?? ["", NaN];
      const boxed = numberOf(card.worth);
      assert.ok(key.endsWith(by === "flat" ? "all" : (card.label ?? "")), `${by}: card ${i} is ${key}, the box says ${card.label}`);
      // The page draws the box's number as the screen observes it, the whole percent.
      assert.equal(value, boxed === null ? null : fulfillment.whole(boxed), `${by}: ${key} is worth ${value} here and ${boxed} on the box`);
    });
    // A group nobody has folded is open exactly when it has a number short of Fine, and then its lines show in the box's order.
    assert.deepEqual(
      await shownTodos(app.page),
      cards.flatMap((card) => {
        const boxed = numberOf(card.worth);
        const value = boxed === null ? null : fulfillment.whole(boxed);
        return value !== null && value < 0.7 ? todosIn(card) : [];
      }),
      `${by}: the lines, in the box's order`,
    );
  }
  await openFinder(app.page);
  await app.page.click(`.pill[data-by="category"]`);
});

app.scene("a todo nothing prices draws as unpriced, after every priced line", async () => {
  await app.open(BASE);
  const { id, category } = cast.world.unpriced;
  const entry = (await fold(app.page)).find((candidate) => candidate.todo === id);
  assert.equal(entry?.value, "absent", "the box reads it as no number at all");
  // Its group has no number either, so it sorts after every priced one and is folded to a peek of its line.
  const marks = await app.page.evaluate<string>(`JSON.stringify([
    [...document.querySelectorAll(".tree section.group")].at(-1)?.getAttribute("data-group"),
    ...['.grouphead[data-key="c:${category}"] > .mark', '.peek[data-todo="${id}"] > .mark'].map((at) =>
      [...(document.querySelector(at)?.children ?? [])].map((child) => [child.localName, child.textContent])),
  ])`);
  assert.equal(
    marks,
    JSON.stringify([`c:${category}`, [["span", "unpriced"]], [["span", "unpriced"]]]),
    "the last group, and its mark and its line's are the word, with no pie",
  );
});

app.scene("on a phone the first line starts in the top third, under one control that searches, filters and groups", async () => {
  await app.open(BASE);
  // The finder is page state, shut whenever the list is drawn anew.
  await app.page.reload();
  await app.page.waitFor(`document.querySelectorAll(".tree .row").length > 0`);
  const top = await app.page.evaluate<number>(`document.querySelector(".tree .row").getBoundingClientRect().top`);
  assert.ok(top < PHONE[1] / 3, `the first line starts at y = ${top} of ${PHONE[1]}`);
  assert.equal(await app.page.evaluate<boolean>(`document.querySelector(".finder").hidden`), true, "the search and the grouping wait behind the magnifier");
  assert.equal(await app.page.evaluate<string>(`document.querySelector(".findsaid").textContent`), "", "the default filter says nothing");
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll(".filtersummary, .filterdrawer").length`), 0);

  // The control lives in the title row and holds the search, the filters and the grouping.
  assert.equal(await app.page.evaluate<boolean>(`document.querySelector(".pagehead .titlerow .findbtn") !== null`), true);
  await openFinder(app.page);
  assert.equal(
    await app.page.evaluate<boolean>(`[".search", ".attentionbtn", ".pill[data-by]"].every((q) => document.querySelector(".finder " + q) !== null)`),
    true,
  );
  // Shut over a narrowed list, it says what narrows it.
  await app.page.clickText(".finder .segmented .segment", "Done");
  await app.page.click(".findbtn");
  await app.page.waitFor(`document.querySelector(".finder").hidden === true`);
  assert.equal(await app.page.evaluate<string>(`document.querySelector(".findsaid").textContent`), "Done");
  await openFinder(app.page);
  await app.page.clickText(".finder .segmented .segment", "Open");
  await app.page.waitFor(`document.querySelector(".findsaid").textContent === ""`);
});

app.scene("every line draws its pie alone, and names its whole percent on hover", async () => {
  await app.open(BASE);
  const marks = await app.page.evaluate<[string, number, string, string][]>(
    `[...document.querySelectorAll(".tree .row, .tree .grouphead")].filter((n) => n.querySelector(".mark .pie") !== null).map((n) => { const m = n.querySelector(".mark"); return [n.getAttribute("data-todo") ?? n.getAttribute("data-key"), Number(n.closest("[data-worth]").getAttribute("data-worth")), m.title, m.textContent] })`,
  );
  assert.ok(marks.length > 0, "the list has priced lines");
  for (const [key, worth, title, text] of marks) {
    assert.equal(title, `${Math.round(worth * 100)}%`, key);
    assert.equal(text, "", `${key}: no number is printed beside the pie`);
  }
});

app.scene("an unconfirmed line reads as words, and its page still shows it as written", async () => {
  await app.open(BASE);
  const { id, body } = todos.agents;
  const text = await app.page.evaluate<string>(
    `fetch("/api/fold").then((r) => r.json()).then((b) => b.entries.find((e) => e.todo === ${JSON.stringify(id)})?.authored?.text ?? "")`,
  );
  assert.ok(text !== "" && !/[*_#\\[\]]/.test(text), `the core's words: ${text}`);
  const line = await app.page.evaluate<{ title: string; chip: string; tip: string }>(`(() => {
    const r = document.querySelector('${row(id)}');
    const c = r.querySelector(".chip.prov");
    return { title: r.querySelector(".rowtitle").textContent, chip: c.textContent, tip: c.title };
  })()`);
  assert.equal(line.title, text, "the line is the words, with no markup character");
  assert.equal(line.chip, "unconfirmed", "and is still marked");
  assert.match(line.tip, /confirm/, line.tip);
  // The rule stands: nothing renders it, and its own page shows the source until it is confirmed.
  await openTodo(app.page, id);
  await app.page.waitFor(`document.querySelector(".todoview .todohead .todobody") !== null`, { timeout: 8000 });
  assert.equal(await app.page.evaluate<string>(`document.querySelector(".todoview .todohead .todobody").textContent`), body);
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll(".todoview .todohead .todobody strong").length`), 0);
});

app.scene("a line opened in place does not print its title a second time", async () => {
  await app.open(BASE);
  const { id } = todos.urgent;
  await openInPlace(app.page, id);
  const [title, panel] = await app.page.evaluate<[string, string]>(
    `[document.querySelector('${row(id)} .rowtitle').textContent, document.querySelector('.rowopen[data-todo="${id}"]').textContent]`,
  );
  assert.ok(title !== "" && !panel.includes(title), `the panel repeats the title: ${panel}`);
  await app.page.click(`${row(id)} .rowmain`);
  await app.page.waitFor(`document.querySelector('.rowopen[data-todo="${id}"]') === null`);
});

app.scene("the add sheet starts in a category, marks the chips while none is lit, and says a refusal beside its field", async () => {
  await app.open(WRITE_BASE);
  await app.page.evaluate(`localStorage.removeItem("suzatary.ui.todos.category/")`);
  await app.page.reload();
  await app.page.waitFor(`document.querySelectorAll(".tree .row").length > 0`);
  await app.page.click(".addsheet .pickerhandle");
  await app.page.waitFor(`document.querySelector(".addsheet.open") !== null`);
  // Nothing was used before and the list is not narrowed: the chips say what they ask for, in place.
  await app.page.waitFor(`document.querySelector(".addsheet .chipprompt")?.hidden === false`);
  assert.equal(await app.page.evaluate<string>(`document.querySelector(".addsheet .chipprompt").textContent`), "Pick a category");

  // An empty line is refused beside the line, a missing category beside the chips, and neither under the tab bar.
  const refusals = `[...document.querySelectorAll(".addsheet .formrefusal")].filter((p) => !p.hidden)`;
  const said = async (): Promise<{ text: string; after: string; bottom: number; bar: number }> =>
    app.page.evaluate(`(() => {
      const p = ${refusals}[0];
      const before = p.previousElementSibling;
      const after = before?.matches(".chipfield") ? "chips" : before?.matches(".draftbody") || before?.querySelector(".draftbody") ? "line" : before?.className ?? "";
      return { text: p.textContent, after, bottom: p.getBoundingClientRect().bottom, bar: document.querySelector("#tabbar").getBoundingClientRect().top };
    })()`);
  await app.page.click(".addbtn");
  await app.page.waitFor(`${refusals}.length === 1`);
  const body = await said();
  assert.equal(body.text, "Say what the todo is.");
  assert.equal(body.after, "line", "beside the line");
  assert.ok(body.bottom <= body.bar, `at y = ${body.bottom}, under the tab bar at ${body.bar}`);
  await app.page.type(".addsheet .draftbody", "Renew the parking permit");
  await app.page.click(".addbtn");
  await app.page.waitFor(`${refusals}[0]?.textContent === "Pick a category."`);
  const category = await said();
  assert.equal(category.after, "chips", "beside the chips");
  assert.ok(category.bottom <= category.bar, `at y = ${category.bottom}, under the tab bar at ${category.bar}`);

  // Added in one, the next starts in it.
  await pickCategory(app.page, "home");
  await app.page.click(".addbtn");
  await app.page.waitFor(`document.querySelector('${row("renewtheparkingpermit")}') !== null`, { timeout: 8000 });
  await app.page.waitFor(`document.querySelector(".addsheet.open") === null`);
  await app.page.click(".addsheet .pickerhandle");
  await app.page.waitFor(`document.querySelector(".addsheet.open") !== null`);
  const lit = `[...document.querySelectorAll(".addsheet .chiprow .chipbtn")].filter((b) => b.getAttribute("aria-pressed") === "true").map((b) => b.textContent)`;
  assert.deepEqual(await app.page.evaluate<string[]>(lit), ["home"]);
  assert.equal(await app.page.evaluate<boolean>(`document.querySelector(".addsheet .chipprompt").hidden`), true);
  await app.page.click(".addsheet .pickerhandle");
  await app.page.waitFor(`document.querySelector(".addsheet.open") === null`);
});

app.scene("the header counts what needs attention and links to what waits in the Inbox", async () => {
  await app.open(BASE);
  const said = await app.page.evaluate<string>(`document.querySelector(".needs").textContent`);
  assert.match(said, /^1 needs attention/, said);
  assert.equal(await app.page.evaluate<string>(`document.querySelector(".needs a.inboxlink").getAttribute("href")`), "#/inbox");
});

app.scene("a line opens in place, and Open pushes the todo on a phone", async () => {
  await app.open(BASE);
  await openInPlace(app.page, todos.urgent.id);
  assert.equal(await app.page.evaluate<string>(`document.querySelector('${row(todos.urgent.id)} .rowmain').getAttribute("aria-expanded")`), "true");
  const acts = await app.page.evaluate<string[]>(
    `[...document.querySelectorAll('.rowopen[data-todo="${todos.urgent.id}"] .rowacts > *')].map((n) => n.textContent.trim())`,
  );
  assert.deepEqual(acts, ["Open", "Ask"], "a viewer is offered no acts, only the way in and the question");
  await openTodo(app.page, todos.urgent.id);
  await app.page.waitFor(`document.querySelector(".todoview .todoid")?.textContent === "${todos.urgent.id}"`);
  // A pushed screen, not a pane: at 390 the list is gone and the back button is there.
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll(".tree .row").length`), 0);
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll(".todoview .back").length`), 1);
});

app.scene("a fold is the viewer's and stays on this device", async () => {
  await app.open(BASE);
  await app.page.click(`.tree section.group[data-group="c:home"] .grouphead`);
  await app.page.waitFor(`document.querySelector('${row(todos.flat.id)}') === null`);
  const peek = await app.page.evaluate<string>(`document.querySelector('section.group[data-group="c:home"] .peek').textContent`);
  assert.match(peek, new RegExp(todos.flat.body), "a folded group keeps its most urgent todo in sight");
  await app.page.reload();
  await app.page.waitFor(`document.querySelectorAll(".tree .row").length > 0`);
  assert.equal(await app.page.evaluate<boolean>(`document.querySelector('${row(todos.flat.id)}') === null`), true, "the fold came back");
  await app.page.click(`.tree section.group[data-group="c:home"] .grouphead`);
  await app.page.waitFor(`document.querySelector('${row(todos.flat.id)}') !== null`);
});

app.scene("a trusted body is rendered by typst", async () => {
  await app.page.navigate(`${BASE}/#/todo/${todos.urgent.id}`);
  await app.page.waitFor(`document.querySelector(".todobody") !== null`);
  const strong = await app.page.evaluate<string | null>(
    `document.querySelector(".todobody.rich strong")?.textContent ?? null`,
  );
  assert.equal(strong, cast.world.todos.urgent.strong, "the body should arrive as typst's HTML, through web/src/rich.ts");
});

app.scene("an unconfirmed body is shown as source, not rendered", async () => {
  await app.page.navigate(`${BASE}/#/todo/${todos.agents.id}`);
  await app.page.waitFor(`document.querySelector(".todobody") !== null`);
  const text = await app.page.evaluate<string>(`document.querySelector(".todobody.assource").textContent`);
  assert.match(text, new RegExp(`\\*${todos.agents.strong}\\*`), "rendered means confirmed — the asterisks must still be there");
  assert.equal(
    await app.page.evaluate<number>(`document.querySelectorAll(".todobody strong").length`),
    0,
    "an untrusted actor's markup must not become elements",
  );
  const note = await app.page.evaluate<string>(
    `[...document.querySelectorAll(".todoview .legend")].map((e) => e.textContent).join(" ")`,
  );
  assert.match(note, /until you confirm it/);
});

app.scene("at 1280 the same hash is the pane beside the list", async () => {
  await app.resize(DESKTOP);
  await app.open(BASE);
  await app.page.click(`${row(todos.flat.id)} .rowmain`);
  await app.page.waitFor(`window.location.hash === "#/todo/${todos.flat.id}"`);
  await app.page.waitFor(`document.querySelector(".detailpane .todoview .todoid")?.textContent === "${todos.flat.id}"`);
  // The list is still there, still keyed, and the line it opened is marked.
  assert.ok((await shownTodos(app.page)).includes(todos.flat.id));
  assert.equal(
    await app.page.evaluate<string | null>(`document.querySelector(".tree .row.selected")?.getAttribute("data-todo") ?? null`),
    todos.flat.id,
  );
  const [list, pane] = await app.page.evaluate<[number, number]>(
    `[document.querySelector(".listpane").getBoundingClientRect().width, document.querySelector(".detailpane").getBoundingClientRect().width]`,
  );
  // The todo is what is read, so it is the wider column, and the list keeps its 27rem (the 2026-10-13 audit, X4).
  assert.ok(pane > list, `the todo is the wider column: the list took ${list} of ${list + pane}`);
  assert.ok(list <= 27 * 16 + 1, `the list keeps at most 27rem, it took ${list}`);
  await app.resize(PHONE);
});

/**
 * THE FILTER ROW (2026-09-10): each control is on the page, narrows the list the
 * page draws, and turning it off puts everything back — a filter that could not
 * be undone would be a list somebody loses.
 */
app.scene("each filter button narrows the list to its own todos, and off puts the whole list back", async () => {
  await app.open(BASE);
  const everything = await shownTodos(app.page);
  for (const [button, only] of [
    [".attentionbtn", [todos.agents.id]],
    [".datedbtn", [todos.urgent.id]],
  ] as const) {
    // OPEN, not toggle: scenes share one page, and a drawer left open by the
    // previous scene would be closed by a click on its summary.
    await openFinder(app.page);
    await app.page.click(button);
    await app.page.waitFor(`document.querySelector("${button}").getAttribute("aria-pressed") === "true"`);
    assert.deepEqual(await shownTodos(app.page), only, button);
    await app.page.click(button);
    await app.page.waitFor(`document.querySelector("${button}").getAttribute("aria-pressed") === "false"`);
    assert.deepEqual(await shownTodos(app.page), everything, `${button}: turning it off must put the whole list back`);
  }
});

app.scene("the filter is remembered, and comes back parsed rather than trusted", async () => {
  await app.open(BASE);
  await openFinder(app.page);
  await app.page.click(".attentionbtn");
  await app.page.waitFor(`document.querySelectorAll(".tree .row").length === 1`);
  await app.page.reload();
  await app.page.waitFor(`document.querySelector(".attentionbtn")?.getAttribute("aria-pressed") === "true"`);
  await app.page.waitFor(`document.querySelectorAll(".tree .row").length === 1`);
  assert.deepEqual(await shownTodos(app.page), [todos.agents.id]);

  // A hand-edited value degrades to the default rather than breaking the page
  // (`register.stored`, then `todos.parseFilter`), which is the property the remembered filter rests on.
  await app.page.evaluate(`localStorage.setItem("suzatary.ui.todos.filter/", "{not json")`);
  await app.page.reload();
  await app.page.waitFor(`document.querySelectorAll(".tree .row").length > 1`);
  assert.equal(
    await app.page.evaluate<string | null>(`document.querySelector(".attentionbtn").getAttribute("aria-pressed")`),
    "false",
  );
});

/**
 * THE WAITING FILTER (`docs/APP.md` §4 item 5, chain item `listfilters`), over
 * a `waiting_on` this scene writes through the edit form, since no fixture todo
 * carries one.
 */
app.scene("the waiting filter narrows to what carries a waiting_on", async () => {
  await app.open(WRITE_BASE);
  const before = await shownTodos(app.page);
  assert.ok(!before.includes("blockedonharrow"), "a fresh id, so nothing else in this suite can have claimed it");

  // No fixture todo carries a `waiting_on`, so this scene writes one through
  // the edit form (`docs/APP.md` §4 item 2) the way `writes.test.ts` does.
  await app.page.click(".addsheet .pickerhandle");
  await app.page.waitFor(`document.querySelector(".addsheet.open") !== null`);
  await app.page.waitFor(`document.activeElement?.className.includes("draftbody") === true`);
  await app.page.type(".draftbody", "Blocked on Harrow");
  await pickCategory(app.page, "study");
  await app.page.click(".addbtn");
  const id = "blockedonharrow";
  await app.page.waitFor(`document.querySelector('${row(id)}') !== null`, { timeout: 8000 });
  await app.page.waitFor(`document.querySelector(".addsheet.open") === null`, { timeout: 2000 });

  await openTodo(app.page, id);
  await app.page.waitFor(`document.querySelector(".editbox summary") !== null`, { timeout: 4000 });
  await app.page.click(".editbox summary");
  await app.page.waitFor(`document.querySelector(".editbox")?.open === true`, { timeout: 2000 });
  await app.page.waitFor(`document.querySelector(".editbox .draftbody") !== null`, { timeout: 8000 });
  await app.page.type('.editbox input[aria-label="what it is blocked on — empty clears it"]', "counsel");
  await app.page.click(".editbox .addbtn");
  await app.page.waitFor(`document.querySelector(".todoview .waitingbody")?.textContent?.includes("counsel") === true`, {
    timeout: 4000,
  });
  await app.page.click(".todoview .back");
  await app.page.waitFor(`document.querySelector('${row(id)}') !== null`, { timeout: 4000 });
  await openInPlace(app.page, id);
  await app.page.waitFor(`document.querySelector('.rowopen[data-todo="${id}"] .waiting')?.textContent?.includes("counsel") === true`, {
    timeout: 4000,
  });

  const everything = await shownTodos(app.page);
  await openFinder(app.page);
  await app.page.click(".waitingbtn");
  await app.page.waitFor(`document.querySelector(".waitingbtn").getAttribute("aria-pressed") === "true"`);
  const narrowed = await shownTodos(app.page);
  assert.deepEqual(narrowed, [id], `only the todo carrying a waiting_on should remain, got ${narrowed.join(", ")}`);

  await app.page.click(".waitingbtn");
  await app.page.waitFor(`document.querySelector(".waitingbtn").getAttribute("aria-pressed") === "false"`);
  assert.deepEqual(await shownTodos(app.page), everything, "turning it off must put the whole list back");
});

/**
 * A SUBTODO FROM THE LIST (docs/TODOTREE-PLAN-2026-09-25.md step 5): the add
 * sheet opens under the line, the box authors the child and the parent's
 * revision, this replica seals both, and the child is drawn under its parent.
 */
app.scene("Add a subtodo puts the new todo under its parent", async () => {
  await app.open(WRITE_BASE);
  await openInPlace(app.page, todos.checklist.id);
  await app.page.waitFor(`document.querySelector('.rowopen[data-todo="${todos.checklist.id}"] .addsub') !== null`, { timeout: 8000 });
  await app.page.click(`.rowopen[data-todo="${todos.checklist.id}"] .addsub`);
  await app.page.waitFor(`document.querySelector(".addsheet.open .subof")?.hidden === false`);
  assert.match(await app.page.evaluate<string>(`document.querySelector(".subof").textContent`), new RegExp(`A subtodo of ${todos.checklist.body}`));
  await app.page.waitFor(`document.activeElement?.className.includes("draftbody") === true`);
  await app.page.type(".draftbody", "Buy a travel adapter");
  await app.page.click(".addbtn");
  const child = "buyatraveladapter";
  await app.page.waitFor(`document.querySelector('${row(todos.checklist.id)} .rowcount')?.textContent === "1"`, { timeout: 10000 });
  // The parent is still Fine, so its subtodos start folded, like any Fine fold nobody has touched.
  const fold = `document.querySelector('${row(todos.checklist.id)} .rowfold')`;
  if ((await app.page.evaluate<string>(`${fold}.getAttribute("aria-expanded")`)) === "false") {
    await app.page.click(`${row(todos.checklist.id)} .rowfold`);
  }
  await app.page.waitFor(`document.querySelector('.tree .row[data-key$="/t:${todos.checklist.id}/t:${child}"]') !== null`, { timeout: 10000 });
  const shown = await shownTodos(app.page);
  assert.equal(shown.indexOf(child), shown.indexOf(todos.checklist.id) + 1, "the child's line is right under its parent");
});

/**
 * A LINE'S FULFILLMENT IS ITS PIE, THE NUMBER ITS LABEL (docs/DESIGN-DECISIONS-2026-09-25.md
 * §F; the number left the line on 2026-09-28): the pie's filled share and its label are the server's value, and its
 * colour is the value's own, so different values are different colours.
 */
app.scene("a line's pie fills to its number, in its value's colour", async () => {
  await app.open(BASE);
  await app.page.waitFor(`document.querySelectorAll(".tree .row svg.pie").length >= 3`, { timeout: 15000 });
  const values = new Map((await fold(app.page)).map((e) => [e.todo, e.value]));
  const pies = await app.page.evaluate<[string, string, number, string, string][]>(
    `[...document.querySelectorAll(".tree .row")].filter((row) => row.querySelector("svg.pie") !== null).map((row) => [
        row.getAttribute("data-todo"),
        row.querySelector("svg.pie").getAttribute("aria-label"),
        Number(row.querySelector("svg.pie .piefill").getAttribute("stroke-dasharray").split(" ")[0]),
        getComputedStyle(row.querySelector("svg.pie .piefill")).stroke,
        row.querySelector("svg.pie").getAttribute("width"),
      ])`,
  );
  assert.ok(pies.length >= 3, `at least three drawn pies, got ${pies.length}`);
  for (const [todo, said, share, stroke, width] of pies) {
    const value = values.get(todo) ?? null;
    // A pie is drawn only for a number. The fold is a function of the clock, so the two reads are a moment apart.
    assert.ok(typeof value === "number", `${todo} has a pie, so its fulfillment is a number, not ${value}`);
    assert.ok(Math.abs(share - value) <= 0.02, `${todo}'s pie fills ${share}, its fulfillment is ${value}`);
    assert.ok(Math.abs(Number(said.replace("% fulfilled", "")) - value * 100) <= 2, `${todo} says ${said}, its fulfillment is ${value}`);
    assert.notEqual(stroke, "none", `${todo}'s pie is coloured`);
    assert.equal(width, "14", `${todo}'s pie is small`);
  }
  const byValue = [...new Map(pies.map(([todo, , , stroke]) => [values.get(todo), stroke])).entries()];
  assert.equal(new Set(byValue.map(([, stroke]) => stroke)).size, byValue.length, "different values are different colours");
  assert.ok(
    (await app.page.evaluate<number>(`document.querySelectorAll(".tree .grouphead svg.pie").length`)) > 0,
    "a group header draws its composed value's pie",
  );
});


/**
 * THE THIRTY DAYS' DATES ARE SHORT AND NEVER TOUCH (docs/DESIGN-DECISIONS-2026-09-25.md
 * §F): on a phone every date the line shows is the locale's short day, and no
 * two labels' boxes overlap.
 */
app.scene("at phone width the thirty days' dates are short and no two overlap", async () => {
  await app.resize(PHONE);
  await app.page.navigate(`${BASE}/#/todo/${todos.urgent.id}`);
  await app.page.waitFor(`document.querySelector(".curvebox .curvedates") !== null`, { timeout: 15000 });
  const labels = await app.page.evaluate<[string, number, number][]>(
    `[...document.querySelectorAll(".curvedates .curvedate")]
       .filter((l) => getComputedStyle(l).display !== "none")
       .map((l) => { const r = l.getBoundingClientRect(); return [l.textContent, r.left, r.right]; })`,
  );
  assert.ok(labels.length >= 3, `the start, now and the end at least, got ${JSON.stringify(labels)}`);
  for (const [said] of labels) assert.equal(/\d{4}/.test(said), false, `${said} is short`);
  const sorted = [...labels].sort((a, b) => a[1] - b[1]);
  sorted.slice(1).forEach(([said, left], i) => {
    const [before, , right] = sorted[i] ?? ["", 0, 0];
    assert.ok(right <= left, `${before} ends at ${right}, ${said} starts at ${left}`);
  });
});
