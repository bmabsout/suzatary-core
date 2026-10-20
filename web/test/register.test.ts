// The ui tier's register: the frontier of its writes, joined by union, read in the discrete order's completion.

import { test } from "node:test"
import assert from "node:assert/strict"

import register from "../src/algebra/register"
import lo from "../src/algebra/lo"
import d from "../src/wire/decoder"
import ui from "../src/app/ui"
import kv from "../src/app/kv"
import { forAll, memoryStore, type Rng } from "./gen"

const REPLICAS = ["ada", "grace", "edsger"]

// Replicas that write and sync at random, each holding every write it has seen beside its register.
type Played = { readonly all: register.Write<number>[]; r: register.T<number> }
const play = (rng: Rng, steps = 12): Played[] => {
  const rs: Played[] = REPLICAS.map(() => ({ all: [], r: register.EMPTY }))
  for (let i = 0; i < steps; i += 1) {
    const at = rng.int(rs.length)
    const me = rs[at] as Played
    if (rng.bool(0.6)) {
      me.r = register.write(me.r, REPLICAS[at] as string, rng.int(4))
      me.all.push(...me.r)
    } else {
      const other = rs[rng.int(rs.length)] as Played
      me.all.push(...other.all)
      me.r = register.join(me.r, other.r)
    }
  }
  return rs
}
const some = (rng: Rng): register.T<number> => (play(rng)[rng.int(REPLICAS.length)] as Played).r

test("law: join is idempotent, commutative and associative, with EMPTY its unit", () =>
  forAll(300, r => {
    // Three replicas of one world: a dot names one write, so two worlds' registers never meet.
    const [a, b, c] = play(r, 20).map(p => p.r) as [register.T<number>, register.T<number>, register.T<number>]
    assert.deepEqual(register.join(a, a), a)
    assert.deepEqual(register.join(a, b), register.join(b, a))
    assert.deepEqual(register.join(register.join(a, b), c), register.join(a, register.join(b, c)))
    assert.deepEqual(register.join(register.EMPTY, a), a)
  }))

test("law: a register holds the frontier of every write it has seen, so reading commutes with union", () =>
  forAll(300, r => {
    for (const p of play(r, 20)) assert.deepEqual(p.r, register.join(register.EMPTY, p.all))
  }))

test("law: a write sees everything held, so it is the value, whatever was there", () =>
  forAll(300, r => {
    const v = r.int(9)
    assert.deepEqual(register.read(register.write(some(r), r.pick(REPLICAS), v)), { type: "Value", v })
  }))

test("law: two replicas writing without seeing each other read as a conflict holding both, never a loss", () =>
  forAll(300, r => {
    const base = some(r)
    const a = register.write(base, "ada", 1 + r.int(4))
    const b = register.write(base, "grace", 10 + r.int(4))
    const g = register.read(register.join(a, b))
    assert.deepEqual(register.candidates(g).slice().sort(), [register.one(register.read(a), 0), register.one(register.read(b), 0)].sort())
    assert.equal(g.type, "Conflict")
    // The next write, by either, descends from both and settles it.
    assert.equal(register.read(register.write(register.join(a, b), r.pick(["ada", "grace"]), 7)).type, "Value")
  }))

test("law: twins are one candidate; a stored write set parses back to itself, and junk parses to nothing", () =>
  forAll(200, r => {
    const base = some(r)
    assert.equal(register.read(register.join(register.write(base, "ada", 3), register.write(base, "grace", 3))).type, "Value")
    const x = some(r)
    assert.deepEqual(lo.withDefault(d.json(register.stored(d.num))(JSON.stringify(x)), null), x)
    assert.equal(register.stored(d.num)([{ by: "ada", n: 0, seen: {}, v: 1 }]).type, "Error")
    assert.equal(register.stored(d.num)({}).type, "Error")
  }))

const DRAFT = ui.kind<string>({ name: "draft", tier: "Owner", initial: "", value: d.str })

