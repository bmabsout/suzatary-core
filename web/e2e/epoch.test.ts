/**
 * A DEVICE HOLDING A LINE THE BOX NO LONGER HOLDS: the box answers the lines in a later epoch, and a tab that kept a
 * line the box replaced, or a piece of a pull from before, drops its copy and takes the store again whole, so no
 * replaced line stays a head. Its own server and its own file, so the reset it makes reaches no other scene's origin or
 * state.
 */

import assert from "node:assert/strict"
import { createHash } from "node:crypto"

import { PHONE, SIGNED_POST, suite, trustThisDevice } from "./harness"

const app = await suite("epoch")

const required = (name: string): string => {
  const value = process.env[name]
  if (value === undefined || value === "") throw new Error(`${name} is unset — scripts/web-e2e.sh starts the epoch's server`)
  return value
}
const EPOCH_BASE = required("E2E_BASE_EPOCH")
const EPOCH_INSTANCE = required("E2E_EPOCH_INSTANCE")

/** Every row of every store of the database `name`, as one string: what the device holds of it. */
const held = (name: string): string => `new Promise((ok, no) => {
  const r = indexedDB.open(${JSON.stringify(name)})
  r.onerror = () => no(String(r.error))
  r.onsuccess = () => {
    const db = r.result
    const stores = [...db.objectStoreNames]
    if (stores.length === 0) return (db.close(), ok("{}"))
    const tx = db.transaction(stores, "readonly")
    const out = {}
    let left = stores.length
    for (const s of stores) {
      const keys = tx.objectStore(s).getAllKeys()
      const values = tx.objectStore(s).getAll()
      values.onsuccess = () => {
        out[s] = keys.result.map((k, i) => [k, values.result[i]])
        if (--left === 0) (db.close(), ok(JSON.stringify(out)))
      }
    }
  }
})`

/** The lines' row as the device keeps it, or null. */
const linesRow = `${held("suzatary")}.then((all) => (JSON.parse(all).chain ?? []).find(([k]) => k === "objects:store/lines")?.[1] ?? null)`

/** `write(store)` run in one read-write transaction of the store `store` of the database `name`. */
const put = (name: string, store: string, write: string): string => `new Promise((done, fail) => {
  const r = indexedDB.open(${JSON.stringify(name)})
  r.onerror = () => fail(String(r.error))
  r.onsuccess = () => {
    const tx = r.result.transaction(${JSON.stringify(store)}, "readwrite")
    ;(${write})(tx.objectStore(${JSON.stringify(store)}))
    tx.oncomplete = () => (r.result.close(), done(true))
    tx.onerror = () => fail(String(tx.error))
  }
})`

/** The rows of the database `name`'s store `store`, as `[key, value]` pairs. */
const rowsOf = (name: string, store: string): string =>
  `${held(name)}.then((all) => JSON.parse(all)[${JSON.stringify(store)}] ?? [])`

async function converse(text: string): Promise<string> {
  await app.page.navigate(`${EPOCH_BASE}/#/secretary/default`)
  await app.page.waitFor(`document.querySelector(".pagetitle") !== null`, { timeout: 15000 })
  const id = await app.page.evaluate<string>(
    `${SIGNED_POST}("/api/secretary/conversations", JSON.stringify({title: "Untitled"})).then((r) => r.json()).then((j) => j.id)`,
  )
  await app.page.navigate(`${EPOCH_BASE}/#/secretary/${id}`)
  await app.page.waitFor(`document.querySelector(".composebox") !== null`, { timeout: 8000 })
  await app.page.type(".composebox", text)
  await app.page.click(".composesend")
  return id
}

