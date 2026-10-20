// The frame's islands: mounted once with the page, handed every value, destroyed with it, and never touched by a body redraw.

import { test } from "node:test"
import assert from "node:assert/strict"

import reading from "../src/algebra/reading"
import frame from "../src/view/frame"
import placeholder from "../src/view/placeholder"
import { rng } from "./gen"
import type { Rng } from "./gen"

// Just enough of a DOM for the frame: a tree of nodes that counts every time one is taken out of its parent.
class FakeNode {
  parentNode: FakeNode | null = null
  readonly childNodes: FakeNode[] = []
  detached = 0
  text = ""
  get firstChild(): FakeNode | null {
    return this.childNodes[0] ?? null
  }
  get nextSibling(): FakeNode | null {
    const at = this.parentNode?.childNodes ?? []
    return at[at.indexOf(this) + 1] ?? null
  }
  get textContent(): string {
    return this.text + this.childNodes.map(c => c.textContent).join("")
  }
  set textContent(t: string) {
    for (const c of [...this.childNodes]) this.removeChild(c)
    this.text = t
  }
  removeChild(c: FakeNode): FakeNode {
    this.childNodes.splice(this.childNodes.indexOf(c), 1)
    c.parentNode = null
    c.detached += 1
    return c
  }
  insertBefore(c: FakeNode, before: FakeNode | null): FakeNode {
    c.parentNode?.removeChild(c)
    const i = before === null ? this.childNodes.length : this.childNodes.indexOf(before)
    this.childNodes.splice(i, 0, c)
    c.parentNode = this
    return c
  }
  appendChild(c: FakeNode): FakeNode {
    return this.insertBefore(c, null)
  }
  replaceChildren(...cs: (FakeNode | string)[]): void {
    for (const c of [...this.childNodes]) this.removeChild(c)
    for (const c of cs) this.appendChild(typeof c === "string" ? Object.assign(new FakeNode(), { text: c }) : c)
  }
}
class FakeElement extends FakeNode {
  hidden = false
  className = ""
  value = ""
  constructor(readonly tag: string) {
    super()
  }
  setAttribute(k: string, v: string): void {
    if (k === "class") this.className = v
  }
  readonly classList = {
    toggle: (c: string, on: boolean): void => {
      const cs = this.className.split(" ").filter(x => x !== "" && x !== c)
      this.className = [...cs, ...(on ? [c] : [])].join(" ")
    },
  }
  addEventListener(): void {}
}
Object.assign(globalThis, {
  Node: FakeNode,
  document: {
    createElement: (tag: string) => new FakeElement(tag),
    createElementNS: (_: string, tag: string) => new FakeElement(tag),
    createTextNode: (t: string) => Object.assign(new FakeNode(), { text: t }),
  },
})

const settled = (): Promise<void> => new Promise(done => setTimeout(done, 0))
// `forAll`, for a check that waits on what the page reads.
const forAllAsync = async (runs: number, check: (r: Rng) => Promise<void>): Promise<void> => {
  for (let seed = 1; seed <= runs; seed += 1) {
    await check(rng(seed * 2654435761)).catch((e: unknown) => {
      throw new Error(`seed ${seed}: ${e instanceof Error ? e.message : String(e)}`)
    })
  }
}
const all = (n: FakeNode): FakeNode[] => [n, ...n.childNodes.flatMap(all)]

type Seen = { mounts: number; updates: number[]; destroys: number; node: FakeElement }

// A page over a number whose own events add to it, with one island that records what it is handed.
const counter = (seen: Seen) => {
  let send: (e: number) => void = () => {}
  const page: frame.Page<number, number, "draft"> = {
    className: "counter",
    title: "Counter",
    reads: () => reading.constant(0),
    step: (v, e) => v + e,
    listen: dispatch => ((send = dispatch), () => {}),
    islands: () => {
      seen.mounts += 1
      return { draft: { node: seen.node as unknown as Node, update: v => void seen.updates.push(v), destroy: () => void (seen.destroys += 1) } }
    },
    view: (v, _, __, at) => [String(v), at.draft, String(-v)],
  }
  return { page, dispatch: (e: number) => send(e) }
}

