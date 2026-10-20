// The composer's state: one value stepped by a closed sum of events, so its laws hold over any sequence of them.

import { test } from "node:test"
import assert from "node:assert/strict"

import composing from "../src/chat/composing"
import again from "../src/chat/again"
import { parse, print } from "../src/chat/refs"
import type { Reference } from "../src/chat/refs"
import type { AttachmentRow, TurnRow } from "../src/wire/generated/wire"
import { forAll, referenceText } from "./gen"
import type { Rng } from "./gen"

const reference = (r: Rng): Reference => {
  const parsed = parse(referenceText(r))
  assert.ok(parsed !== null)
  return parsed
}
const file = (r: Rng): AttachmentRow => ({ id: `f${r.int(4)}`, media_type: "application/pdf", name: "form.pdf", private: "" })
const withheld = (r: Rng): composing.Withheld => ({ base64: r.string("ab", 4), name: "id.pdf", why: "it names a number", folder: "Private" })
const question = (r: Rng): TurnRow => ({
  index: r.int(20),
  who: "person",
  verb: "",
  text: r.pick(["where is the form", "and the receipt?"]),
  refused: false,
  references: r.bool() ? [] : [referenceText(r)],
  attachments: [],
  queued: false,
  blocks: [],
  at: "2026-01-01T00:00:00",
  remembered: null,
  attempt: "current",
  gone: null,
})

// Any event, the held-back files and uploads it answers drawn mostly from those the state holds, so they land; a file
// held back is always a new one, as each upload's answer is.
const event = (r: Rng, s: composing.T): composing.Event => {
  const w = s.held.length > 0 && r.bool(0.7) ? r.pick(s.held).w : withheld(r)
  const id = s.uploading.size > 0 && r.bool(0.7) ? r.pick([...s.uploading]) : r.int(8)
  const sent: composing.Sent = { draft: { text: r.pick(["", "kept words"]), references: [], files: [] }, mode: again.asking }
  const all: composing.Event[] = [
    { type: "Typed", text: r.pick(["", "  ", "a question", "a question, longer"]) },
    { type: "Quoted", text: r.pick(["a line", "two\nlines"]) },
    { type: "Attached", reference: reference(r) },
    { type: "Detached", reference: r.pick([...s.draft.references, reference(r)]) },
    { type: "Picked", id: r.int(8) },
    { type: "Filed", id, file: file(r) },
    { type: "Withheld", id, w: withheld(r) },
    { type: "Lost", id },
    { type: "Unfiled", id: `f${r.int(4)}` },
    { type: "Keeping", w },
    { type: "Unkept", w },
    { type: "Kept", w, file: file(r) },
    { type: "Dropped", w },
    { type: "Edit", question: question(r) },
    { type: "Cancel" },
    { type: "Send" },
    { type: "Failed", sent },
  ]
  return r.pick(all)
}

const walk = (r: Rng, check: (s: composing.T, e: composing.Event, next: composing.T) => void): void => {
  let s = composing.of("", [])
  for (let i = r.int(40); i > 0; i -= 1) {
    const e = event(r, s)
    const next = composing.step(s, e)
    check(s, e, next)
    s = next
  }
}

const distinct = <A>(xs: readonly A[], key: (a: A) => string): boolean => new Set(xs.map(key)).size === xs.length

test("law: every state reached holds each chip and each file once", () =>
  forAll(300, r =>
    walk(r, (_, __, s) => {
      assert.ok(distinct(s.draft.references, print), "chips are a set by spelling")
      assert.ok(distinct(s.draft.files, f => f.id), "files are a set by id")
    })))

test("law: the uploads on their way are the picks no answer has named", () =>
  forAll(300, r => {
    const expected = new Set<number>()
    walk(r, (_, e, next) => {
      if (e.type === "Picked") expected.add(e.id)
      if (e.type === "Filed" || e.type === "Withheld" || e.type === "Lost") expected.delete(e.id)
      assert.deepEqual([...next.uploading].sort(), [...expected].sort())
    })
  }))

test("law: keeping or dropping a file held back takes that file and no other", () =>
  forAll(300, r =>
    walk(r, (s, e, next) => {
      if (e.type !== "Kept" && e.type !== "Dropped") return
      assert.deepEqual(next.held, s.held.filter(h => h.w !== e.w))
    })))

test("law: a file being kept stays Keeping through every event but its own answer", () =>
  forAll(300, r =>
    walk(r, (s, e, next) => {
      const own = (w: composing.Withheld): boolean => (e.type === "Unkept" || e.type === "Kept" || e.type === "Dropped") && e.w === w
      for (const h of s.held) {
        if (h.type !== "Keeping" || own(h.w)) continue
        const after = next.held.find(x => x.w === h.w)
        if (after !== undefined) assert.equal(after.type, "Keeping")
        else assert.ok(e.type === "Cancel" || e.type === "Send", "only the box emptying lets it go")
      }
    })))

test("law: a send empties the box at once, chips and held-back files too, and leaves Editing; one not ready changes nothing", () =>
  forAll(300, r =>
    walk(r, (s, e, next) => {
      if (e.type !== "Send") return
      if (composing.sending(s).type !== "Ready") return assert.equal(next, s, "the same object, so nothing wakes")
      assert.deepEqual(next.draft, composing.empty)
      assert.deepEqual(next.held, [])
      assert.equal(next.mode.type, "Asking")
    })))

test("law: a failed send comes back, in the mode it was asked in, only into an empty box", () =>
  forAll(300, r =>
    walk(r, (s, e, next) => {
      if (e.type !== "Failed") return
      if (!composing.isEmpty(s.draft)) return assert.equal(next, s, "what was typed since stands")
      assert.equal(next.draft, e.sent.draft)
      assert.equal(next.mode, e.sent.mode)
    })))

test("law: typing the words already in the box, or attaching a chip it holds, is the same state", () =>
  forAll(200, r =>
    walk(r, (_, __, next) => {
      assert.equal(composing.step(next, { type: "Typed", text: next.draft.text }), next)
      for (const reference of next.draft.references) assert.equal(composing.step(next, { type: "Attached", reference }), next)
    })))

test("Send waits for a file on its way and a file held back, and a blank box sends nothing", () => {
  const typed = composing.step(composing.of("", []), { type: "Typed", text: "where is the form" })
  assert.equal(composing.sending(composing.of("   ", [])).type, "Blank")
  assert.equal(composing.sending(composing.step(typed, { type: "Picked", id: 1 })).type, "Uploading")
  const w = { base64: "ab", name: "id.pdf", why: "it names a number", folder: "Private" }
  const held = composing.step(composing.step(typed, { type: "Picked", id: 1 }), { type: "Withheld", id: 1, w })
  assert.equal(composing.sending(held).type, "Held")
  assert.equal(composing.sending(composing.step(held, { type: "Dropped", w })).type, "Ready")
})
