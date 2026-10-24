// A row takes what comes in, in order: the owed keys' blocks forgotten, the row joined, then the forgotten keys dropped
// in memory and on the device; a forget that fails keeps the keys and is said, and joins what came in all the same.

import { test } from "node:test"
import assert from "node:assert/strict"

import lo from "../src/algebra/lo"
import type { StoredObject } from "../src/wire/generated/wire"
import locked from "../src/prodrome/locked"
import take from "../src/prodrome/take"
import { NOTHING } from "../src/prodrome/replica"

const [DEAD, LIVE] = ["d".repeat(64), "e".repeat(64)]
const incoming = locked.of(NOTHING, { [DEAD]: "00", [LIVE]: "11" })

type Said = readonly [string, unknown]

const effects = (forget: lo.Settled<null>, alter: lo.Settled<null> = lo.val(null)) => {
  const said: Said[] = []
  let row = locked.NONE
  const fx: take.Effects<StoredObject> = {
    held: () => row,
    dispatch: e => {
      said.push([e.type, e.type === "Held" ? locked.names(e.row.keys) : e.keys])
      row = e.type === "Held" ? locked.join(row, e.row) : locked.without(row, e.keys)
    },
    purge: {
      erased: async r => (said.push(["erased", locked.names(r.keys)]), { [DEAD]: ["block"] }),
      forget: blocks => (said.push(["forget", blocks]), Promise.resolve(forget)),
    },
    dead: () => new Set([DEAD]),
    alter: gone => (said.push(["alter", gone]), Promise.resolve(alter)),
    owing: failed => void said.push(["owing", failed]),
  }
  return { fx, said, row: () => row }
}

test("law: the blocks go first, then the row is joined, then the keys are dropped, in memory before the device", async () => {
  const { fx, said, row } = effects(lo.val(null))
  await take.into(fx, incoming)
  assert.deepEqual(said, [
    ["erased", [DEAD]],
    ["forget", ["block"]],
    ["Held", [DEAD, LIVE]],
    ["Forgot", [DEAD]],
    ["alter", [DEAD]],
    ["owing", null],
  ])
  assert.deepEqual(locked.names(row().keys), [LIVE])
})

test("a forget that fails joins what came in, keeps the owed key, drops nothing and says why", async () => {
  const { fx, said, row } = effects(lo.error("the cache did not open"))
  await take.into(fx, incoming)
  assert.deepEqual(said.map(([what]) => what), ["erased", "forget", "Held", "owing"])
  assert.deepEqual(said.at(-1), ["owing", "the cache did not open"])
  assert.deepEqual(locked.names(row().keys), [DEAD, LIVE], "the key stays, to be asked again")
})

test("a device copy that was not written is said, and the key is owed again by the next take that reads it", async () => {
  const { fx, said } = effects(lo.val(null), lo.error("quota"))
  await take.into(fx, incoming)
  assert.match(String(said.at(-1)?.[1]), /quota/)
})

test("a row that owes nothing asks for no forget and writes nothing to the device", async () => {
  const { fx, said } = effects(lo.val(null))
  await take.into({ ...fx, dead: () => new Set() }, incoming)
  assert.deepEqual(said, [["Held", [DEAD, LIVE]], ["owing", null]])
})
