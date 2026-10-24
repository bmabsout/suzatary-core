// The form: step is pure, a drawn form is a function of its value, a submit sends exactly the typed value, and a half-typed field survives a redraw.

import { test } from "node:test"
import assert from "node:assert/strict"

import form from "../src/view/form"
import ui from "../src/app/ui"
import parsed from "../src/algebra/parsed"
import price from "../src/todo/price"
import data from "../src/algebra/data"
import { forAll, rng } from "./gen"
import type { Rng } from "./gen"

let reads = 0

// Just enough of a DOM for a form: attributes, the properties a control holds, listeners, and children.
class FakeNode {
  parentNode: FakeElement | null = null
  text = ""
  // A page's root is connected, and so is everything under it; every read is counted, the work a keystroke costs.
  connected = false
  get isConnected(): boolean {
    reads += 1
    return this.connected || (this.parentNode?.isConnected ?? false)
  }
  get textContent(): string {
    return this.text
  }
}
class FakeElement extends FakeNode {
  readonly childNodes: FakeNode[] = []
  readonly attrs = new Map<string, string>()
  readonly listeners = new Map<string, ((e: unknown) => void)[]>()
  readonly classes = new Set<string>()
  hidden = false
  value = ""
  checked = false
  constructor(readonly tag: string) {
    super()
  }
  get className(): string {
    return [...this.classes].join(" ")
  }
  override get textContent(): string {
    return this.childNodes.map(c => c.textContent).join("")
  }
  override set textContent(t: string) {
    this.replaceChildren(t)
  }
  readonly classList = {
    toggle: (c: string, on: boolean) => void (on ? this.classes.add(c) : this.classes.delete(c)),
    contains: (c: string) => this.classes.has(c),
  }
  setAttribute(k: string, v: string): void {
    if (k === "class") v.split(" ").filter(c => c !== "").forEach(c => this.classes.add(c))
    else if (k === "hidden") this.hidden = true
    else if (k === "value") this.value = v
    else if (k === "checked") this.checked = true
    else this.attrs.set(k, v)
  }
  getAttribute(k: string): string | null {
    return this.attrs.get(k) ?? null
  }
  hasAttribute(k: string): boolean {
    return this.attrs.has(k)
  }
  removeAttribute(k: string): void {
    this.attrs.delete(k)
  }
  addEventListener(type: string, l: (e: unknown) => void): void {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), l])
  }
  fire(type: string): void {
    for (const l of this.listeners.get(type) ?? []) l({ preventDefault: () => {}, stopPropagation: () => {} })
  }
  appendChild(c: FakeNode): FakeNode {
    c.parentNode = this
    this.childNodes.push(c)
    return c
  }
  replaceChildren(...cs: (FakeNode | string)[]): void {
    this.childNodes.splice(0).forEach(c => (c.parentNode = null))
    cs.forEach(c => this.appendChild(typeof c === "string" ? Object.assign(new FakeNode(), { text: c }) : c))
  }
  focus(): void {}
}
Object.assign(globalThis, {
  Node: FakeNode,
  document: {
    createElement: (tag: string) => new FakeElement(tag),
    createElementNS: (_: string, tag: string) => new FakeElement(tag),
    createTextNode: (t: string) => Object.assign(new FakeNode(), { text: t }),
  },
})

// What a person sees of a node: its shape, the properties a control shows, and its text.
type Snap = { tag: string; cls: string; hidden: boolean; value: string; checked: boolean; pressed: string | null; text: string; kids: Snap[] }
const snap = (n: FakeNode): Snap =>
  n instanceof FakeElement
    ? { tag: n.tag, cls: n.className, hidden: n.hidden, value: n.value, checked: n.checked, pressed: n.getAttribute("aria-pressed"), text: "", kids: n.childNodes.map(snap) }
    : { tag: "#text", cls: "", hidden: false, value: "", checked: false, pressed: null, text: n.text, kids: [] }
