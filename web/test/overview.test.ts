// The overview's pure half, the one function the page draws a line by: a line is drawn failing exactly when its standing
// asks something of the owner, its words say what is missing and what failed, a line in place says nothing, every group
// is drawn or offered once, and a line draws exactly the open cards its references name.

import { test } from "node:test"
import assert from "node:assert/strict"
import { readFileSync } from "node:fs"

import type { Entry, LineStanding, ProposalRow } from "../src/wire/generated/wire"
import type { Row } from "../src/inbox/inbox"
import payload from "../src/wire/generated/payload"
import waiting from "../src/inbox/waiting"
import overview from "../src/pages/overview"
import { aCard } from "./cards"
import { authoredAs, cardRef, failureClass, forAll, GROUPS, lineStanding, overviewLine, overviewOf, type Rng } from "./gen"

const at = (s: string) => `[${s}]`

const ASKS: readonly LineStanding["kind"][] = ["locked", "store_failed", "unreadable", "by_hand", "failing", "missing"]

const entry = (r: Rng): Entry => ({
  todo: r.string("abc", 6),
  state: "open",
  states: [],
  at: "",
  claimed: "",
  unconfirmed: false,
  recurring: false,
  value: null,
  conflicts: {},
  authored: authoredAs({ kind: "decision" }),
  candidates: [],
  stream: [],
})

const proposal = (index: number): ProposalRow => ({
  index,
  act: { kind: "claude_sign_in", sentence: "connect" },
  card: aCard(),
  why: "",
  sources: [],
  at: "",
  state: "standing",
  outcome: "",
  tries: 0,
  turn: 0,
  coding: null,
  announced: null,
})

// A row the box names `proposal:<conversation>/<index>`.
const row = (r: Rng, conversation: string, index: number): Row => ({
  kind: "proposal",
  w: waiting.decision("life", entry(r)),
  conversation: { id: conversation, title: "", model: "test", opened: "", turns: [], proposals: [], unplaced: [], state: "idle", first: 0, total: 0, again: null },
  proposal: proposal(index),
})

test("law: a line is drawn failing exactly when its standing asks something of the owner", () => {
  forAll(300, r => {
    const l = overviewLine(r)
    assert.equal(overview.drawn(l, at).health?.type === "Asks", ASKS.includes(l.standing.kind), JSON.stringify(l.standing))
  })
})

test("law: a line in place says nothing, and every other standing says something", () => {
  forAll(300, r => {
    const s = lineStanding(r)
    const h = overview.health(s, at)
    assert.equal(h === null, s.kind === "kept")
    if (h !== null) assert.ok(h.text.length > 0)
  })
})

test("law: the words name every missing thing, every failure's words, every step's reason and every instant they rest on", () => {
  forAll(300, r => {
    const s = lineStanding(r)
    const text = overview.health(s, at)?.text ?? ""
    const named =
      s.kind === "missing" ? s.what
      : s.kind === "failing" ? [s.words, at(s.since), at(s.at)].filter((n, i) => i < 2 || n !== at(s.since))
      : s.kind === "synced" || s.kind === "answered" ? [at(s.at)]
      : s.kind === "by_hand" ? [s.why]
      : []
    for (const n of named) assert.ok(text.includes(n), `${text} names ${n}`)
  })
})

test("law: a terminal's command is drawn only apart from the words, and only for a step a terminal takes", () => {
  forAll(300, r => {
    const l = overviewLine(r)
    const d = overview.drawn(l, at)
    if (l.standing.kind === "by_hand") {
      assert.equal(d.atTheBox, l.standing.command)
      if (!l.standing.why.includes(l.standing.command)) assert.ok(!(d.health?.text ?? "").includes(l.standing.command), d.health?.text)
    } else assert.equal(d.atTheBox, null)
  })
})

test("law: a line draws its label, its detail and its standing's words, and nothing of its cards", () => {
  forAll(300, r => {
    const l = overviewLine(r)
    const d = overview.drawn(l, at)
    assert.equal(d.label, l.label)
    assert.equal(d.detail, l.detail)
    assert.deepEqual(overview.drawn({ ...l, cards: r.array(1 + r.int(3), () => cardRef(r)) }, at), d)
  })
})

test("law: a group with nothing connected lists nothing, and every group is either drawn or offered, once, in order", () => {
  forAll(300, r => {
    const o = overviewOf(r)
    const drawn = overview.connected(o)
    assert.ok(drawn.every(g => g.lines.length > 0))
    assert.deepEqual([...drawn.map(g => g.group), ...overview.offered(o)].sort(), [...GROUPS].sort())
    assert.deepEqual(drawn.map(g => g.group), GROUPS.filter(g => drawn.some(d => d.group === g)))
  })
})

test("law: a line draws exactly the rows its references name, in its order", () => {
  forAll(300, r => {
    const rows = r.array(r.int(8), () => row(r, r.pick(["setup", "secretask", "c"]), r.int(6)))
    const l = overviewLine(r)
    const drawn = overview.cards(l, rows)
    for (const x of drawn) assert.ok(l.cards.includes(overview.reference(x) ?? ""))
    for (const x of rows) {
      const ref = overview.reference(x) ?? ""
      assert.equal(drawn.includes(x), l.cards.includes(ref))
    }
  })
})

