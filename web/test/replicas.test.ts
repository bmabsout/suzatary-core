// The tab's replicas of the inbox and the conversations read as the box reads the same objects, byte for byte,
// whatever order the objects arrived in and however a sync was cut, and two tabs that hold the same objects read alike.

import { test } from "node:test"
import assert from "node:assert/strict"
import { createRequire } from "node:module"
import { readFileSync } from "node:fs"

import type { Core, Named } from "../src/prodrome/core"
import { chainsOf, erasedOf, keptChains, objectsOf, secretaryOf } from "../src/prodrome/core"
import { parentsOf } from "../src/prodrome/replica"
import type { MemoryPayload } from "../src/wire/generated/wire"
import { forAll, type Rng } from "./gen"
import { heldChain } from "./held"

const core = ((): Core => {
  const built = process.env["SUZATARY_PRODROME_WASM"]
  if (built === undefined || built === "") throw new Error("SUZATARY_PRODROME_WASM is unset — run scripts/web-test.sh")
  return createRequire(import.meta.url)(`${built}/nodejs/prodrome.js`) as Core
})()

type Shipped = { hash: string; text: string }
type Asked = { id: string; before: string | null; limit: string | null; about: string | null; memory: boolean; answer: string }
const box = JSON.parse(readFileSync("app/core/tests/web/replicas.json", "utf8")) as {
  at: string
  chains: { name: string; untrusted: string[]; objects: Shipped[] }[]
  inbox: Shipped[]
  conversations: Shipped[]
  keys: Record<string, string>
  stale: Record<string, string>
  erased: Record<string, string[]>
  memory: MemoryPayload
  list: string
  proposals: string
  windows: Asked[]
  searches: { q: string; answer: string }[]
}
const named = (xs: readonly Shipped[]): Named[] => xs.map(o => ({ hash: o.hash, literal: o.text }))
const inbox = named(box.inbox)
const conversations = named(box.conversations)
const chains = box.chains.map(c => ({ name: c.name, untrusted: c.untrusted, objects: named(c.objects), heads: [] }))

/** Every answer the box gave, read by the module from these objects: the raw strings, so equal means byte for byte. */
const answers = (i: readonly Named[], c: readonly Named[], held = chains, given: Record<string, string> = box.keys): string[] => {
  const keys = JSON.stringify(given)
  const p = new core.Proposals(objectsOf(i))
  const t = new core.Conversations(objectsOf(c))
  const o = t.opened(keys)
  const priced = new core.Chains()
  try {
    for (const h of held) priced.hold(h.name, objectsOf(h.objects), JSON.stringify(h.untrusted))
    const memory = JSON.stringify(box.memory)
    return [
      p.inbox(box.at, priced, memory),
      o.list(p),
      ...box.windows.map(w =>
        o.conversation(w.id, p, w.before ?? undefined, w.limit ?? undefined, w.about ?? undefined, w.memory ? memory : undefined)),
      ...box.searches.map(s => o.search(s.q)),
    ]
  } finally {
    priced.free()
    o.free()
    t.free()
    p.free()
  }
}
const expected = [box.proposals, box.list, ...box.windows.map(w => w.answer), ...box.searches.map(s => s.answer)]

/** A random order the objects may arrive in by sync: every object after the objects it rests on. */
const causal = (r: Rng, objects: readonly Named[]): Named[] => {
  const held = new Set<string>()
  const left = [...objects]
  const out: Named[] = []
  while (left.length > 0) {
    const ready = left.filter(o => parentsOf(o).every(p => held.has(p)))
    const next = r.pick(ready)
    out.push(next)
    held.add(next.hash)
    left.splice(left.indexOf(next), 1)
  }
  return out
}

/** A sync cut into rounds: each round's replica is the union so far, so every prefix is a down-set. */
const rounds = (r: Rng, objects: readonly Named[]): Named[][] => {
  const cuts = [...new Set(r.array(r.int(4), () => r.int(objects.length + 1)))].sort((a, b) => a - b)
  return [...cuts, objects.length].map(n => objects.slice(0, n))
}

const shuffled = <A>(r: Rng, xs: readonly A[]): A[] => {
  const out = [...xs]
  for (let i = out.length - 1; i > 0; i -= 1) {
    const j = r.int(i + 1)
    ;[out[i], out[j]] = [out[j] as A, out[i] as A]
  }
  return out
}

test("the module reads the box's objects as the box does, byte for byte", () => {
  assert.deepEqual(answers(inbox, conversations), expected)
})

test("an erased slot reads as erased whatever key a tab still holds, and the tab forgets what the box's sweep does", () => {
  assert.deepEqual(answers(inbox, conversations, chains, box.stale), expected, "a stale key opens nothing an erasure names")
  const lines = new core.Conversations(objectsOf(conversations))
  try {
    assert.deepEqual(erasedOf(lines, box.stale, []), box.erased)
    assert.deepEqual(erasedOf(lines, box.keys, []), Object.fromEntries(Object.keys(box.erased).map(k => [k, []])), "the box gives no key an erasure names")
  } finally {
    lines.free()
  }
  const deed = JSON.parse(expected[2] ?? "") as { turns: { index: number; text: string; gone: unknown }[] }
  assert.deepEqual(deed.turns.find(t => t.index === 1), { ...deed.turns.find(t => t.index === 1), text: "[Ada erased these words]", gone: { why: "asked", by: "ada" } })
})

test("a reading is a function of the object set: any order, any duplicate", () => {
  forAll(40, r => {
    const twice = (xs: readonly Named[]) => shuffled(r, [...xs, ...xs.filter(() => r.bool(0.3))])
    const held = chains.map(c => ({ ...c, objects: twice(c.objects) }))
    assert.deepEqual(answers(twice(inbox), twice(conversations), held), expected)
  })
})

