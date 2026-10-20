// `stream`: every stream from the box gives up by one policy, over random runs of opens and failures.

import { test } from "node:test"
import assert from "node:assert/strict"

import stream from "../src/algebra/stream"
import { forAll } from "./gen"

const event = (r: { bool: (p?: number) => boolean }): stream.Event =>
  r.bool(0.3) ? { type: "Opened" } : { type: "Failed", closed: r.bool(0.1) }

test("law: a stream is given up after GIVE_UP failures with no open between, or at once when closed, and for good", () => {
  forAll(300, r => {
    const es = r.array(r.int(20), () => event(r))
    let run = 0
    let given = false
    let h = stream.START
    for (const e of es) {
      h = stream.step(h, e)
      run = e.type === "Opened" ? 0 : run + 1
      given = given || (e.type === "Failed" && (e.closed || run >= stream.GIVE_UP))
      assert.equal(h.type === "GivenUp", given)
    }
  })
})

test("open: the socket is closed once, when it is given up, and each open is told", () => {
  const ls: Record<string, (() => void)[]> = { open: [], error: [], message: [] }
  let closed = 0
  let opened = 0
  let gaveUp = 0
  const socket: stream.Socket = {
    readyState: 0,
    addEventListener: (type: string, l: (e: { readonly data: string }) => void) => void ls[type]?.push(() => l({ data: "" })),
    close: () => void (closed += 1),
  }
  stream.open(() => socket, { frame: () => {}, opened: () => void (opened += 1), gaveUp: () => void (gaveUp += 1) }, () => () => {})
  const fire = (type: string) => ls[type]?.forEach(l => l())
  fire("open")
  for (let i = 0; i < stream.GIVE_UP + 2; i += 1) fire("error")
  assert.deepEqual([opened, closed, gaveUp], [1, 1, 1])
})

// A socket the test drives: its listeners, how often it was closed.
const fake = () => {
  const ls: Record<string, (() => void)[]> = { open: [], error: [], message: [] }
  let closed = 0
  const socket: stream.Socket = {
    readyState: 0,
    addEventListener: (type: string, l: (e: { readonly data: string }) => void) => void ls[type]?.push(() => l({ data: "" })),
    close: () => void (closed += 1),
  }
  return { socket, fire: (type: string) => ls[type]?.forEach(l => l()), closed: () => closed }
}

test("law: a given-up stream opens anew when the network may be back, and a live one is left alone", () => {
  forAll(100, r => {
    const made: ReturnType<typeof fake>[] = []
    let back = (): void => {}
    let opened = 0
    stream.open(() => (made.push(fake()), made[made.length - 1]!.socket), { frame: () => {}, opened: () => void (opened += 1) }, b => ((back = b), () => {}))
    back()
    assert.equal(made.length, 1, "a stream not given up is not made again")
    const failures = r.bool(0.3) ? 1 : stream.GIVE_UP
    for (let i = 0; i < failures; i += 1) made[0]!.fire("error")
    back()
    assert.equal(made.length, failures >= stream.GIVE_UP ? 2 : 1)
    if (made.length === 2) {
      made[1]!.fire("open")
      assert.equal(opened, 1, "the new socket's open is told")
      for (let i = 0; i < stream.GIVE_UP; i += 1) made[1]!.fire("error")
      back()
      assert.equal(made.length, 3, "and it gives up and comes back by the same policy")
    }
  })
})

test("open: closing removes the wake, so nothing opens after it", () => {
  let woken = 1
  const f = fake()
  const close = stream.open(() => f.socket, { frame: () => {} }, () => () => void (woken = 0))
  close()
  assert.deepEqual([woken, f.closed()], [0, 1])
})

test("law: a given-up stream on a tab that stays shown and online opens anew on the slow tick, and only then", t => {
  t.mock.timers.enable({ apis: ["setInterval"] })
  const quiet = { addEventListener: () => {}, removeEventListener: () => {}, visibilityState: "visible" }
  Object.assign(globalThis, { window: quiet, document: quiet })
  const made: ReturnType<typeof fake>[] = []
  const close = stream.open(() => (made.push(fake()), made[made.length - 1]!.socket), { frame: () => {} })
  t.mock.timers.tick(stream.TICK_MS * 3)
  assert.equal(made.length, 1, "a live stream is not made again by the tick")
  for (let i = 0; i < stream.GIVE_UP; i += 1) made[0]!.fire("error")
  t.mock.timers.tick(stream.TICK_MS - 1)
  assert.equal(made.length, 1, "nothing before the tick")
  t.mock.timers.tick(1)
  assert.equal(made.length, 2, "the tick opens it anew")
  close()
  for (let i = 0; i < stream.GIVE_UP; i += 1) made[1]!.fire("error")
  t.mock.timers.tick(stream.TICK_MS * 2)
  assert.equal(made.length, 2, "closed, the tick is gone")
})
