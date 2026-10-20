// The composer empties when a draft leaves: a send that answers late never clears what was typed after the click,
// and a failed send comes back only into an empty box.

import { test } from "node:test"
import assert from "node:assert/strict"

import { mountComposer } from "../src/chat/composer"
import type { Composer } from "../src/chat/composer"
import again from "../src/chat/again"
import drafts from "../src/chat/drafts"
import { parse, print } from "../src/chat/refs"
import type { Reference } from "../src/chat/refs"
import type { TurnRow } from "../src/wire/generated/wire"
import app from "../src/app/app"
import time from "../src/app/time"
import ui from "../src/app/ui"
import kv from "../src/app/kv"
import cast from "./cast"
import { memoryStore, rng } from "./gen"
import type { Rng } from "./gen"

// Just enough of a DOM for the composer: attributes, a control's value, listeners and children.
class FakeNode {
  parentNode: FakeElement | null = null
  text = ""
  get textContent(): string {
    return this.text
  }
}
class FakeElement extends FakeNode {
  readonly childNodes: FakeNode[] = []
  readonly attrs = new Map<string, string>()
  readonly listeners = new Map<string, ((e: unknown) => void)[]>()
  readonly style: Record<string, string> = {}
  readonly scrollHeight = 0
  hidden = false
  disabled = false
  value = ""
  constructor(readonly tag: string) {
    super()
  }
  readonly classList = { toggle: () => {} }
  setAttribute(k: string, v: string): void {
    if (k === "hidden") this.hidden = true
    else this.attrs.set(k, v)
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
  override get textContent(): string {
    return this.childNodes.map(c => c.textContent).join("")
  }
  focus(): void {}
  click(): void {}
  setSelectionRange(): void {}
}
const toasted = new FakeElement("div")
Object.assign(globalThis, {
  Node: FakeNode,
  document: {
    createElement: (tag: string) => new FakeElement(tag),
    createElementNS: (_: string, tag: string) => new FakeElement(tag),
    createTextNode: (t: string) => Object.assign(new FakeNode(), { text: t }),
    getElementById: (id: string) => (id === "toast" ? toasted : null),
  },
  window: { innerWidth: 1200, setTimeout: () => 0, clearTimeout: () => {} },
})

const all = (n: FakeNode): FakeElement[] => (n instanceof FakeElement ? [n, ...n.childNodes.flatMap(all)] : [])
const one = (root: FakeNode, cls: string): FakeElement => {
  const found = all(root).find(e => (e.attrs.get("class") ?? "").split(" ").includes(cls))
  assert.ok(found !== undefined, cls)
  return found
}

type Said = { text: string; references: readonly string[] }
// A send the test answers by hand, whenever it likes.
type Pending = Said & { answer: () => void; fail: (e: Error) => void }

const mounted = () => {
  app.dispatch({ type: "Met", me: { type: "Writes", login: cast.world.owner.email, actor: cast.world.owner.actor }, served: [], categories: [], zone: time.UTC })
  const m = memoryStore()
  const draft = drafts.of(ui.over(kv.over(() => m), "wide"), "c")
  const sends: Pending[] = []
  const edits: Said[] = []
  const pending = (text: string, references: readonly Reference[]): Promise<void> =>
    new Promise((answer, fail) => sends.push({ text, references: references.map(print), answer: () => answer(), fail }))
  const composer: Composer = mountComposer({
    seeded: [],
    focus: false,
    onSend: pending,
    onStop: () => "idle",
    again: {
      offer: () => again.none,
      edit: (text, references) => (edits.push({ text, references: references.map(print) }), pending(text, references)),
    },
    draft,
  })
  const root = composer.root as unknown as FakeElement
  const box = one(root, "composebox")
  const type = (text: string): void => {
    box.value = text
    box.fire("input")
  }
  const send = (): void => one(root, "composesend").fire("click")
  const chips = (): string[] => all(one(root, "composechips")).filter(e => e.tag === "span").map(e => e.childNodes[0]?.textContent ?? "")
  const saved = (): string => {
    const g = draft.read()
    return g.type === "Value" ? g.v : g.type === "Unwritten" ? "" : "(conflict)"
  }
  return { composer, root, box, type, send, sends, edits, chips, saved }
}

// `forAll`, awaiting each seed's run: a send answers on a later turn of the loop.
const forAllLater = async (runs: number, check: (r: Rng) => Promise<void>): Promise<void> => {
  for (let seed = 1; seed <= runs; seed += 1)
    await check(rng(seed * 2654435761)).catch((e: unknown) => {
      throw new Error(`seed ${seed}: ${e instanceof Error ? e.message : String(e)}`)
    })
}

const settled = (): Promise<void> => new Promise(done => setTimeout(done, 0))
const words = (r: Rng, n: number): string => `question ${n} ${r.string("abc ", 1 + r.int(8))}x`
const reference = (r: Rng): Reference => {
  const parsed = parse(`todo:life/${r.pick(Object.values(cast.world.todos)).id}`)
  assert.ok(parsed !== null)
  return parsed
}

test("law: a sent draft leaves the box at the click, and an answer however late never clears what was typed after it", () =>
  forAllLater(50, async r => {
    const c = mounted()
    const first = words(r, 1)
    const chip = reference(r)
    c.type(first)
    c.composer.attach(chip)
    c.send()
    assert.deepEqual(c.sends.map(s => [s.text, s.references]), [[first, [print(chip)]]], "the draft as it stood, sent once")
    assert.equal(c.box.value, "", "the box is empty at once")
    assert.deepEqual(c.chips(), [], "the chips went with it")
    assert.equal(c.saved(), "", "and the sent words are no saved draft")
    const next = words(r, 2)
    c.type(next)
    const sent = c.sends[0]
    assert.ok(sent !== undefined)
    if (r.bool()) sent.answer()
    else sent.fail(new Error("the box said no"))
    await settled()
    assert.equal(c.box.value, next, "what was typed after the click stands")
    assert.equal(c.saved(), next)
    c.send()
    assert.deepEqual(c.sends.map(s => s.text), [first, next], "and it is the next send")
    c.composer.destroy()
  }))

test("law: a failed send comes back, words, chips and saved draft, only into an empty box", () =>
  forAllLater(50, async r => {
    const c = mounted()
    const first = words(r, 1)
    const chip = reference(r)
    c.type(first)
    c.composer.attach(chip)
    c.send()
    c.sends[0]?.fail(new Error("the box said no"))
    await settled()
    assert.equal(c.box.value, first)
    assert.deepEqual(c.chips(), [print(chip)])
    assert.equal(c.saved(), first, "an unsent draft is a saved draft again")
    assert.equal(toasted.textContent, "the box said no", "the failure is said")
    c.composer.destroy()
  }))

test("law: a send that fails after the composer is gone hands its words back to the conversation's draft", () =>
  forAllLater(50, async r => {
    const c = mounted()
    const first = words(r, 1)
    c.type(first)
    c.send()
    c.composer.destroy()
    c.sends[0]?.fail(new Error("the box said no"))
    await settled()
    assert.equal(c.saved(), first, "the next composer of this conversation opens on it")
  }))

test("a send that lands leaves the box empty, and nothing comes back", async () => {
  const c = mounted()
  c.type("where is the form")
  c.send()
  c.sends[0]?.answer()
  await settled()
  assert.equal(c.box.value, "")
  assert.equal(c.saved(), "")
  c.composer.destroy()
})

test("a blank box sends nothing", () => {
  const c = mounted()
  c.type("   ")
  c.send()
  assert.deepEqual(c.sends, [])
  c.composer.destroy()
})

test("a reworded question leaves Editing when sent, and a failed one comes back still being edited", async () => {
  const c = mounted()
  const question: TurnRow = {
    index: 0,
    who: "person",
    verb: "",
    text: "where is the form",
    refused: false,
    references: [],
    attachments: [],
    queued: false,
    blocks: [],
    at: "2026-01-01T00:00:00",
    remembered: null,
    attempt: "current",
    gone: null,
  }
  const editing = one(c.root, "composeediting")
  c.composer.edit(question)
  assert.equal(editing.hidden, false)
  c.type("where is the new form")
  c.send()
  assert.deepEqual(c.edits.map(e => e.text), ["where is the new form"], "sent as the question reworded")
  assert.equal(editing.hidden, true, "the box is asking again at once")
  c.sends[0]?.fail(new Error("the box said no"))
  await settled()
  assert.equal(c.box.value, "where is the new form")
  assert.equal(editing.hidden, false, "and resending it rewords the question again")
  c.send()
  assert.equal(c.edits.length, 2)
  c.composer.destroy()
})
