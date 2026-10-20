// The box's folds kept by what they read: one fetch per chain per heads, a fold past its next change asked again, and a
// tab that has not heard a chain's heads never reusing a fold.

import { test } from "node:test"
import assert from "node:assert/strict"

import folds from "../src/prodrome/folds"
import type { FoldPayload } from "../src/wire/generated/wire"
import { forAllAsync } from "./gen"

const payload = (chain: string, next: string | null): FoldPayload => ({
  at: "2026-09-02T12:00:00",
  heads: [chain],
  entries: [],
  failed: [],
  next: next === null ? null : { at: next, exact: true },
})

const fetching = () => {
  const asked: string[] = []
  return { asked, ask: (chain: string) => (asked.push(chain), Promise.resolve(payload(chain, null))) }
}

test("law: one fetch per chain per heads, however many visits and in whatever order", async () => {
  await forAllAsync(100, async r => {
    const f = fetching()
    let known: folds.Known = new Map([["life", ["a"]], ["project", ["b"]]])
    const c = folds.cache(() => known, f.ask, () => 0)
    // One fold per chain is kept, so a visit fetches exactly when what the chain's fold read has moved since its last.
    const last = new Map<string, string | null>()
    let moved = 0
    for (let i = 0; i < 20; i += 1) {
      if (r.bool(0.2)) known = new Map([["life", [r.pick(["a", "c"])]], ["project", [r.pick(["b", "d"])]]])
      const chain = r.pick(["life", "project"])
      await c.of(chain)
      if (last.get(chain) !== folds.keyOf(known)) moved += 1
      last.set(chain, folds.keyOf(known))
    }
    assert.equal(f.asked.length, moved)
  })
})

test("visits while a fetch is on its way share it", async () => {
  const f = fetching()
  const c = folds.cache(() => new Map([["life", ["a"]]]), f.ask, () => 0)
  await Promise.all([c.of("life"), c.of("life"), c.of("life")])
  assert.deepEqual(f.asked, ["life"])
})

test("a fold is asked again once the instant its numbers next move has come", async () => {
  const asked: string[] = []
  let now = 0
  const at = Date.UTC(2026, 8, 3) // the fold's next change, 2026-09-03T00:00:00
  const c = folds.cache(() => new Map([["life", ["a"]]]), chain => (asked.push(chain), Promise.resolve(payload(chain, "2026-09-03T00:00:00"))), () => now)
  await c.of("life")
  now = at - 1
  await c.of("life")
  assert.equal(asked.length, 1)
  now = at
  await c.of("life")
  assert.equal(asked.length, 2)
})

test("a chain whose heads the tab has not heard is fetched every time, and a put stands for a fetch", async () => {
  const f = fetching()
  let known: folds.Known = new Map([["life", null]])
  const c = folds.cache(() => known, f.ask, () => 0)
  await c.of("life")
  await c.of("life")
  assert.equal(f.asked.length, 2)
  known = new Map([["life", ["a"]]])
  c.put("life", c.key(), payload("life", null))
  await c.of("life")
  assert.equal(f.asked.length, 2)
})

test("a fold is kept under the heads known when it was asked: heads that moved while it was on its way ask again", async () => {
  const f = fetching()
  let known: folds.Known = new Map([["life", ["a"]]])
  const c = folds.cache(() => known, f.ask, () => 0)
  // Asked at "a"; the stream says "b" before the answer lands.
  const asked = c.key()
  known = new Map([["life", ["b"]]])
  c.put("life", asked, payload("life", null))
  await c.of("life")
  assert.deepEqual(f.asked, ["life"], "the fold read at a is not taken for b")
  // One asked through the cache is kept under the heads it was asked at, too: asked at d, landing once the stream says
  // e, it is not taken for e.
  known = new Map([["life", ["d"]]])
  const pending = c.of("life")
  known = new Map([["life", ["e"]]])
  await pending
  await c.of("life")
  assert.deepEqual(f.asked, ["life", "life", "life"], "asked at b, at d, then again at e")
})

test("a failed fetch is not kept", async () => {
  let n = 0
  const c = folds.cache(() => new Map([["life", ["a"]]]), () => ((n += 1), n === 1 ? Promise.reject(new Error("offline")) : Promise.resolve(payload("life", null))), () => 0)
  await assert.rejects(c.of("life"))
  await c.of("life")
  assert.equal(n, 2)
})
