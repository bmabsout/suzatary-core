// The inbox's registers, read by the built module from a frozen proposal store, agree with the box's fold.

import { test } from "node:test"
import assert from "node:assert/strict"
import { createRequire } from "node:module"
import { readdirSync, readFileSync } from "node:fs"

import type { Core, Named } from "../src/prodrome/core"
import { proposalReadings, readingOf } from "../src/prodrome/core"

const core = (): Core => {
  const built = process.env["SUZATARY_PRODROME_WASM"]
  if (built === undefined || built === "") throw new Error("SUZATARY_PRODROME_WASM is unset — run scripts/web-test.sh")
  return createRequire(import.meta.url)(`${built}/nodejs/prodrome.js`) as Core
}
const wasm = core()
const store = "app/io/tests/fixtures/inbox-proposals"
const objects: Named[] = readdirSync(`${store}/objects`)
  .sort()
  .map(f => ({ hash: f.replace(/\.py$/, ""), literal: readFileSync(`${store}/objects/${f}`, "utf8") }))
const registers = JSON.parse(readFileSync(`${store}/registers.json`, "utf8")) as Record<
  string,
  { content: string[]; state: string[] }
>

test("every proposal's registers are the ones the box folds", () => {
  const read = proposalReadings(wasm, objects, null, []).readings
  assert.deepEqual(read.map(r => r.proposal).sort(), Object.keys(registers).sort())
  for (const r of read) {
    const theirs = registers[r.proposal]
    assert.deepEqual(r.registers.content.map(w => w.hash), theirs?.content, `${r.proposal} content`)
    assert.deepEqual(r.registers.state.map(w => w.hash), theirs?.state, `${r.proposal} state`)
    assert.ok(r.registers.content.every(w => w.value.proposal === r.proposal), `${r.proposal} asks for itself`)
  }
  assert.ok(read.some(r => r.registers.state.length === 0), "some proposal still stands")
  assert.ok(read.some(r => r.registers.state.some(w => w.value.phase === "trying")), "some proposal is mid-try")
})

test("a reading is a function of the object set, not of the order it arrived in", () => {
  const once = proposalReadings(wasm, objects, null, []).readings
  const reversed = proposalReadings(wasm, [...objects].reverse(), null, []).readings
  const twice = proposalReadings(wasm, [...objects, ...objects.slice(0, 5)].reverse(), null, []).readings
  assert.deepEqual(reversed, once)
  assert.deepEqual(twice, once)
})

test("before anything was asked, nothing is", () => {
  const read = proposalReadings(wasm, objects, "2026-01-01T00:00:00", []).readings
  assert.equal(read.length, Object.keys(registers).length, "every proposal is still a row")
  assert.ok(read.every(r => r.registers.content.length === 0 && r.registers.state.length === 0))
})

test("a tampered object is left out with what rests on it, named, and the rest reads as if it never arrived", () => {
  const [first, ...rest] = objects
  if (first === undefined) throw new Error("the store holds objects")
  const read = proposalReadings(wasm, [{ hash: first.hash, literal: `${first.literal} ` }, ...rest], null, [])
  assert.deepEqual(read.excluded.map(x => x.name), [first.hash])
  assert.match(read.excluded[0]?.why ?? "", /does not hash to its own name/)
  const resting = new Set(read.excluded[0]?.resting)
  assert.deepEqual(read.readings, proposalReadings(wasm, rest.filter(o => !resting.has(o.hash)), null, []).readings)
  assert.deepEqual(proposalReadings(wasm, objects, null, []).excluded, [])
})

test("a proposal's registers read by the tab's one reading agree with the box's own fold of the same store", () => {
  for (const r of proposalReadings(wasm, objects, null, []).readings) {
    const theirs = registers[r.proposal]
    const width = (n: number) => (n === 0 ? "Unwritten" : n === 1 ? "Value" : "Conflict")
    assert.equal(readingOf(r.registers.state).type, width(theirs?.state.length ?? 0), `${r.proposal} state`)
    assert.equal(readingOf(r.registers.content).type, width(theirs?.content.length ?? 0), `${r.proposal} content`)
  }
})