const all = (n: FakeNode): FakeElement[] => (n instanceof FakeElement ? [n, ...n.childNodes.flatMap(all)] : [])

const look = (label: string): form.Look => ({ cls: label, label })
const OPTIONS = [{ value: "a", label: "A" }, { value: "b", label: "B" }, { value: "c", label: "C" }]
const blank = () => ({
  line: form.line(look("line")),
  lines: form.lines(look("lines"), 3),
  secret: form.secret(look("secret")),
  day: form.day(look("day"), "2026-01-01"),
  choice: form.choice("Pick one", OPTIONS),
  toggle: form.toggle(look("toggle")),
  price: form.priced(price.EMPTY),
})
type F = ReturnType<typeof blank>
type V = { [K in keyof F]: F[K]["v"] }
const NAMES = Object.keys(blank()) as (keyof F)[]

const words = (r: Rng): string => r.string("ab c", r.int(6))
const priceOf = (r: Rng): price.Form =>
  r.pick([
    () => price.flat(r.int(101) / 100),
    () => price.deadline(`2027-0${1 + r.int(9)}-1${r.int(10)}`, 1 + r.int(60)),
    () => price.recur({ days: 1 + r.int(90), from: r.int(101) / 100, to: r.int(101) / 100 }),
    () => price.periodic({ days: 1 + r.int(90), from: r.int(101) / 100, to: r.int(101) / 100 }),
  ])()
const valuesOf = (r: Rng): V => ({
  line: words(r),
  lines: words(r),
  secret: words(r),
  day: r.pick(["", "2027-03-04", "2026-12-31"]),
  choice: r.pick(["", "a", "b", "c"]),
  toggle: r.bool(),
  price: priceOf(r),
})
const valueOf = (r: Rng, name: keyof F): V[keyof F] => valuesOf(r)[name]
const typed = (r: Rng): form.Event<F> => {
  const name = r.pick(NAMES)
  return { type: "Typed", name, v: valueOf(r, name) } as form.Event<F>
}
const event = (r: Rng): form.Event<F> =>
  r.pick([
    () => typed(r),
    () => typed(r),
    () => typed(r),
    (): form.Event<F> => ({ type: "Refused", reason: `no: ${words(r)}`, about: r.pick([parsed.whole, parsed.field(r.pick(NAMES))]) }),
    (): form.Event<F> => ({ type: "Sent" }),
    (): form.Event<F> => ({ type: "Reset", to: form.of(form.fill(blank(), valuesOf(r))) }),
  ])()
const stateOf = (r: Rng): form.T<F> => r.array(r.int(8), () => event(r)).reduce(form.step, form.of(blank()))
const plain = (v: form.T<F>): string => JSON.stringify(v)

const settled = (): Promise<void> => new Promise(done => setTimeout(done, 0))
const forAllAsync = async (runs: number, check: (r: Rng) => Promise<void>): Promise<void> => {
  for (let seed = 1; seed <= runs; seed += 1) {
    await check(rng(seed * 2654435761)).catch((e: unknown) => {
      throw new Error(`seed ${seed}: ${e instanceof Error ? e.message : String(e)}`)
    })
  }
}

const quietly = { send: () => Promise.resolve(), say: () => {} }
const drawn = (m: form.Mounted<F>): Snap => {
  const root = new FakeElement("form")
  NAMES.forEach(k => root.appendChild(m.at(k) as unknown as FakeNode))
  root.appendChild(m.when("toggle", "when") as unknown as FakeNode)
  root.appendChild(m.said as unknown as FakeNode)
  return snap(root)
}
const mounted = (v: form.T<F>) => form.mount<F, F>(v, { read: parsed.ok, ...quietly })

test("law: step is pure — the same state and event give the same state, and neither is touched", () => {
  forAll(300, (r, seed) => {
    const s = stateOf(r)
    const e = event(r)
    const [before, said] = [plain(s), JSON.stringify(e)]
    assert.deepEqual(form.step(s, e), form.step(s, e), `seed ${seed}`)
    assert.equal(plain(s), before, `seed ${seed}: the state was not mutated`)
    assert.equal(JSON.stringify(e), said, `seed ${seed}: the event was not mutated`)
  })
})

