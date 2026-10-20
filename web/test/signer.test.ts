// The device's key is minted only where the device keeps none: a row that does not read is said, never written over.

import { test } from "node:test"
import assert from "node:assert/strict"

import signer from "../src/prodrome/signer"
import type idb from "../src/app/idb"
import { forAll } from "./gen"

test("law: an unreadable signer row never mints a key", async () => {
  const rows: idb.Row<signer.Key>[] = []
  forAll(100, r =>
    rows.push(
      r.pick<idb.Row<signer.Key>>([
        { type: "Absent" },
        { type: "Unreadable", why: r.string("abc ", 6) },
        { type: "Unavailable", why: r.string("abc ", 6) },
        { type: "Held", v: { public: r.string("0123456789abcdef", 64), key: {} as CryptoKey } },
      ]),
    ),
  )
  for (const row of rows) {
    let mints = 0
    const fresh: signer.Key = { public: "f".repeat(64), key: {} as CryptoKey }
    const held = await signer.keyOf(row, async () => ((mints += 1), fresh)).catch((): "Asked again" => "Asked again")
    assert.equal(mints, row.type === "Absent" ? 1 : 0, row.type)
    if (row.type === "Unreadable") assert.deepEqual(held !== "Asked again" && held.type, "Unreadable", "a row that does not read is offered a new key")
    if (row.type === "Unavailable") assert.equal(held, "Asked again", "a store that does not open is a failure, so it is asked again")
    if (row.type === "Held") assert.deepEqual(held, { type: "Held", key: row.v })
    if (row.type === "Absent") assert.deepEqual(held, { type: "Held", key: fresh })
  }
})
