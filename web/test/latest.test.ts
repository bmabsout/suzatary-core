// `latest`: answers landing in any order are kept in the order asked, and every dropped one is let go.

import { test } from "node:test"
import assert from "node:assert/strict"

import latest from "../src/algebra/latest"
import { forAll } from "./gen"

const settled = async (): Promise<void> => {
  for (let turn = 0; turn < 8; turn += 1) await Promise.resolve()
}

test("law: an answer landing after a later-asked one is dropped and let go; what lands climbs", async () => {
  const cases: { n: number; order: number[]; failing: boolean[] }[] = []
  forAll(200, r => {
    const n = 1 + r.int(8)
    const order = Array.from({ length: n }, (_, i) => i)
    for (let i = n - 1; i > 0; i -= 1) {
      const j = r.int(i + 1)
      ;[order[i], order[j]] = [order[j] ?? 0, order[i] ?? 0]
    }
    cases.push({ n, order, failing: r.array(n, () => r.bool(0.2)) })
  })
  for (const { n, order, failing } of cases) {
    const landed: number[] = []
    const released: number[] = []
    const l = latest.of<number>(x => void (x.type === "Val" ? landed.push(x.v) : landed.push(-1)), x => void released.push(x))
    const settle: (() => void)[] = []
    for (let i = 0; i < n; i += 1) {
      void l.ask(() => new Promise<number>((ok, no) => settle.push(() => (failing[i] ? no(new Error("no")) : ok(i)))))
    }
    for (const i of order) {
      settle[i]?.()
      await settled()
    }
    const kept = order.filter((i, k) => order.slice(0, k).every(j => j < i))
    assert.deepEqual(landed, kept.map(i => (failing[i] ? -1 : i)))
    assert.deepEqual(released.sort(), order.filter(i => !kept.includes(i) && !failing[i]).sort())
  }
})

test("close: nothing lands after it, and an answer on its way is let go", async () => {
  const landed: number[] = []
  const released: number[] = []
  const l = latest.of<number>(x => void (x.type === "Val" && landed.push(x.v)), x => void released.push(x))
  const pending = l.ask(() => Promise.resolve(7))
  l.close()
  await pending
  assert.deepEqual([landed, released], [[], [7]])
})
