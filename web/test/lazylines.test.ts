// A device holding part of the lines' content holds a state the box could have sent: its skeleton is a down-set the box
// sent, its content is the box's bytes for their names, and every reading the worker answers of the content held is the
// reading of the whole pull at the same skeleton, or the page names what it still fetches.

import { test } from "node:test"
import assert from "node:assert/strict"
import { createRequire } from "node:module"
import { readFileSync } from "node:fs"

import type { Core, Named } from "../src/prodrome/core"
import { conversationOf, listOfLines, objectsOf, searchOf } from "../src/prodrome/core"
import lines from "../src/prodrome/lines"
import content from "../src/prodrome/content"
import { empty, pulled } from "../src/prodrome/replica"
import type { Replica } from "../src/prodrome/replica"
import type { SkeletonNode, SkeletonPayload } from "../src/wire/generated/wire"
import { forAll, forAllAsync, type Rng } from "./gen"

const core = ((): Core => {
  const built = process.env["SUZATARY_PRODROME_WASM"]
  if (built === undefined || built === "") throw new Error("SUZATARY_PRODROME_WASM is unset — run scripts/web-test.sh")
  return createRequire(import.meta.url)(`${built}/nodejs/prodrome.js`) as Core
})()

type Shipped = { hash: string; text: string }
const box = JSON.parse(readFileSync("app/core/tests/web/replicas.json", "utf8")) as {
  inbox: Shipped[]
  conversations: Shipped[]
  skeleton: SkeletonPayload
  keys: Record<string, string>
}
const inbox: Named[] = box.inbox.map(o => ({ hash: o.hash, literal: o.text }))
const literals = new Map(box.conversations.map(o => [o.hash, o.text]))
const ids = [...new Set(box.skeleton.nodes.flatMap(n => (n.header.node === "line" ? [n.header.conversation] : [])))]

// A down-set of the skeleton the box's pieces could have left, in the order they arrived.
const skeleton = (r: Rng): Replica<SkeletonNode> => {
  const held = new Set<string>()
  const left = [...box.skeleton.nodes]
  const nodes: SkeletonNode[] = []
  for (let n = r.int(left.length + 1); n > 0; n -= 1) {
    const next = r.pick(left.filter(o => o.parents.every(p => held.has(p))))
    nodes.push(next)
    held.add(next.hash)
    left.splice(left.indexOf(next), 1)
  }
  return pulled(empty<SkeletonNode>(), { ...box.skeleton, objects: nodes, untrusted: [], root: null }, 1)
}

// Some of the box's literals for nodes the skeleton holds: content is fetched for a node the skeleton names, and let go
// once none names it.
const fetched = (r: Rng, s: Replica<SkeletonNode>): ReadonlyMap<string, string> =>
  new Map(s.objects.filter(() => r.bool(0.7)).map(n => [n.hash, literals.get(n.hash)!]))

// Every answer the worker's lines holding `objects` give about `scope`, or the refusal they give instead.
const read = (objects: readonly Named[], scope: lines.Scope): string => {
  const held = new core.Lines()
  const proposals = new core.Proposals(objectsOf(inbox))
  try {
    held.hold(objectsOf(objects), "[]")
    held.keys(JSON.stringify(box.keys), "[]")
    return scope.type === "Whole"
      ? JSON.stringify([listOfLines(held, proposals), searchOf(held, "draft"), searchOf(held, "train")])
      : JSON.stringify(conversationOf(held, proposals, scope.id, {}, null))
  } catch (e: unknown) {
    return `refused: ${String(e)}`
  } finally {
    proposals.free()
    held.free()
  }
}

const scopes: readonly lines.Scope[] = [lines.WHOLE, ...ids.map((id): lines.Scope => ({ type: "Conversation", id })), { type: "Conversation", id: "never-opened" }]