test("law: a typed value lands in its field alone, and typing what is there wakes nobody", () => {
  forAll(300, (r, seed) => {
    const s = stateOf(r)
    const name = r.pick(NAMES)
    const v = valueOf(r, name)
    const next = form.step(s, { type: "Typed", name, v } as form.Event<F>)
    assert.deepEqual(next.fields[name].v, v, `seed ${seed}`)
    NAMES.filter(k => k !== name).forEach(k => assert.equal(next.fields[k], s.fields[k], `seed ${seed}: ${String(k)}`))
    assert.equal(form.step(next, { type: "Typed", name, v: next.fields[name].v } as form.Event<F>), next, `seed ${seed}`)
  })
})

test("law: sending leaves no secret in the value, and quiets a refusal", () => {
  forAll(200, (r, seed) => {
    const sent = form.step(stateOf(r), { type: "Sent" })
    assert.equal(sent.fields.secret.v, "", `seed ${seed}`)
    assert.deepEqual(sent.said, form.quiet, `seed ${seed}`)
  })
})

test("law: a drawn form is a function of its value, whatever it was drawn from before", () => {
  forAll(150, (r, seed) => {
    const m = mounted(form.of(blank()))
    r.array(r.int(10), () => event(r)).forEach(m.form.dispatch)
    assert.deepEqual(drawn(m), drawn(mounted(m.form.state())), `seed ${seed}`)
  })
})

test("law: a submit sends exactly the typed value, and a refusal sends nothing and is shown beside the button", async () => {
  await forAllAsync(100, async r => {
    const values = valuesOf(r)
    const sent: V[] = []
    const said: string[] = []
    const refuse = r.bool(0.3)
    const m = form.mount<F, V>(form.of(blank()), {
      read: f => (refuse ? parsed.refused("not like that") : parsed.ok(Object.fromEntries(NAMES.map(k => [k, f[k].v])) as V)),
      send: b => (sent.push(b), Promise.resolve()),
      say: s => void said.push(s),
    })
    const root = new FakeElement("form")
    NAMES.forEach(k => root.appendChild(m.at(k) as unknown as FakeNode))
    ;(["line", "lines", "secret", "day"] as const).forEach(k => {
      const input = m.at(k) as unknown as FakeElement
      input.value = values[k]
      input.fire("input")
    })
    NAMES.filter(k => !["line", "lines", "secret", "day"].includes(k)).forEach(k =>
      m.form.dispatch({ type: "Typed", name: k, v: values[k] } as form.Event<F>),
    )
    const button = m.button("Go", "go") as unknown as FakeElement
    button.fire("click")
    await settled()
    if (refuse) {
      assert.deepEqual(sent, [])
      assert.deepEqual(said, ["not like that"])
      assert.equal((m.said as unknown as FakeElement).hidden, false)
      assert.equal(m.said.textContent, "not like that")
      assert.equal(m.form.state().fields.secret.v, values.secret, "a refused secret is still the owner's to send")
      return
    }
    assert.deepEqual(sent, [values])
    assert.deepEqual(said, [])
    assert.equal(m.form.state().fields.secret.v, "")
    assert.equal((m.at("secret") as unknown as FakeElement).value, "", "a sent secret is not drawn again")
  })
})

test("a box's refusal is kept beside the button and said once", async () => {
  const said: string[] = []
  const m = form.mount<F, F>(form.of(blank()), { read: parsed.ok, send: () => Promise.reject(new Error("the box said no")), say: s => void said.push(s) })
  ;(m.button("Go", "go") as unknown as FakeElement).fire("click")
  await settled()
  assert.deepEqual(said, ["the box said no"])
  assert.equal(m.form.state().said.type, "Refused")
  assert.equal(m.said.textContent, "the box said no")
})

