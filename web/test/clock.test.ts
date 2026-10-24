// The clock as a source: `sooner` is a meet with `never` its identity, and a wake fires once at its instant, never
// on a period.

import { test } from "node:test"
import assert from "node:assert/strict"

import clock from "../src/algebra/clock"
import due from "../src/app/due"
import type onscreen from "../src/app/store"
import time from "../src/app/time"
import { datedNext, isDated } from "../src/todo/todos"
import type { Entry, SpecNode } from "../src/wire/generated/wire"
import { forAll, type Rng } from "./gen"
import { everywhere } from "./zones"

const next = (r: Rng): clock.Next => (r.bool(0.2) ? clock.never : clock.at(r.int(20), r.bool()))

test("sooner is a meet: associative, commutative, idempotent, never its identity, and the earliest instant", () =>
  forAll(300, r => {
    const [a, b, c] = [next(r), next(r), next(r)]
    assert.deepEqual(clock.sooner(a, clock.sooner(b, c)), clock.sooner(clock.sooner(a, b), c))
    assert.deepEqual(clock.sooner(a, b), clock.sooner(b, a))
    assert.deepEqual(clock.sooner(a, clock.never), a)
    assert.deepEqual(clock.sooner(a, a), a)
    assert.deepEqual(clock.sooner(), clock.never)
    const ms = [a, b, c].flatMap(n => clock.fold(n, { Never: () => [], At: m => [m] }))
    const first = Math.min(...ms)
    const exact = [a, b, c].some(n => clock.fold(n, { Never: () => false, At: (m, e) => m === first && e }))
    assert.deepEqual(clock.sooner(a, b, c), ms.length === 0 ? clock.never : clock.at(first, exact), "the earliest, exact where any answer there is")
  }))

// Timers over a clock this test moves: each `set` is one pending wake.
const fake = (from = 0) => {
  let now = from
  let id = 0
  const pending = new Map<number, { at: number; k: () => void }>()
  const fx: clock.Timers = { now: () => now, set: (k, ms) => (pending.set(++id, { at: now + ms, k }), id), clear: h => void pending.delete(h) }
  const run = (to: number): void => {
    for (;;) {
      const due = [...pending.entries()].filter(([, p]) => p.at <= to).sort((x, y) => x[1].at - y[1].at)[0]
      if (due === undefined) break
      pending.delete(due[0])
      now = due[1].at
      due[1].k()
    }
    now = to
  }
  return { fx, run, pending: () => pending.size }
}

test("a wake fires once, at its instant, and re-arms past the furthest one timer reaches", () =>
  forAll(100, r => {
    const t = fake()
    let fired: number[] = []
    const at = 1 + r.int(2 ** 33)
    clock.wake(clock.at(at), () => fired.push(t.fx.now()), t.fx)
    t.run(at - 1)
    assert.deepEqual(fired, [])
    t.run(at + 10 * 2 ** 31)
    assert.deepEqual(fired, [at])
    assert.equal(t.pending(), 0, "nothing is left ticking")
    fired = []
    const cancel = clock.wake(clock.at(at), () => fired.push(t.fx.now()), t.fx)
    cancel()
    t.run(2 * at + 2 ** 34)
    assert.deepEqual(fired, [], "a disarmed wake never fires")
    clock.wake(clock.never, () => fired.push(0), t.fx)
    assert.equal(t.pending(), 0, "never arms nothing")
  }))

const pacerStart = (r: Rng): clock.Pacer => ({ last: r.int(1_000_000), pace: clock.FIRST_PACE_MS * 2 ** r.int(7) })

test("an exact answer is waited for exactly, whatever the pacer, and resets it when it fires", () =>
  forAll(300, r => {
    const p = pacerStart(r)
    const n = clock.at(p.last + r.int(10_000))
    assert.deepEqual(clock.paced(n, p), n)
    assert.equal(clock.fired(n, p, p.last + 5).pace, clock.FIRST_PACE_MS)
    assert.deepEqual(clock.paced(clock.never, p), clock.never)
  }))

// A wake that finds nothing moved arms "now" again, as the shell does; the gaps are the pace.
const backingOff = (rounds: number): number[] => {
  const t = fake()
  const rearm = clock.pacer(t.fx)
  const fires: number[] = []
  const go = (): void => rearm(clock.at(t.fx.now(), false), () => void (fires.push(t.fx.now()), fires.length < rounds && go()))
  go()
  t.run(10_000_000)
  return fires.slice(1).map((f, i) => f - (fires[i] ?? 0))
}

test("a bound that comes back at once waits the pace after the last wake, doubling to the longest as each fires", () => {
  assert.deepEqual(backingOff(11), [1000, 2000, 4000, 8000, 16000, 32000, 60000, 60000, 60000, 60000])
  const far: clock.Pacer = { last: 0, pace: clock.LONGEST_PACE_MS }
  const n = clock.at(2 * clock.LONGEST_PACE_MS, false)
  assert.deepEqual(clock.paced(n, far), n, "a bound further off than the pace is waited for as it is")
  assert.equal(clock.fired(n, far, 2 * clock.LONGEST_PACE_MS).pace, clock.FIRST_PACE_MS, "and resets it")
})