test("a sync cut anywhere reads, and the last round reads as the box", () => {
  forAll(25, r => {
    const i = rounds(r, causal(r, inbox))
    const c = rounds(r, causal(r, conversations))
    // The two stores sync apart, so a conversation can name a proposal the inbox has not brought yet: that refusal is
    // the reading's, and any other is a bug.
    for (const held of i) for (const lines of c.slice(0, 2)) {
      try {
        answers(held, lines)
      } catch (error: unknown) {
        assert.match(String(error), /does not hold|no conversation named/)
      }
    }
    assert.deepEqual(answers(i.at(-1) ?? [], c.at(-1) ?? []), expected)
  })
})

test("two tabs that took the same objects in different rounds read alike", () => {
  forAll(25, r => {
    const one = answers(causal(r, inbox), causal(r, conversations))
    const two = answers(shuffled(r, inbox), causal(r, conversations).reverse())
    assert.deepEqual(two, one)
    assert.deepEqual(one, expected)
  })
})

test("a proposal is priced by the todo it serves, and by nothing where the tab holds no chain", () => {
  const values = (held: typeof chains) =>
    (JSON.parse(answers(inbox, conversations, held)[0] ?? "") as { proposals: { serves: string | null; value: unknown }[] }).proposals
      .filter(p => p.serves !== null)
      .map(p => p.value)
  assert.ok(values(chains).length > 0 && values(chains).every(v => typeof v === "number"), "a served todo prices it")
  assert.ok(values([]).every(v => v === "absent"), "a chain not held reads ∅")
})

test("the decoded readings are the wire's shapes", () => {
  const lines = new core.Conversations(objectsOf(conversations))
  const s = secretaryOf(core, inbox, lines, box.keys)
  const held = chainsOf(core, chains.map(c => heldChain(c.name, c.objects, c.untrusted)))
  try {
    const listed = s.inbox(box.at, box.memory, held).proposals.map(p => p.headline)
    assert.equal(listed.length, 5)
    assert.ok(listed.includes(`remember (person): ${box.memory.cards[0]?.fact}`), "a kept card's words")
    assert.ok(listed.includes("remember (person): its words are deleted"), "a dismissed card's are gone")
    assert.ok(s.inbox(box.at, null, held).proposals.some(p => p.headline === "remember (person)"), "no memory, no words")
    assert.deepEqual(s.list().conversations.map(c => [c.id, c.unplaced.length]), [["trains", 0], ["lagging", 2], ["about-the-deed", 0]])
    const lagging = s.conversation("lagging", {}, null)
    assert.deepEqual(lagging.unplaced, [{ index: 0, why: "notHeld" }, { index: 1, why: "unasked" }], "lines the inbox cannot place, with why")
    assert.deepEqual(lagging.proposals.map(p => p.index), [2], "the placed one keeps its card and its number")
    const trains = s.conversation("trains", {}, box.memory)
    assert.deepEqual(trains.turns.flatMap(t => (t.remembered === null ? [] : [t.remembered])), [
      { state: "standing", fact: 1, text: "Takes the early train" },
    ])
    assert.deepEqual(
      s.conversation("trains", {}, null).turns.flatMap(t => (t.remembered === null ? [] : [t.remembered.state])),
      ["unread"],
    )
    assert.ok(trains.turns.flatMap(t => t.blocks).every(b => b.html === null), "the module renders nothing")
    const words = (memory: MemoryPayload | null) =>
      s.conversation("trains", {}, memory).proposals.flatMap(p => (p.act.kind === "remember" ? [p.act.words.state] : []))
    assert.deepEqual(words(box.memory), ["kept", "gone"], "a card's words as the memory keeps them")
    assert.deepEqual(words(null), ["unread", "unread"], "and none without it")
    assert.throws(() => s.conversation("nowhere", {}, null), /no conversation named/)
  } finally {
    held.free()
    s.free()
    lines.free()
  }
})

// The agent completes the deed's todo: trusted, the completion binds and the function goes flat; untrusted, it does not.
test("a chain whose policy alone moved is held again, and links as a fresh set under that policy", () => {
  const served = { kind: "refIn", store: "life", entity: "email-harrow" }
  const completed = chains.map(c => {
    if (c.name !== "life") return c
    const t = new core.Todos(objectsOf(c.objects))
    try {
      const written = JSON.parse(t.append(core.lifecycle("Completed", "email-harrow", "2026-09-01T12:00:00", "triage", ""), null)) as { objects: Shipped[] }
      return { ...c, objects: [...c.objects, ...named(written.objects)] }
    } finally {
      t.free()
    }
  })
  const linkedIn = (set: ReturnType<typeof chainsOf>): string => {
    const l = set.linker("{}", box.at)
    try {
      return l.link(JSON.stringify(served))
    } finally {
      l.free()
    }
  }
  const fresh = (untrusted: string[]): string => {
    const set = chainsOf(core, completed.map(c => heldChain(c.name, c.objects, untrusted)))
    try {
      return linkedIn(set)
    } finally {
      set.free()
    }
  }
  const kept = keptChains(core)
  try {
    for (const untrusted of [[], ["triage"], [], ["triage"]]) {
      assert.equal(linkedIn(kept(completed.map(c => heldChain(c.name, c.objects, untrusted))).chains), fresh(untrusted), `under [${untrusted.join(", ")}]`)
    }
    assert.notEqual(fresh([]), fresh(["triage"]), "the policy moves what the todo links to")
  } finally {
    kept([]).chains.free()
  }
})