test("law: a half-typed value survives a redraw, in the same node", () => {
  forAll(150, (r, seed) => {
    const m = mounted(stateOf(r))
    const k = r.pick(["line", "lines", "day"] as const)
    const node = m.at(k) as unknown as FakeElement
    const half = `${words(r)}…`
    node.value = half
    node.fire("input")
    r.array(1 + r.int(6), () => event(r))
      .filter(e => e.type === "Refused" || (e.type === "Typed" && e.name !== k))
      .forEach(m.form.dispatch)
    assert.equal(m.at(k), node, `seed ${seed}`)
    assert.equal(node.value, half, `seed ${seed}`)
    assert.equal(m.form.state().fields[k].v, half, `seed ${seed}`)
  })
})

test("a value set without an input event, as autofill does, survives typing elsewhere and is what is sent", async () => {
  const sent: string[] = []
  const m = form.mount<F, string>(form.of(blank()), { read: f => parsed.ok(f.day.v), send: b => (sent.push(b), Promise.resolve()), say: () => {} })
  ;(m.at("day") as unknown as FakeElement).value = "2027-03-04"
  const line = m.at("line") as unknown as FakeElement
  line.value = "typed after"
  line.fire("input")
  assert.equal((m.at("day") as unknown as FakeElement).value, "2027-03-04")
  ;(m.button("Go", "go") as unknown as FakeElement).fire("click")
  await settled()
  assert.deepEqual(sent, ["2027-03-04"])
})

test("law: parsed is a monad, and a refusal passes through untouched", () => {
  forAll(200, (r, seed) => {
    const p = r.bool() ? parsed.ok(r.int(10)) : parsed.refused<number>(words(r))
    const f = (n: number) => (n % 3 === 0 ? parsed.refused<number>("three") : parsed.ok(n + 1))
    const g = (n: number) => parsed.ok(n * 2)
    assert.deepEqual(parsed.bind(parsed.ok(4), f), f(4), `seed ${seed}`)
    assert.deepEqual(parsed.bind(p, n => parsed.ok(n)), p, `seed ${seed}`)
    assert.deepEqual(parsed.bind(parsed.bind(p, f), g), parsed.bind(p, n => parsed.bind(f(n), g)), `seed ${seed}`)
    assert.deepEqual(parsed.map(p, n => n), p, `seed ${seed}`)
    if (p.type === "Refused") assert.equal(parsed.map<number, number>(p, n => n + 1), p, `seed ${seed}`)
  })
})

// Each law's own row, so one law's draft is never another's.
let rows = 0
const ROWS = form.drafts<F>("formtest")
const row = (): form.Draft<F> => form.draft(ROWS, String((rows += 1)))
const typedAll = (m: form.Mounted<F>, v: V): void => NAMES.forEach(name => m.form.dispatch({ type: "Typed", name, v: v[name] } as form.Event<F>))

// The page a law's forms are shown on: what it holds is what a person can type into.
const page = (): FakeElement => Object.assign(new FakeElement("main"), { connected: true })
const show = (on: FakeElement, m: form.Mounted<F>): void => on.replaceChildren(...NAMES.map(k => m.at(k) as unknown as FakeNode))
const mountOn = (d: form.Draft<F>, more: Partial<form.Spec<F, F>> = {}) => form.mount<F, F>(form.of(blank()), { read: parsed.ok, ...quietly, ...more, draft: d })

test("law: a form drawn again mounts on what was typed into the one shown, its secret excepted", () => {
  forAll(100, (r, seed) => {
    const [d, on] = [row(), page()]
    const v = valuesOf(r)
    const first = mountOn(d)
    show(on, first)
    typedAll(first, v)
    assert.deepEqual(mountOn(d).form.state().fields, form.fill(blank(), { ...v, secret: "" }), `seed ${seed}`)
  })
})