app.scene("a device holding a line the box rewrote drops its copy at the later epoch and holds only the box's lines", async () => {
  await app.resize(PHONE)
  await trustThisDevice(app.page, EPOCH_BASE, EPOCH_INSTANCE)
  await app.open(EPOCH_BASE)
  // The owner's own turn is enough to hold a line and its key: the scene is about the epoch, not the model's answer,
  // whose round the gate's load made late (trains 95 to 97).
  const id = await converse("give me a rendered answer")
  await app.page.waitFor(`document.querySelector(".turn-person") !== null`, { timeout: 20000 })
  await app.page.waitFor(`${linesRow}.then((row) => row !== null && Object.keys(row.keys).length > 0)`, { timeout: 15000 })
  type Row = { replica: { generation: number; objects: { hash: string; literal: string }[]; synced: string[]; words: { epoch: number }[] }; keys: Record<string, string> }
  const row = await app.page.evaluate<Row>(linesRow)
  // What a device that heard the box before a migration holds: the line the box rewrote, a head beside its image as the
  // box now holds it, and words of the epoch before.
  const tip = [...row.replica.objects].reverse().find((o) => o.literal.startsWith("Change("))!
  const literal = tip.literal.replace(/Locked\(key='[0-9a-f]{64}'/, `Locked(key='${"e".repeat(64)}'`)
  const old = { hash: createHash("sha256").update(literal, "utf8").digest("hex"), literal }
  const before = {
    ...row,
    replica: {
      ...row.replica,
      objects: [...row.replica.objects, old],
      synced: [...row.replica.synced, old.hash],
      words: row.replica.words.map((w) => ({ ...w, epoch: 1 })),
    },
  }
  // And a piece of a pull from before it, kept and not landed: its progress, which the later epoch drops too.
  const piece = { replica: { ...before.replica, objects: [old], synced: [old.hash] }, keys: {} }
  // The device is written from a page of the same origin that runs no app: an app tab still open would sync the round's
  // answer, landing later under load, over the row written here, and then hold nothing the later epoch drops.
  await app.page.navigate(`${EPOCH_BASE}/api/about`)
  await app.page.waitFor(`document.readyState === "complete"`)
  await app.page.evaluate(
    put("suzatary", "chain", `(s) => (s.put(${JSON.stringify(before)}, "objects:store/lines"), s.put(${JSON.stringify(piece)}, "objects:store/lines/piece/000000000000"))`),
  )
  await app.page.navigate(`${EPOCH_BASE}/?overtaken=1#/secretary/${id}`)
  await app.page.waitFor(`document.querySelector(".pagetitle") !== null`, { timeout: 15000 })
  await app.page
    .waitFor(
      `${linesRow}.then((row) => row !== null && row.replica.generation > ${row.replica.generation} && row.replica.objects.length > 0 && !row.replica.objects.some((o) => o.hash === "${old.hash}"))`,
      { timeout: 20000 },
    )
    .catch(async (e: unknown) => {
      const now = await app.page.evaluate<Row | null>(linesRow)
      const said = now === null
        ? "no lines row"
        : `generation ${now.replica.generation} (was ${row.replica.generation}), ${now.replica.objects.length} objects, the replaced line ${now.replica.objects.some((o) => o.hash === old.hash) ? "still held" : "gone"}, epochs ${now.replica.words.map((w) => w.epoch).join(",")}`
      throw new Error(`${String(e)}; the device holds ${said}`)
    })
  const after = await app.page.evaluate<Row>(linesRow)
  const box = await app.page.evaluate<{ rest: number; objects: { hash: string }[] }>(`fetch("/api/stores/lines/objects").then((r) => r.json())`)
  assert.equal(box.rest, 0, "the fixture's lines are one piece")
  assert.deepEqual(new Set(after.replica.objects.map((o) => o.hash)), new Set(box.objects.map((o) => o.hash)), "the device holds the box's lines and nothing else")
  assert.ok(after.replica.words.every((w) => w.epoch > 1), "it heard the later epoch")
  assert.ok(Object.keys(after.keys).length > 0, "the keys taken again with the lines")
  const left = await app.page.evaluate<[string, unknown][]>(rowsOf("suzatary", "chain"))
  assert.deepEqual(left.filter(([k]) => k.startsWith("objects:store/lines/piece/")), [], "no piece from before the epoch is kept")
  // The conversation reads again from the box's lines alone.
  await app.page.waitFor(`document.querySelector(".turn-person") !== null`, { timeout: 20000 })
})
