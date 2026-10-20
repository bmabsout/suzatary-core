// The laws of the curve: its window, the line drawn from the core's knots, its colour at every height, and the label.

import { test } from "node:test"
import assert from "node:assert/strict"

import curve from "../src/todo/curve"
import fulfillment from "../src/todo/fulfillment"
import { forAll } from "./gen"
import { everywhere } from "./zones"
import time from "../src/app/time"
import type { Rng } from "./gen"

const NOW = 1_790_000_000
const H = 3600

class FakeNode {}
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
  document: {
    createElement: (tag: string) => new FakeElement(tag),
    createElementNS: (_: string, tag: string) => new FakeElement(tag),
    createTextNode: () => new FakeNode(),
  },
})

const all = (e: FakeElement): FakeElement[] => [e, ...e.kids.flatMap(k => (k instanceof FakeElement ? all(k) : []))]

// Values that land on the thresholds and times that repeat (a jump) as often as anything else.
const genValue = (r: Rng): number => (r.bool(0.2) ? r.pick([0.5, 0.7, 0, 1]) : r.int(1001) / 1000)

// The todo page's thirty days around a now anywhere in a year, with a sample at from, now and to, as the core's knots have.
const genWindow = (r: Rng): curve.Window => curve.thirtyDays(NOW + r.int(365 * 24) * H)

const genXs = (r: Rng, w: curve.Window): curve.Sample[] => {
  const inner = r.array(r.int(30), () => w.from + r.int(w.to - w.from + 1)).filter(t => t !== w.now)
  const ts = [...new Set([w.from, w.now, w.to])].concat(inner).sort((a, b) => a - b)
  return ts.flatMap(t => (r.bool(0.1) && t !== w.now ? [t, t] : [t])).map(t => ({ t, v: genValue(r) }))
}

const genDrawn = (r: Rng): curve.Drawn => {
  const w = genWindow(r)
  const d = curve.fold(curve.of(w, genXs(r, w)), { Unsampled: () => null, Sampled: d => d })
  assert.ok(d !== null)
  return d
}

const vertices = (path: string): [number, number][] =>
  [...path.matchAll(/[ML] (-?[\d.]+) (-?[\d.]+)/g)].map(m => [Number(m[1]), Number(m[2])])

test("law: the window is honoured: split at now, drawn from its start to its end, now's sample first ahead", () => {
  forAll(500, (r, seed) => {
    const w = genWindow(r)
    const xs = genXs(r, w)
    const d = curve.fold(curve.of(w, xs), { Unsampled: () => null, Sampled: d => d })
    assert.ok(d !== null, `seed ${seed}`)
    assert.deepEqual(d.window, w, `seed ${seed}`)
    assert.deepEqual([...d.past, ...d.ahead], xs, `seed ${seed}`)
    assert.ok(d.past.every(p => p.t < w.now), `seed ${seed}`)
    assert.ok(d.ahead.every(p => p.t >= w.now), `seed ${seed}`)
    assert.equal(d.ahead[0].t, w.now, `seed ${seed}`)
    const width = 1 + r.int(400)
    const at = curve.x(w, width)
    assert.equal(at(w.from), 0, `seed ${seed}`)
    assert.ok(Math.abs(at(w.to) - width) < 1e-9, `seed ${seed}`)
    assert.ok(Math.abs(at(w.now) - (width * (w.now - w.from)) / (w.to - w.from)) < 1e-9, `seed ${seed}`)
    for (const p of xs) assert.ok(at(p.t) >= 0 && at(p.t) <= width + 1e-9, `seed ${seed}`)
  })
  assert.deepEqual(curve.thirtyDays(NOW), { from: NOW - 15 * 24 * H, now: NOW, to: NOW + 15 * 24 * H })
})

test("law: the line is drawn from the core's knots, every one a vertex in order, the halves meeting at now's value", () => {
  forAll(300, (r, seed) => {
    const d = genDrawn(r)
    const h = curve.halves(d)
    const vertex = (p: curve.Sample): [number, number] => [curve.x(d.window, curve.W)(p.t), curve.y(p.v)]
    const drawn = (curve.line(d, time.UTC) as unknown as FakeElement)
    const paths = new Map(all(drawn).filter(e => e.tag === "path").map(e => [e.attrs.get("class"), e.attrs.get("d") ?? ""]))
    const check = (said: string | undefined, ps: readonly curve.Sample[]): void => {
      assert.ok(said !== undefined, `seed ${seed}`)
      const vs = vertices(said)
      assert.equal(vs.length, ps.length, `seed ${seed}`)
      vs.forEach(([x, y], i) => {
        const [ex, ey] = vertex(ps[i] ?? { t: NaN, v: NaN })
        assert.ok(Math.abs(x - ex) <= 0.005 && Math.abs(y - ey) <= 0.005, `seed ${seed}: vertex ${i} at ${x},${y}, the knot at ${ex},${ey}`)
      })
    }
    check(paths.get("curveline ahead"), d.ahead)
    if (d.past.length === 0) assert.equal(paths.has("curveline past"), false, `seed ${seed}`)
    else {
      assert.deepEqual(h.past, [...d.past, d.ahead[0]], `seed ${seed}`)
      check(paths.get("curveline past"), h.past)
    }
  })
})