test("arming again on every change of the store neither postpones a paced wake nor advances the pace", () =>
  forAll(200, r => {
    const t = fake()
    const rearm = clock.pacer(t.fx)
    const fires: number[] = []
    const near = (): clock.Next => clock.at(t.fx.now(), false)
    const k = (): void => void fires.push(t.fx.now())
    // One wake fires, so the next near bound waits the first pace after it.
    rearm(near(), k)
    t.run(0)
    assert.deepEqual(fires, [0])
    // Store changes before that wake is due, each arming the same near bound again.
    const changes = 1 + r.int(20)
    for (let i = 0, at = 0; i < changes; i++) {
      at += r.int(clock.FIRST_PACE_MS / 20)
      t.run(at)
      rearm(near(), k)
    }
    t.run(clock.FIRST_PACE_MS)
    assert.deepEqual(fires, [0, clock.FIRST_PACE_MS], "due a pace after the last wake, however often it was armed")
    // The pace doubled once, for the one wake it held back.
    rearm(near(), k)
    t.run(20 * clock.LONGEST_PACE_MS)
    assert.deepEqual(fires, [0, clock.FIRST_PACE_MS, 3 * clock.FIRST_PACE_MS])
  }))

test("a reading's next is the instant it names, on a device in any zone", () =>
  everywhere(() =>
    forAll(300, r => {
      const t = 1_790_000_000_000 + r.int(1_000_000_000)
      const exact = r.bool()
      assert.deepEqual(clock.of({ at: time.moment(t), exact }), clock.at(t, exact))
      assert.deepEqual(clock.of(null), clock.never)
      assert.deepEqual(clock.of({ at: "not an instant", exact }), clock.never)
    }),
  ))

test("a wake two hours on lands two hours on, though a device in New York would read its spellings across the fall-back", () =>
  everywhere(() => {
    // 00:30 and 02:30 UTC on 2026-11-01 are 20:30 and 22:30 EDT the evening before; misread as New York's own wall
    // clock, they straddle the hour that repeats and would be three hours apart.
    const at = Date.UTC(2026, 10, 1, 0, 30)
    const s: onscreen.Fold = { type: "Shown", entries: [], at: "2026-11-01T00:30:00", by: { type: "Box", heads: [], fetchedAt: at, next: { at: "2026-11-01T02:30:00", exact: true }, fresh: true } }
    assert.deepEqual(due.of(s, at, time.UTC), clock.at(at + 2 * 3_600_000))
    const t = fake(at)
    const fired: number[] = []
    clock.wake(due.of(s, at, time.UTC), () => fired.push(t.fx.now()), t.fx)
    t.run(at + 4 * 3_600_000)
    assert.deepEqual(fired, [at + 2 * 3_600_000])
  }))

const entry = (spec: SpecNode | null): Entry => ({
  todo: "t",
  state: "open",
  states: ["open"],
  at: "",
  claimed: "",
  value: 0.5,
  recurring: false,
  unconfirmed: false,
  conflicts: {},
  authored:
    spec === null
      ? null
      : {
          rich: { body: null, detail: null, rationale: null, waiting_on: null, keys: { body: null, detail: null, rationale: null, waiting_on: null } },
          block: { kind: "todo", name: "todo" },
          created: "2026-08-30",
          body: "a dated todo",
          text: "a dated todo",
          detail: "",
          category: "study",
          waiting_on: "",
          rationale: [],
          spec,
          actor: "ada",
          at: "2026-09-01T09:15:00",
          source: null,
          subtodos: [],
        },
  candidates: [],
  stream: [],
})

test("a row's dated edge is when it enters the fortnight and just after it leaves, and is due then", () =>
  everywhere(() =>
    forAll(200, r => {
      const at = "2026-09-02T12:00:00"
      const from = Date.UTC(2026, 8, 2, 12)
      const days = 1 + r.int(40)
      const end = time.moment(from + days * 86_400_000).slice(0, 19)
      const es = [entry({ kind: "decay", start: 0.6, end: 0.05, endDate: end, leadUpHours: 72 })]
      const edge = datedNext(es, at)
      const fortnight = 14 * 86_400_000
      assert.equal(edge, days > 14 ? from + days * 86_400_000 - fortnight : from + days * 86_400_000 + 1)
      if (edge === null) return
      assert.equal(isDated(es[0] as Entry, time.moment(edge - 1)), days <= 14)
      assert.equal(isDated(es[0] as Entry, time.moment(edge)), days > 14)
      const s: onscreen.Fold = { type: "Shown", entries: es, at, by: { type: "Box", heads: [], fetchedAt: from, next: null, fresh: true } }
      const due0 = clock.fold(due.of(s, from, time.UTC), { Never: () => NaN, At: m => m })
      assert.ok(due0 <= edge, "due no later than the edge")
    }),
  ))

test("what is printed to the day moves at the owner's midnight, on a device in any zone", () =>
  everywhere(() =>
    forAll(300, r => {
      const zone: time.Zone = { name: r.pick(["UTC", "America/New_York", "Europe/London", "Europe/Berlin", "Pacific/Chatham"]) }
      const t = Date.UTC(2026, 0, 1) + r.int(400 * 86_400) * 1000 + r.int(1000)
      const next = clock.fold(clock.nextDay(zone, t), { Never: () => NaN, At: m => m })
      assert.ok(next > t && next - t <= 86_400_000)
      assert.equal(time.date(zone, next - 1), time.date(zone, t), "the same day until then")
      assert.equal(time.daysBetween(time.date(zone, t), time.date(zone, next)), 1, "and the next one there")
    }),
  ))