test("law: a form drawn again says what was said of the one shown, until it is sent again", async () => {
  await forAllAsync(50, async r => {
    const [d, on] = [row(), page()]
    const reason = `no: ${words(r)}`
    const refusing = { send: () => Promise.reject(new Error(reason)) }
    const first = mountOn(d, refusing)
    show(on, first)
    typedAll(first, valuesOf(r))
    const root = new FakeElement("form")
    root.appendChild(first.button("Send", "send") as unknown as FakeNode)
    const button = root.childNodes[0] as FakeElement
    for (const f of button.listeners.get("click") ?? []) f({ preventDefault: () => {}, stopPropagation: () => {}, target: button, currentTarget: button })
    await settled()
    await settled()
    const again = mountOn(d, refusing)
    assert.deepEqual(again.form.state(), first.form.state())
    assert.equal(again.form.state().said.type, "Refused")
    show(on, again)
    again.form.dispatch({ type: "Sent" })
    assert.deepEqual(mountOn(d).form.state().said, form.quiet)
  })
})

test("law: a form no longer shown writes nothing to the draft, and the one shown keeps what was typed", () => {
  forAll(100, (r, seed) => {
    const [d, on] = [row(), page()]
    const old = mountOn(d)
    show(on, old)
    const now = mountOn(d)
    show(on, now)
    const [v, w] = [valuesOf(r), valuesOf(r)]
    typedAll(now, v)
    const kept = now.form.state().fields
    typedAll(old, w)
    assert.deepEqual(now.form.state().fields, kept, `seed ${seed}: what the old form typed never reaches the one shown`)
    assert.deepEqual(mountOn(d).form.state().fields, form.fill(blank(), { ...v, secret: "" }), `seed ${seed}`)
  })
})

// A tap on a form's button, and the send it starts held until the law answers it.
const tap = (m: form.Mounted<F>): void => {
  const root = new FakeElement("form")
  root.appendChild(m.button("Send", "send") as unknown as FakeNode)
  const button = root.childNodes[0] as FakeElement
  for (const f of button.listeners.get("click") ?? []) f({ preventDefault: () => {}, stopPropagation: () => {}, target: button, currentTarget: button })
}
const held = () => {
  const answers: { ok: () => void; no: (e: Error) => void }[] = []
  const send = () => new Promise<void>((ok, no) => void answers.push({ ok, no }))
  return { send, answer: (refusal: string | null) => answers.splice(0).forEach(a => (refusal === null ? a.ok() : a.no(new Error(refusal)))) }
}
const typedIn = (v: V): form.Values<F> => ({ ...v, secret: "" })

test("law: a clearing form gives up what it sends as it sends it, and what is typed while the box answers stays, whatever it answers", async () => {
  await forAllAsync(60, async r => {
    const [d, on, box] = [row(), page(), held()]
    const sender = mountOn(d, { clears: true, send: box.send })
    show(on, sender)
    const sent = valuesOf(r)
    typedAll(sender, sent)
    tap(sender)
    assert.deepEqual(sender.form.state(), form.of(blank()), "what was sent left the fields as it was sent")
    // The page may draw the form again while the box answers, and a reading may land before the answer does.
    const shown = r.bool() ? sender : mountOn(d, { clears: true, send: box.send })
    show(on, shown)
    const next = valuesOf(r)
    typedAll(shown, next)
    const refusal = r.bool() ? null : `no: ${words(r)}`
    box.answer(refusal)
    await settled()
    await settled()
    // Typing that leaves every field as the send left it is no typing: a refusal hands what was sent back to it.
    const untouched = data.same(form.values(form.fill(blank(), typedIn(next))), form.values(blank()))
    const kept = refusal !== null && untouched ? sent : next
    assert.deepEqual(form.values(shown.form.state().fields), form.values(form.fill(blank(), typedIn(kept))), "what was typed since stays")
    assert.deepEqual(shown.form.state().said, refusal === null ? form.quiet : { type: "Refused", reason: refusal, about: parsed.whole })
  })
})