const fresh = (): Seen => ({ mounts: 0, updates: [], destroys: 0, node: new FakeElement("textarea") })

test("law: an island is mounted once, handed every value in order, and destroyed with the page", async () => {
  await forAllAsync(100, async r => {
    const seen = fresh()
    const { page, dispatch } = counter(seen)
    const m = frame.open(new FakeElement("div") as unknown as HTMLElement, page)
    assert.equal(seen.mounts, 1, "mounted with the page, before its value")
    await settled()
    const es = r.array(r.int(30), () => r.int(5))
    for (const e of es) dispatch(e)
    const values = es.reduce<number[]>((vs, e) => (e === 0 ? vs : [...vs, (vs.at(-1) ?? 0) + e]), [0])
    assert.equal(seen.mounts, 1)
    assert.deepEqual(seen.updates, values)
    m.destroy()
    dispatch(1)
    assert.equal(seen.destroys, 1)
    assert.deepEqual(seen.updates, values, "nothing is handed to an island after its page is gone")
  })
})

test("law: a body redraw never takes an island's node out of the page, and leaves what the DOM holds in it", async () => {
  await forAllAsync(100, async r => {
    const seen = fresh()
    const { page, dispatch } = counter(seen)
    const root = new FakeElement("div") as unknown as HTMLElement
    frame.open(root, page)
    await settled()
    seen.node.value = "half typed"
    const placed = seen.node.parentNode
    const detached = seen.node.detached
    for (const e of r.array(1 + r.int(20), () => 1 + r.int(5))) dispatch(e)
    assert.ok(placed !== null && seen.node.parentNode === placed)
    assert.equal(seen.node.detached, detached)
    assert.equal(seen.node.value, "half typed")
    const v = seen.updates.at(-1) ?? 0
    assert.deepEqual(placed.childNodes.map(c => (c === seen.node ? "island" : c.textContent)), [String(v), "island", String(-v)])
    assert.ok(all(root as unknown as FakeNode).includes(seen.node))
  })
})

test("law: own events heard before the first value are folded into it when it lands, in order", async () => {
  await forAllAsync(100, async r => {
    const seen = fresh()
    let send: (e: number) => void = () => {}
    let land: (v: number) => void = () => {}
    const page: frame.Page<number, number, "draft"> = {
      className: "late",
      title: "Late",
      reads: () => reading.once(() => new Promise<number>(done => (land = done))),
      step: (v, e) => v * 10 + e,
      listen: dispatch => ((send = dispatch), () => {}),
      islands: () => ({ draft: { node: seen.node as unknown as Node, update: v => void seen.updates.push(v) } }),
      view: (v, _, __, at) => [String(v), at.draft],
    }
    frame.open(new FakeElement("div") as unknown as HTMLElement, page)
    const es = r.array(r.int(6), () => r.int(10))
    for (const e of es) send(e)
    assert.deepEqual(seen.updates, [], "nothing is drawn before the value")
    const v = r.int(10)
    land(v)
    await settled()
    assert.deepEqual(seen.updates, [es.reduce((a, e) => a * 10 + e, v)])
  })
})

test("law: a change to what is behind alone draws the status line, never the body", async () => {
  await forAllAsync(50, async r => {
    let behind: reading.Behind | null = null
    const ears = new Set<() => void>()
    let views = 0
    const page: frame.Page<number> = {
      className: "behind",
      title: "Behind",
      reads: () => ({ ...reading.constant(7), hear: e => (ears.add(e), () => void ears.delete(e)), behind: () => behind }),
      view: v => ((views += 1), [String(v)]),
    }
    const root = new FakeElement("div")
    frame.open(root as unknown as HTMLElement, page)
    await settled()
    const drawn = views
    for (const slow of r.array(1 + r.int(10), () => r.bool())) {
      behind = slow ? { why: r.bool() ? null : "the box did not answer", since: 1_791_000_000_000 } : null
      for (const e of [...ears]) e()
    }
    assert.equal(views, drawn, "the body was not drawn again")
    const said = all(root).map(n => n.textContent).join(" ")
    assert.equal(behind === null ? !/As of|Could not refresh/.test(said) : /As of|Could not refresh/.test(said), true)
  })
})

