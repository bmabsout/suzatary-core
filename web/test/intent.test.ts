// The intent vocabulary: what the kit prints the shell parses back, and nothing else parses.

import { test } from "node:test"
import assert from "node:assert/strict"

import intent from "../src/typst/intent"
import { SLUG, forAll } from "./gen"

const anIntent = (r: Parameters<Parameters<typeof forAll>[1]>[0]): intent.Intent => ({
  type: r.pick(["Open", "Complete"]),
  todo: { kind: "todo", chain: r.pick(["life", "suzatary"]), id: r.string(SLUG.replace("-", "_"), 1 + r.int(16)) },
})

test("parse reads back what print wrote, for every intent", () =>
  forAll(500, r => {
    const i = anIntent(r)
    assert.deepEqual(intent.parse(intent.print(i)), i)
  }))

test("the kit's spelling is the printed one: a verb, a space, a todo's reference", () => {
  assert.deepEqual(intent.parse("open todo:life/file-permit"), { type: "Open", todo: { kind: "todo", chain: "life", id: "file-permit" } })
  assert.deepEqual(intent.parse("complete todo:suzatary/x"), { type: "Complete", todo: { kind: "todo", chain: "suzatary", id: "x" } })
})

test("anything outside the vocabulary is refused", () => {
  for (const text of [
    "",
    "open",
    "open ",
    "open  todo:life/x",
    "OPEN todo:life/x",
    "delete todo:life/x",
    "send todo:life/x",
    "open mail:a1b2c3d4",
    "open todo:life/x extra",
    "open todo:life/",
    "javascript:alert(1)",
    "complete todo:life/x\n",
    "constructor todo:life/x",
    "toString todo:life/x",
  ])
    assert.equal(intent.parse(text), null, JSON.stringify(text))
})

test("a random string parses only when it prints back to itself", () =>
  forAll(500, r => {
    const text = r.string(`${SLUG} :/_`, r.int(30))
    const i = intent.parse(text)
    if (i !== null) assert.equal(intent.print(i), text)
  }))