test("law: the stroke's colour at a height is the colour of the value there, red at 0 and green at 1", () => {
  assert.equal(curve.y(0), curve.H)
  assert.equal(curve.y(1), 0)
  const d = curve.fold(curve.of(curve.thirtyDays(NOW), [{ t: NOW, v: 0.4 }, { t: NOW + 360 * H, v: 0.9 }]), {
    Unsampled: () => null,
    Sampled: d => d,
  })
  assert.ok(d !== null)
  const els = all(curve.line(d, time.UTC) as unknown as FakeElement)
  const g = els.find(e => e.tag === "linearGradient")
  assert.ok(g !== undefined)
  assert.equal(g.attrs.get("gradientUnits"), "userSpaceOnUse")
  const [y1, y2] = [Number(g.attrs.get("y1")), Number(g.attrs.get("y2"))]
  const stops = g.kids.filter((k): k is FakeElement => k instanceof FakeElement)
  assert.equal(stops.length, curve.STOPS + 1)
  for (const s of stops) {
    const o = Number(s.attrs.get("offset"))
    // The height a stop sits at is the height of the value it is coloured by.
    assert.ok(Math.abs(y1 + o * (y2 - y1) - curve.y(o)) < 1e-9, `offset ${o}`)
    assert.equal(s.attrs.get("stop-color"), fulfillment.colour(o))
  }
  assert.deepEqual([stops[0]?.attrs.get("offset"), stops.at(-1)?.attrs.get("offset")], ["0", "1"])
  const dot = els.find(e => e.attrs.get("class") === "curvedot")
  assert.match(dot?.attrs.get("style") ?? "", new RegExp(`background:${fulfillment.colour(0.4).replace(/[()%.]/g, c => `\\${c}`)}`))
})

test("law: the first crossing below a threshold is at the first sample that is worse, met from the one before", () => {
  forAll(500, (r, seed) => {
    const s = genDrawn(r).ahead
    for (const x of [fulfillment.WATCH, fulfillment.FINE]) {
      const k = s.findIndex(p => p.v < x)
      const at = curve.below(s, x)
      const [a, b] = [s[k - 1], s[k]]
      if (b === undefined) assert.equal(at, null, `seed ${seed}`)
      else if (a === undefined) assert.equal(at, b.t, `seed ${seed}`)
      else {
        assert.ok(at !== null && a.t <= at && at <= b.t, `seed ${seed}: ${at} not in [${a.t}, ${b.t}]`)
        assert.ok(s.slice(0, k).every(p => p.v >= x), `seed ${seed}`)
      }
    }
  })
})

test("the label names the value and state now, and the next state it would cross into, within the window", () => {
  const at = (h: number, v: number) => ({ t: NOW + h * H, v })
  const thirty = (xs: curve.Sample[]): string =>
    curve.fold(curve.of(curve.thirtyDays(NOW), xs), { Unsampled: () => "", Sampled: curve.label })
  assert.equal(thirty([at(-360, 0.9), at(0, 0.62), at(24, 0.54), at(36, 0.46), at(360, 0.3)]), "Fulfillment 62% now, watch; problem in about 30 hours")
  assert.equal(thirty([at(0, 0.3), at(360, 0.3)]), "Fulfillment 30% now, problem for the next 15 days")
  assert.equal(thirty([at(0, 0.3), at(0, 1), at(360, 1)]), "Fulfillment 30% now, problem; watch within the hour")
  assert.equal(thirty([at(-360, 0.1), at(0, 0.82), at(360, 0.8)]), "Fulfillment 82% now, fine for the next 15 days")
  assert.equal(thirty([at(-360, 0.9), at(0, 0.62), at(96, 0.54), at(240, 0.4), at(360, 0.3)]), "Fulfillment 62% now, watch; problem in about 6 days")
})

test("a date label is the owner's day in the locale's short form, with no year, on a device in any zone", () =>
  everywhere(() => {
    const short = (y: number, m: number, d: number): string =>
      new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", timeZone: "UTC" }).format(new Date(Date.UTC(y, m, d, 12)))
    const t = Date.UTC(2026, 8, 13, 2) / 1000
    assert.equal(curve.dayOf(t, time.UTC), short(2026, 8, 13))
    assert.equal(curve.dayOf(t, { name: "America/New_York" }), short(2026, 8, 12), "two in the morning in UTC is the evening before in New York")
    assert.equal(curve.dayOf(t, time.UTC).includes("2026"), false)
  }))

test("of: nothing from now on is unsampled, and any is sampled in order", () => {
  const w = curve.thirtyDays(NOW)
  assert.deepEqual(curve.of(w, []), curve.unsampled)
  assert.deepEqual(curve.of(w, [{ t: NOW - H, v: 0.4 }]), curve.unsampled)
  const s = [{ t: NOW, v: 0.4 }, { t: NOW + 360 * H, v: 0.9 }]
  assert.deepEqual(curve.of(w, s), curve.sampled({ window: w, past: [], ahead: [s[0] ?? { t: 0, v: 0 }, ...s.slice(1)] }))
})