test("the box's own overview decodes, holds every group once in order, and names no secret", () => {
  const samples: unknown = JSON.parse(readFileSync("app/core/tests/web/answers.json", "utf8"))
  const sampled = typeof samples === "object" && samples !== null && "Overview" in samples && Array.isArray(samples.Overview) ? samples.Overview : []
  assert.ok(sampled.length > 0, "the box writes an overview")
  for (const sample of sampled) {
    const read = payload.Overview(sample.value)
    if (read.type !== "Val") throw new Error(`an Overview sample does not decode: ${JSON.stringify(read)}`)
    assert.deepEqual(read.v.groups.map(g => g.group), [...GROUPS])
    assert.equal(/PLUGIN_|_API_KEY|_TOKEN/.test(JSON.stringify(read.v)), false)
  }
})

test("law: a line's words are a function of its standing alone", () => {
  forAll(300, r => {
    const l = overviewLine(r)
    const other = { ...overviewLine(r), standing: l.standing }
    assert.deepEqual(overview.drawn(other, at).health, overview.drawn(l, at).health)
  })
})

test("law: a failure names what its class asks of the owner, and two kinds of class say two things", () => {
  forAll(300, r => {
    const [a, b] = [failureClass(r), failureClass(r)]
    // A revoked link asks the same of the owner whatever status said so; the status is in the failure's own words.
    assert.equal(overview.cause(a) === overview.cause(b), a.kind === b.kind)
    assert.equal(overview.cause(a) === "", a.kind === "unclassified")
  })
})

test("setup is unfinished while a line asks something of the owner, and says how many steps are done", () => {
  forAll(200, r => {
    const o = overviewOf(r)
    const lines = o.groups.flatMap(g => g.lines)
    const asking = lines.filter(l => overview.health(l.standing, at)?.type === "Asks").length
    const p = overview.progress(o)
    if (asking === 0) {
      assert.deepEqual(p, { type: "Finished" })
      assert.equal(overview.unfinished(p), null)
    } else {
      assert.deepEqual(p, { type: "Unfinished", done: lines.length - asking, of: lines.length })
      assert.equal(overview.unfinished(p), `Finish setting up · ${lines.length - asking} of ${lines.length} done`)
    }
  })
})

test("a failing line says its cause once, and its last try only when it differs from when it began", () => {
  const failing = (cls: Parameters<typeof overview.cause>[0], since: string, tried: string) =>
    overview.health({ kind: "failing", since, at: tried, class: cls, words: "it answered 401 (Unauthorized)" }, at)?.text ?? ""
  const revoked = failing({ kind: "revoked", status: "unauthorized" }, "2026-10-17T17:00:00", "2026-10-17T17:00:00")
  assert.equal(revoked.match(/401/g)?.length, 1, revoked)
  assert.ok(!revoked.includes("last tried"), revoked)
  assert.match(revoked, /\. Its link was changed or revoked, so store the new one\.$/)
  const later = failing({ kind: "unclassified" }, "2026-10-17T17:00:00", "2026-10-17T18:00:00")
  assert.match(later, /it answered 401 \(Unauthorized\) \(last tried /)
  assert.match(failing({ kind: "key_refused" }, "2026-10-17T17:00:00", "2026-10-17T17:00:00"), /card below/)
})

test("every standing the box writes is drawn by its standing alone, each failure's class named", () => {
  const read = payload.Overview(JSON.parse(readFileSync("app/core/tests/web/standings.json", "utf8")))
  if (read.type !== "Val") throw new Error(`the standings vector does not decode: ${JSON.stringify(read)}`)
  const lines = read.v.groups.flatMap(g => g.lines)
  const kinds = new Set(lines.map(l => l.standing.kind))
  assert.ok(kinds.size >= 10, `the vector holds every arm: ${[...kinds].join(", ")}`)
  forAll(50, r => {
    for (const l of lines) {
      const [mine, theirs] = [overview.drawn({ ...overviewLine(r), standing: l.standing }, at), overview.drawn(l, at)]
      assert.deepEqual([mine.health, mine.atTheBox], [theirs.health, theirs.atTheBox])
      if (l.standing.kind === "failing") assert.ok(overview.health(l.standing, at)?.text.endsWith(`.${overview.cause(l.standing.class)}`))
    }
  })
  const failing = lines.flatMap(l => (l.standing.kind === "failing" ? [{ kind: l.standing.class.kind, text: overview.health(l.standing, at)?.text }] : []))
  assert.equal(new Set(failing.map(f => f.text)).size, new Set(failing.map(f => f.kind)).size, "each kind of class's line says something of its own")
})

test("law: a grant is drawn inside a link's card exactly when the line holds both open", () => {
  const states: readonly ProposalRow["state"][] = ["standing", "failed", "acted", "dismissed"]
  forAll(300, r => {
    const of = (kind: "secret" | "grant", index: number): Row => {
      const base = row(r, "setup", index)
      if (base.kind !== "proposal") throw new Error("a proposal row")
      const act: ProposalRow["act"] =
        kind === "secret"
          ? { kind, label: "your Work calendar's secret link", found: null, link: null }
          : { kind, plugin: "ics", origin: "https://cal.example.test", sentence: "allow it" }
      return { ...base, proposal: { ...base.proposal, act, state: r.pick(states) } }
    }
    const rows = r.array(r.int(4), () => of(r.pick(["secret", "grant"] as const), r.int(6)))
    const open = (kind: string) => rows.some(x => x.kind === "proposal" && x.proposal.act.kind === kind && (x.proposal.state === "standing" || x.proposal.state === "failed"))
    const a = overview.allowing(rows)
    assert.equal(a !== null, open("secret") && open("grant"))
    if (a !== null) assert.deepEqual([a.link, a.grant].map(x => x.kind === "proposal" && x.proposal.act.kind), ["secret", "grant"])
  })
})
