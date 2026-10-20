// `once` and `shared`, the two ways a run's answer is kept, over random interleavings of calls and settlings.

import { test } from "node:test"
import assert from "node:assert/strict"

import once from "../src/algebra/once"
import { rng } from "./gen"

// Runs the test settles by hand, each numbered by when it started.
const world = () => {
  const inFlight: { readonly n: number; readonly settle: (ok: boolean) => void }[] = []
  let runs = 0
  const run = (): Promise<number> =>
    new Promise<number>((ok, no) => {
      const n = ++runs
      inFlight.push({ n, settle: good => (good ? ok(n) : no(new Error(String(n)))) })
    })
  return { run, settle: (ok: boolean) => inFlight.shift()?.settle(ok), inFlight: () => inFlight.length, runs: () => runs }
}

const settled = async (): Promise<void> => {
  for (let turn = 0; turn < 8; turn += 1) await Promise.resolve()
}

test("law: once keeps a success and forgets a failure", async () => {
  for (let seed = 1; seed <= 200; seed += 1) {
    const r = rng(seed * 2654435761)
    const w = world()
    const o = once.of(w.run)
    let kept: number | null = null
    for (let step = 0; step < 40; step += 1) {
      if (w.inFlight() > 0 && r.bool(0.4)) {
        const ok = r.bool(0.6)
        const n = w.runs()
        w.settle(ok)
        await settled()
        if (ok) kept = n
      } else if (w.inFlight() === 0 && r.bool(0.1)) {
        o.forget()
        kept = null
      } else {
        const before = w.runs()
        const answered = o.get().catch(() => null)
        await settled()
        if (kept !== null) {
          assert.equal(await answered, kept, `seed ${seed}: a kept success answers`)
          assert.equal(w.runs(), before, `seed ${seed}: a kept success runs nothing`)
        }
      }
      assert.ok(w.inFlight() <= 1, `seed ${seed}: one run at a time`)
    }
  }
})

test("once: a failure is forgotten, so the next call runs again; a success is not", async () => {
  const w = world()
  const o = once.of(w.run)
  const first = o.get().catch(() => "failed")
  assert.equal(o.get(), o.get(), "a run in flight is the one every call gets")
  w.settle(false)
  assert.equal(await first, "failed")
  await settled()
  const second = o.get()
  assert.equal(w.runs(), 2)
  w.settle(true)
  assert.equal(await second, 2)
  assert.equal(await o.get(), 2)
  assert.equal(w.runs(), 2)
  o.forget()
  void o.get()
  assert.equal(w.runs(), 3, "forgotten, the next call runs")
})

test("once: a run that throws before its promise is a failed run", async () => {
  let calls = 0
  const o = once.of((): Promise<number> => {
    calls += 1
    if (calls === 1) throw new Error("no")
    return Promise.resolve(calls)
  })
  assert.equal(await o.get().then(() => "ran", (e: unknown) => String(e)), "Error: no")
  assert.equal(await o.get(), 2)
})

test("law: shared joins a run in flight", async () => {
  for (let seed = 1; seed <= 200; seed += 1) {
    const r = rng(seed * 2246822519)
    const w = world()
    const s = once.shared(w.run)
    let idleCalls = 0
    let current: Promise<number> | null = null
    for (let step = 0; step < 40; step += 1) {
      if (w.inFlight() > 0 && r.bool(0.4)) {
        w.settle(r.bool(0.7))
        await settled()
        current = null
      } else {
        const p = s()
        p.catch(() => {})
        if (current === null) {
          idleCalls += 1
          current = p
        } else {
          assert.equal(p, current, `seed ${seed}: a call during a run joins it`)
        }
      }
      assert.ok(w.inFlight() <= 1, `seed ${seed}: one run at a time`)
    }
    assert.equal(w.runs(), idleCalls, `seed ${seed}: a run starts only for a call made while none was in flight`)
  }
})