test("law: a refusal hands what was sent back to fields left as the send left them, its secret excepted, to the form shown", async () => {
  await forAllAsync(60, async r => {
    const [d, on, box] = [row(), page(), held()]
    const sender = mountOn(d, { clears: true, send: box.send })
    show(on, sender)
    const v = valuesOf(r)
    typedAll(sender, v)
    tap(sender)
    const shown = r.bool() ? sender : mountOn(d, { clears: true, send: box.send })
    show(on, shown)
    box.answer(`no: ${words(r)}`)
    await settled()
    await settled()
    assert.deepEqual(shown.form.state().fields, form.fill(blank(), typedIn(v)))
    assert.equal(shown.form.state().said.type, "Refused")
  })
})

// What a person and the page do to one row's forms, in any order: a draw that mounts a form and does not place it, a
// draw that mounts one and shows it, a visit away, a form put back after it was replaced, a keystroke into the form
// shown, and a send that clears.
type Move = "mount" | "show" | "leave" | "back" | "type" | "send"
const MOVES: readonly Move[] = ["mount", "show", "leave", "back", "type", "type", "type", "send"]
const BOUND = 64

test("law: whatever is drawn, left and put back, every keystroke into the form shown is its draft, at a bounded cost", async () => {
  for (const length of [10, 100, 800]) {
    await forAllAsync(20, async r => {
      const d = row()
      const cell = ui.cell(d.kind, d.key)
      const on = page()
      const forms: form.Mounted<F>[] = []
      let shown: form.Mounted<F> | null = null
      const put = (m: form.Mounted<F>): void => (show(on, m), void (shown = m))
      for (let i = 0; i < length; i++) {
        const move = r.pick(MOVES)
        if (move === "mount") forms.push(mountOn(d, { clears: true }))
        else if (move === "show") forms.push(mountOn(d, { clears: true })), put(forms[forms.length - 1] as form.Mounted<F>)
        else if (move === "leave") (on.replaceChildren(), (shown = null))
        else if (move === "back" && forms.length > 0) put(r.pick(forms))
        else if (move === "send" && shown !== null) {
          const root = new FakeElement("form")
          root.appendChild((shown as form.Mounted<F>).button("Send", "send") as unknown as FakeNode)
          const button = root.childNodes[0] as FakeElement
          for (const f of button.listeners.get("click") ?? []) f({ preventDefault: () => {}, stopPropagation: () => {}, target: button, currentTarget: button })
          await settled()
          await settled()
        } else if (move === "type" && shown !== null) {
          const m: form.Mounted<F> = shown
          reads = 0
          m.form.dispatch({ type: "Typed", name: "line", v: m.form.state().fields.line.v + r.pick(["a", "b", " "]) })
          assert.ok(reads <= BOUND, `length ${length}, move ${i}: a keystroke read the page ${reads} times`)
          assert.deepEqual(cell.get().values, form.values(m.form.state().fields), `length ${length}, move ${i}: the draft is what was typed`)
        }
      }
    })
  }
})

test("a refusal is said on the line of the field it is about, and on the form's line when that field has none or it is about the whole", async () => {
  const subjects: parsed.About[] = [parsed.field("line"), parsed.field("day"), parsed.whole]
  for (const about of subjects) {
    const [d, on] = [row(), page()]
    const m = mountOn(d, { read: () => parsed.refused("no", about) })
    const line = m.saidBy("line") as unknown as FakeElement
    const said = m.said as unknown as FakeElement
    show(on, m)
    tap(m)
    await settled()
    const at = about.type === "Field" && about.name === "line" ? line : said
    for (const l of [line, said]) {
      assert.equal(l.hidden, l !== at, JSON.stringify(about))
      assert.equal(l.textContent, l === at ? "no" : "", JSON.stringify(about))
    }
    // The box's own refusal is about the whole.
    m.form.dispatch({ type: "Refused", reason: "the box said no", about: parsed.whole })
    assert.deepEqual([line.hidden, said.hidden, said.textContent], [true, false, "the box said no"])
  }
})
