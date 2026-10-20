// A reading is total over what a replica holds (Prodrome SPEC law 44): a print that does not parse at its store's
// schema, as an inbox card from before `Remember` carried its `text` did on the owner's phone, is left out with what
// rests on it, every other reading answers as the box does, and the line says what was left out.

import { test } from "node:test"
import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { createRequire } from "node:module"
import { readFileSync } from "node:fs"

import type { Core, Named, Secretary } from "../src/prodrome/core"
import { entriesOf, keptChains, objectsOf, secretaryOf } from "../src/prodrome/core"
import excluded from "../src/prodrome/excluded"
import type { MemoryPayload } from "../src/wire/generated/wire"
import { heldChain } from "./held"

const core = ((): Core => {
  const built = process.env["SUZATARY_PRODROME_WASM"]
  if (built === undefined || built === "") throw new Error("SUZATARY_PRODROME_WASM is unset — run scripts/web-test.sh")
  return createRequire(import.meta.url)(`${built}/nodejs/prodrome.js`) as Core
})()

type Shipped = { hash: string; text: string }
const box = JSON.parse(readFileSync("app/core/tests/web/replicas.json", "utf8")) as {
  at: string
  inbox: Shipped[]
  conversations: Shipped[]
  keys: Record<string, string>
  memory: MemoryPayload
}
const named = (xs: readonly Shipped[]): Named[] => xs.map(o => ({ hash: o.hash, literal: o.text }))
const print = (literal: string): Named => ({ hash: createHash("sha256").update(literal, "utf8").digest("hex"), literal })

const asked = box.inbox.find(o => o.text.includes("act=Remember(text="))
// A dismissal resting on its card; the lagging conversation's own dismissal rests on nothing.
const dismissed = box.inbox.find(o => o.text.includes("event=Dismissed(") && /deps=\('[0-9a-f]{64}',\)/.test(o.text))
if (asked === undefined || dismissed === undefined) throw new Error("replicas.json has no remember card and dismissal")
const LOST = "p00000000000000000000000000000001"
const proposal = (text: string): string => text.replace(/proposal='p[0-9a-f]{32}'/, `proposal='${LOST}'`)
// The card as an older schema printed it, and a dismissal resting on it.
const unread = print(proposal(asked.text).replace(/act=Remember\(text='[0-9a-f]+', /, "act=Remember("))
const resting = print(proposal(dismissed.text).replace(/deps=\('[0-9a-f]{64}',\)/, `deps=('${unread.hash}',)`))

// No chain held: every proposal's price reads as it would with nothing to refer into.
const none = new core.Chains()
const inbox = named(box.inbox)
const conversations = named(box.conversations)
const held = [...inbox, unread, resting]

const answers = (s: Secretary): unknown[] => [
  s.inbox(box.at, box.memory, none),
  s.list(),
  ...s.list().conversations.map(c => s.conversation(c.id, {}, box.memory)),
  s.search("deed"),
]
const reading = <A>(i: readonly Named[], ask: (s: Secretary) => A): A => {
  const lines = new core.Conversations(objectsOf(conversations))
  const s = secretaryOf(core, i, lines, box.keys)
  try {
    return ask(s)
  } finally {
    s.free()
    lines.free()
  }
}

test("law: an unparseable print and what rests on it change no reading of the rest", () => {
  assert.deepEqual(reading(held, answers), reading(inbox, answers))
  assert.deepEqual(reading(inbox, s => s.excluded), excluded.NONE)
})

test("law: what was left out is exactly the print, in the parse's words, with what rests on it", () => {
  const left = reading(held, s => s.excluded)
  assert.deepEqual(left.map(x => [x.name, x.resting]), [[unread.hash, [resting.hash]]])
  assert.match(left[0]?.why ?? "", /^failed to parse: Asked\.act: Remember\(\.\.\.\) has no field text$/)
  assert.equal(excluded.said(excluded.left(left)), "1 object on this device could not be read and was left out (1 resting on it)")
})

test("law: a chain held over an unreadable print reads what it can, and the set says what it left out", () => {
  const junk = print("Sealed(prev=None, event=Nonsense())")
  assert.deepEqual(entriesOf(core, [junk], null, [], "life", none).entries, [])
  const kept = keptChains(core)
  try {
    const left = kept([heldChain("life", [junk], [])]).left.get("life")
    assert.deepEqual(left?.objects.map(x => x.name), [junk.hash])
    assert.deepEqual(left?.chains, [])
    assert.equal(kept([]).left.size, 0, "a chain no longer held says nothing")
  } finally {
    kept([]).chains.free()
  }
})
