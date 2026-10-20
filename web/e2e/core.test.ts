/**
 * THE CORE RUNS IN A REAL BROWSER.
 *
 * `web/test/wasm.test.ts` drives the same `.wasm` under node and pins its
 * numbers against the conformance vectors. What it cannot see is whether
 * a TAB can get hold of it: the module is fetched from a URL beside the
 * bundle, instantiated through `WebAssembly.instantiateStreaming`, and reached
 * by a dynamic `import()` that `scripts/build-web.sh` deliberately leaves
 * unbundled. Every one of those is a place where a page would fall silently
 * back to `verify.ts` and still look green — the fallback exists precisely so
 * that nothing breaks, which is why only a browser can tell the two apart.
 *
 * So this asserts the one sentence that distinguishes them: the chain page's
 * "Checked by" line names the WebAssembly core. A run where the module failed
 * to load says the other sentence, and this test goes red.
 */

import assert from "node:assert/strict";

import { BASE, PHONE, WRITE_BASE, atTheTerminal, suite } from "./harness";

const app = await suite("core");

app.scene("the chain page's verdict comes from the WebAssembly core", async () => {
  await app.page.navigate(`${BASE}/#/chain`);
  await app.page.waitFor(`document.querySelector(".checker") !== null`, { timeout: 30000 });

  const checker = await app.page.evaluate<string>(`document.querySelector(".checker").textContent`);
  assert.match(checker, /Checked by the core itself, compiled to WebAssembly/, `the fallback ran: ${checker}`);
  assert.match(checker, /parsed as an envelope/);

  // And the verdict it produced is the clean one, over every object the server
  // sent: a "Checked by" line above an empty or failing table would name the
  // right checker and mean nothing.
  const verdict = await app.page.evaluate<string>(`document.querySelector(".verdict").textContent`);
  assert.match(verdict, /objects verified by this browser/, verdict);
  assert.equal(await app.page.evaluate<boolean>(`document.querySelector(".verdict").classList.contains("ok")`), true);

  // The rows are drawn a window at a time, so the table says how many it holds and each row its place among them.
  const rows = await app.page.evaluate<number>(`Number(document.querySelector("#chain").getAttribute("aria-rowcount")) - 1`);
  const objects = await app.page.evaluate<number>(
    `fetch("/api/chain").then((r) => r.json()).then((b) => b.links.length)`,
  );
  assert.equal(rows, objects, "the table does not hold one row per object");
  const drawn = await app.page.evaluate<number[]>(`[...document.querySelectorAll("#chain tbody tr[aria-rowindex]")].map((r) => Number(r.getAttribute("aria-rowindex")))`);
  assert.ok(drawn.length > 0 && drawn.length <= objects, `${drawn.length} rows drawn of ${objects}`);
  assert.deepEqual(drawn, drawn.map((_, i) => (drawn[0] ?? 0) + i), "the rows drawn are one run of the table, in order");
  assert.equal(await app.page.evaluate<number>(`document.querySelectorAll("#chain tbody tr.bad-row").length`), 0);

  // The causal order is the finding only the core has: the SubtleCrypto path
  // walks parents and never sorts, so this sentence is a second witness that
  // `linearise` ran here.
  const legend = await app.page.evaluate<string>(
    `[...document.querySelectorAll(".legend")].map((p) => p.textContent).join(" ")`,
  );
  assert.match(legend, new RegExp(`All ${objects} of them went into one causal order`), legend);
});

// The chain moves under the page every few minutes on the box, and the page draws its table again. What the reader did to
// the last one stands: the box stays where they scrolled it, and the query typed into the filter still filters.
app.scene("a chain that moves under a scrolled table, then a filtered one, keeps the place and the filter", async () => {
  await app.page.viewport(PHONE[0], 320);
  try {
    await app.page.navigate(`${WRITE_BASE}/#/chain`);
    await app.page.waitFor(`document.querySelector(".checker") !== null && document.querySelector("#chain tbody tr[aria-rowindex]") !== null`, { timeout: 30000 });
    const objects = () => app.page.evaluate<number>(`Number(document.querySelector("#chain").getAttribute("aria-rowcount")) - 1`);
    const box = `document.querySelector(".chainpage .scroller")`;
    const moved = async (id: string, past: number): Promise<number> => {
      atTheTerminal(["todo", "add", id, "--body", "Added under the chain page", "--rationale", "the chain moves under the page", "--category", "home", "--priority", "60"]);
      await app.page.waitFor(`Number(document.querySelector("#chain").getAttribute("aria-rowcount")) - 1 > ${past}`, { timeout: 30000 });
      return objects();
    };

    const before = await objects();
    const scrolled = await app.page.evaluate<number>(`(() => { const b = ${box}; b.scrollTop = 60; return b.scrollTop })()`);
    assert.ok(scrolled > 0, "the box scrolls at this height");
    await app.page.waitFor(`new Promise((ok) => setTimeout(() => ok(true), 200))`);
    const after = await moved("chainmovedunderascroll", before);
    await app.page.waitFor(`${box}.scrollTop === ${scrolled}`, { timeout: 10000 });

    // The query is a todo the table holds and the next todo's id does not contain.
    const query = await app.page.evaluate<string>(`[...document.querySelectorAll("#chain tbody tr[aria-rowindex]")].map((r) => r.dataset.todo).find((t) => t !== "") ?? ""`);
    assert.notEqual(query, "", "a drawn row names a todo");
    assert.ok(!"chainmovedunderafilter".includes(query), query);
    const drawn = `document.querySelectorAll("#chain tbody tr[aria-rowindex]").length`;
    const matching = `[...document.querySelectorAll("#chain tbody tr[aria-rowindex]")].filter((r) => r.textContent.toLowerCase().includes(${JSON.stringify(query.toLowerCase())})).length`;
    await app.page.type(".chainpage .filterbox input", query);
    await app.page.waitFor(`${drawn} > 0 && ${drawn} === ${matching}`, { timeout: 10000 });
    const filtered = await app.page.evaluate<number>(drawn);
    assert.ok(filtered < after, `${filtered} of ${after} rows under the filter`);
    await moved("chainmovedunderafilter", after);
    assert.equal(await app.page.evaluate<string>(`document.querySelector(".chainpage .filterbox input").value`), query, "the query is carried");
    assert.equal(await app.page.evaluate<number>(drawn), await app.page.evaluate<number>(matching), "every row drawn is the filter's");
    assert.equal(await app.page.evaluate<number>(drawn), filtered, "the new table's rows are the filter's");
  } finally {
    await app.page.viewport(PHONE[0], PHONE[1]);
  }
});