test("law: while a page waits it draws its placeholder in its own shape, with no words, the same node however often it is drawn", async () => {
  await forAllAsync(50, async r => {
    let behind: reading.Behind | null = null
    let land: (v: number) => void = () => {}
    const ears = new Set<() => void>()
    const once = reading.once(() => new Promise<number>(done => (land = done)))
    const page: frame.Page<number> = {
      className: "waits",
      title: "",
      shape: placeholder.ROWS,
      reads: () => ({ ...once, hear: e => (ears.add(e), once.hear(e)), behind: () => behind }),
      view: v => [String(v)],
    }
    const root = new FakeElement("div")
    frame.open(root as unknown as HTMLElement, page)
    const waiting = (): FakeElement[] => all(root).filter((n): n is FakeElement => n instanceof FakeElement && n.className.startsWith("placeholder"))
    const [shown] = waiting()
    assert.ok(shown !== undefined && shown.className === "placeholder ph-rows", "the shape the page names")
    assert.equal(all(root).map(n => n.textContent).join(""), "", "no sentence says it is loading")
    for (const slow of r.array(1 + r.int(8), () => r.bool())) {
      behind = slow ? { why: null, since: 1_791_000_000_000 } : null
      for (const e of [...ears]) e()
    }
    assert.equal(shown.detached, 0, "drawn again, the placeholder is never taken out, so its 300 ms do not start again")
    land(r.int(9))
    await settled()
    assert.deepEqual(waiting(), [], "gone once the value is here")
  })
})

test("law: a value that still waits keeps the placeholder, the same node from the first draw until it has something to show", async () => {
  await forAllAsync(50, async r => {
    let land: (v: number) => void = () => {}
    let send: (e: number) => void = () => {}
    const page: frame.Page<number, number> = {
      className: "asks",
      title: "",
      shape: placeholder.CARDS,
      // Below ten the box is still asked; the page has nothing of its own to show.
      waits: v => v < 10,
      reads: () => reading.once(() => new Promise<number>(done => (land = done))),
      step: (v, e) => v + e,
      listen: dispatch => ((send = dispatch), () => {}),
      view: v => (v < 10 ? [] : [String(v)]),
    }
    const root = new FakeElement("div")
    frame.open(root as unknown as HTMLElement, page)
    const waiting = (): FakeElement[] => all(root).filter((n): n is FakeElement => n instanceof FakeElement && n.className.startsWith("placeholder"))
    const column = (): string => (root.childNodes[0] as FakeElement).className
    const [shown] = waiting()
    assert.ok(shown !== undefined && column().split(" ").includes("pagewaiting"))
    land(r.int(5))
    await settled()
    for (const e of r.array(r.int(4), () => r.int(2))) send(e)
    assert.deepEqual(waiting(), [shown], "the value waits: the same node, never a second")
    assert.equal(shown.detached, 0)
    send(10)
    assert.deepEqual(waiting(), [], "gone once the value has something to show")
    assert.ok(!column().split(" ").includes("pagewaiting"), "and the title waits no more")
  })
})

test("a page that failed to load is not waiting, so its title shows no bar", async () => {
  const page: frame.Page<number> = {
    className: "fails",
    title: "",
    reads: () => reading.once(() => Promise.reject(new Error("down"))),
    view: v => [String(v)],
  }
  const root = new FakeElement("div")
  frame.open(root as unknown as HTMLElement, page)
  await settled()
  await settled()
  assert.ok(!(root.childNodes[0] as FakeElement).className.split(" ").includes("pagewaiting"))
  assert.match(all(root).map(n => n.textContent).join(""), /Could not load: down/)
})