test("law: a synced draft typed on two devices reads as a conflict on both once they meet, never a loss", () =>
  forAll(100, r => {
    const [m1, m2] = [memoryStore(), memoryStore()]
    const phone = ui.over(kv.over(() => m1), "phone")
    const desk = ui.over(kv.over(() => m2), "desk")
    const [x, y] = [`a${r.int(99)}`, `b${r.int(99)}`]
    phone.cell(DRAFT, "c1").set(x)
    desk.cell(DRAFT, "c1").set(y)
    desk.receive(DRAFT, "c1", phone.held(DRAFT, "c1"))
    phone.receive(DRAFT, "c1", desk.held(DRAFT, "c1"))
    for (const d of [phone, desk]) assert.deepEqual(d.cell(DRAFT, "c1").read(), { type: "Conflict", vs: [y, x] })
    phone.cell(DRAFT, "c1").set(`${x}${y}`)
    desk.receive(DRAFT, "c1", phone.held(DRAFT, "c1"))
    assert.deepEqual(desk.cell(DRAFT, "c1").read(), { type: "Value", v: `${x}${y}` })
  }))

test("two tabs over one storage are two writers of one register: each reads what the other wrote", () => {
  const m = memoryStore()
  const [one, two] = [ui.over(kv.over(() => m), "one"), ui.over(kv.over(() => m), "two")]
  let heard = 0
  two.cell(DRAFT, "c").hear(() => (heard += 1))
  one.cell(DRAFT, "c").set("hello")
  two.heard("suzatary.ui.draft/c")
  assert.equal(heard, 1)
  assert.equal(two.cell(DRAFT, "c").get(), "hello")
  two.cell(DRAFT, "c").set("hello, Ada")
  assert.deepEqual(one.cell(DRAFT, "c").read(), { type: "Value", v: "hello, Ada" })
})

test("law: tabs interleaving at random never lose a write — each rests on what its writer saw, so a stale one conflicts", () =>
  forAll(300, r => {
    const m = memoryStore()
    const tabs = ["t1", "t2", "t3"].map(by => ({ by, ui: ui.over(kv.over(() => m), by), basis: register.EMPTY as register.T<string> }))
    // Every write made, with the past its writer passed: the only writes it may supersede.
    const made: { by: string; n: number; saw: register.Clock }[] = []
    const clock = (t: register.T<string>): register.Clock =>
      t.reduce<Record<string, number>>((c, w) => (Object.entries(w.seen).forEach(([k, n]) => (c[k] = Math.max(c[k] ?? 0, n))), c), {})
    for (let i = 0; i < 24; i += 1) {
      const t = tabs[r.int(tabs.length)] as (typeof tabs)[number]
      const c = t.ui.cell(DRAFT, "c")
      if (r.bool(0.3)) t.basis = c.seen()
      else {
        const saw = clock(t.basis)
        t.basis = c.write(t.basis, `${t.by}:${i}`)
        const w = t.basis[0] as register.Write<string>
        made.push({ by: w.by, n: w.n, saw })
      }
    }
    const frontier = tabs[0]?.ui.held(DRAFT, "c") ?? []
    for (const w of made) {
      const standing = frontier.some(f => f.by === w.by && f.n === w.n)
      const superseded = made.some(u => (u.saw[w.by] ?? 0) >= w.n && !(u.by === w.by && u.n === w.n))
      assert.ok(standing || superseded, `${w.by}#${w.n} was lost: no write that saw it superseded it`)
    }
  }))

test("law: cutting a frontier changes neither its reading nor its join with any write resting on it", () =>
  forAll(300, r => {
    const a = (play(r, 20)[r.int(REPLICAS.length)] as Played).r
    assert.deepEqual(register.read(register.cut(a)), register.read(a))
    const next = register.write(register.cut(a), "ada", 7, register.latest(a, "ada"))
    assert.deepEqual(register.read(register.join(register.cut(a), next)), register.read(register.join(a, next)))
  }))

test("a Device register's stored past names only the replicas its frontier holds, however many tabs wrote it", () => {
  const m = memoryStore()
  const theme = ui.kind<string>({ name: "look", tier: "Device", initial: "", value: d.str })
  for (let i = 0; i < 40; i += 1) ui.over(kv.over(() => m), `tab${i}`).cell(theme).set(`look ${i}`)
  const stored = JSON.parse(m.getItem("suzatary.ui.look/") ?? "[]") as { seen: Record<string, number> }[]
  assert.deepEqual(stored.map(w => Object.keys(w.seen)), [["tab39"]])
})