test("law: a reading of part-fetched lines is the whole pull's reading at the same skeleton, or names what it fetches", () => {
  forAll(40, r => {
    const s = skeleton(r)
    const kept = fetched(r, s)
    const has = (name: string): boolean => kept.has(name)
    const names = new Set(s.objects.map(n => n.hash))
    // What the box's whole pull at this skeleton would have sent: every node with its literal.
    const whole = s.objects.map(n => ({ hash: n.hash, literal: literals.get(n.hash)! }))
    for (const scope of scopes) {
      const need = lines.need(s, has, scope)
      if (need.type === "Fetching") {
        assert.ok(need.nodes.length > 0)
        assert.ok(need.nodes.every(n => names.has(n.hash) && !has(n.hash)), "it fetches only what the skeleton names and the device lacks")
        continue
      }
      const held = [...kept].map(([hash, literal]) => ({ hash, literal }))
      assert.equal(read(held, scope), read(whole, scope), JSON.stringify(scope))
    }
  })
})

test("law: content is taken only where its bytes hash to the name asked for, in the place asked", async () => {
  const objects = box.conversations.map(o => ({ hash: o.hash, literal: o.text }))
  await forAllAsync(20, async r => {
    const asked = objects.filter(() => r.bool(0.5)).slice(0, content.MOST_HASHES)
    if (asked.length === 0) return
    const names = asked.map(o => o.hash)
    assert.equal((await content.admit(names, { objects: asked })).type, "Val")
    const i = r.int(asked.length)
    const forged = asked.map((o, j) => (j === i ? { ...o, literal: `${o.literal} ` } : o))
    assert.equal((await content.admit(names, { objects: forged })).type, "Error", "a literal that is not its name's")
    if (asked.length > 1) assert.equal((await content.admit(names, { objects: [...asked].reverse() })).type, "Error", "answers out of place")
    assert.equal((await content.admit(names, { objects: asked.slice(1) })).type, "Error", "an answer short of the question")
  })
})

test("an opened conversation's names go out before the whole store's batches still queued, and each name is asked once", async () => {
  const asked: string[][] = []
  const answers: (() => void)[] = []
  // Each ask settles the wait for it, so the test moves when an answer goes out and never on a timer.
  let sent = (): void => {}
  const next = (): Promise<void> => new Promise(done => (sent = done))
  const fetch = content.fetcher({
    ask: hashes => {
      asked.push([...hashes])
      sent()
      return new Promise(done => answers.push(() => done({ objects: hashes.map(h => ({ hash: h, literal: literals.get(h)! })) })))
    },
    asked: () => {},
    came: () => {},
    lost: () => {},
    atOnce: () => 4,
    held: () => Promise.resolve(),
  })
  // One name a batch, so the whole store queues more batches than go out at once.
  const one = box.skeleton.nodes.map(n => ({ ...n, size: content.MOST_BYTES }))
  const whole = fetch("Behind", one)
  await new Promise(done => setTimeout(done, 0))
  assert.equal(asked.length, 4, "as many out as may be")
  const opened = one.filter(n => !asked.flat().includes(n.hash)).slice(-2)
  const first = fetch("First", opened)
  await new Promise(done => setTimeout(done, 0))
  for (let i = 0; i < opened.length; i += 1) {
    const going = next()
    answers.shift()!()
    await going
  }
  assert.deepEqual(
    asked.slice(4, 4 + opened.length).flat().sort(),
    opened.map(n => n.hash).sort(),
    "the next out are the opened conversation's",
  )
  while (asked.flat().length < one.length) {
    const more = next()
    answers.shift()!()
    await more
  }
  for (const answer of answers.splice(0)) answer()
  await Promise.all([whole, first])
  assert.equal(new Set(asked.flat()).size, asked.flat().length, "no name asked twice")
})

test("over HTTP/1.1 one content answer is out at a time, beside the three streams, under the browser's six a host", () => {
  assert.equal(content.atOnce("http/1.1"), 1)
  assert.equal(content.atOnce(""), 1, "a protocol the browser has not said is read as HTTP/1.1")
  assert.equal(content.atOnce("h2"), 4)
})
