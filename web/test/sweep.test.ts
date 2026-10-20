// The keys the ui registers replaced are swept once: an unsent draft moves into its register, every old key goes.

import { test } from "node:test"
import assert from "node:assert/strict"

import sweep from "../src/app/sweep"
import registers from "../src/app/registers"
import register from "../src/algebra/register"
import ui from "../src/app/ui"
import kv from "../src/app/kv"
import { memoryStore } from "./gen"

const old = (m: ReturnType<typeof memoryStore>): sweep.Old => ({ keys: m.keys, get: k => m.getItem(k), drop: k => m.removeItem(k) })

test("a draft moves into its register, every replaced key goes, and nothing else is touched", () => {
  const m = memoryStore()
  for (const [k, v] of [
    ["suzatary.draft.c1", "hello, Ada"],
    ["suzatary.draft.c2", "   "],
    ["suzatary.theme", "dark"],
    ["suzatary.chain", "life"],
    ["suzatary.secretarypanel.open", "0"],
    ["suzatary.todos.filter", "{}"],
    ["suzatary.todos.by", "flat"],
    ["suzatary.todos.folds.life", "{}"],
    ["suzatary.fold.life", "kept"],
  ] as const)
    m.setItem(k, v)
  const tab = ui.over(kv.over(() => m), "tab")
  sweep.run(old(m), tab)
  assert.equal(register.one(tab.cell(registers.draft, "c1").read(), ""), "hello, Ada")
  assert.equal(tab.cell(registers.draft, "c2").read().type, "Unwritten")
  assert.deepEqual(m.keys().sort(), ["suzatary.fold.life", "suzatary.ui.draft/c1"])
  sweep.run(old(m), tab)
  assert.deepEqual(m.keys().sort(), ["suzatary.fold.life", "suzatary.ui.draft/c1"], "a second sweep finds nothing")
})

test("a draft already in its register is not written over by an old key", () => {
  const m = memoryStore()
  const tab = ui.over(kv.over(() => m), "tab")
  tab.cell(registers.draft, "c1").set("newer")
  m.setItem("suzatary.draft.c1", "older")
  sweep.run(old(m), tab)
  assert.equal(register.one(tab.cell(registers.draft, "c1").read(), ""), "newer")
})
