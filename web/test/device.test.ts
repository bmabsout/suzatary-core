// The device section offers a new key wherever this device holds a key row, read or not, and nowhere else.

import { test } from "node:test"
import assert from "node:assert/strict"

import device from "../src/app/device"
import rendered from "../src/app/rendered"

class FakeNode {
  constructor(readonly text = "") {}
}
class FakeElement extends FakeNode {
  readonly attrs = new Map<string, string>()
  readonly kids: FakeNode[] = []
  constructor(readonly tag: string) {
    super()
  }
  setAttribute(k: string, v: string) {
    this.attrs.set(k, v)
  }
  appendChild(k: FakeNode) {
    this.kids.push(k)
  }
  addEventListener() {}
}
Object.assign(globalThis, {
  Node: FakeNode,
  document: { createElement: (tag: string) => new FakeElement(tag), createTextNode: (t: string) => new FakeNode(t) },
})

const all = (e: FakeNode): FakeElement[] => (e instanceof FakeElement ? [e, ...e.kids.flatMap(all)] : [])
const text = (e: FakeNode): string => (e instanceof FakeElement ? e.kids.map(text).join("") : e.text)

const offered = (signing: device.Signing, act: string): string[] =>
  all(device.view({ persist: { type: "Granted" }, usage: rendered.none, budget: rendered.BUDGET, signing }, { ask: () => {}, renew: () => {}, pair: () => Promise.resolve() }) as unknown as FakeNode)
    .filter(e => e.attrs.get("class")?.includes(act))
    .map(text)
const renews = (signing: device.Signing): string[] => offered(signing, "act-renewkey")

test("an unreadable key row is offered a new key, and a device with no key row is not", () => {
  assert.deepEqual(renews({ type: "Unreadable", e: "the key kept here does not read (it is not a key this browser made)" }), ["Use a new key on this device"])
  assert.deepEqual(renews({ type: "Keyed", key: "ab".repeat(32), standing: "untrusted" }), ["Use a new key on this device"])
  for (const s of [{ type: "Unheard" }, { type: "Reads" }, { type: "Unsupported", e: "this browser keeps no keys" }] as const) assert.deepEqual(renews(s), [], s.type)
})

test("an untrusted key is offered the code from the box, and a trusted or revoked one is not", () => {
  const keyed = (standing: "trusted" | "untrusted" | "revoked"): device.Signing => ({ type: "Keyed", key: "ab".repeat(32), standing })
  assert.deepEqual(offered(keyed("untrusted"), "act-pair"), ["Enter the code from the box"])
  assert.deepEqual(offered(keyed("trusted"), "act-pair"), [])
  assert.deepEqual(offered(keyed("revoked"), "act-pair"), [])
})
