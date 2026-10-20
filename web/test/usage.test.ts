// A percentage under "reported" and nothing else; the scope line; credits as a named sum.

import { test } from "node:test"
import assert from "node:assert/strict"

import type { UsageAllowance, UsageRefusal, UsageScope, UsageTokens, UsageWindow } from "../src/wire/generated/wire"
import usage from "../src/pages/usage"
import { forAll } from "./gen"
import type { Rng } from "./gen"

const NOW = Date.parse("2026-09-14T15:00:00Z")

const windowWith = (allowance: UsageAllowance): UsageWindow => ({
  window: "five_hour",
  since: "2026-09-14T10:00:00Z",
  requests: 12,
  tokens: { fresh: 100, cache_write: 50, cache_read: 900, output: 200 },
  by_model: [],
  by_who: [],
  cache_hit: 0.8,
  last_refusal: null,
  on_credits: null,
  most_seen_before_refusal: null,
  allowance,
  elsewhere: { fresh: 0, cache_write: 0, cache_read: 0, output: 0 },
})

test("reported carries a percentage, and only reported does", () => {
  const d = usage.describeAllowance(
    windowWith({ kind: "reported", utilization: 0.42, resets_at: "2026-09-14T20:00:00Z", as_of: "2026-09-14T14:30:00Z" }),
    NOW,
  )
  assert.equal(d.percentage, 0.42)
  assert.match(d.text, /as of/)
  assert.match(d.text, /resets/)
})

test("rate limited never carries a percentage, and names the retry wait", () => {
  const d = usage.describeAllowance(
    windowWith({ kind: "rate_limited", since: "2026-09-14T14:00:00Z", retry_after_seconds: 3472, last_reported: null }),
    NOW,
  )
  assert.equal(d.percentage, null, "the exact usageceilinglabel bug: no percentage here")
  assert.match(d.text, /rate-limiting this account/)
  assert.match(d.text, /retry in 58 min/)
  assert.match(d.text, /it has never answered/)
  assert.ok(!d.text.includes("%"))
})

test("rate limited with a prior success names it, not a fraction of the ceiling", () => {
  const d = usage.describeAllowance(
    windowWith({
      kind: "rate_limited",
      since: "2026-09-14T14:00:00Z",
      retry_after_seconds: 60,
      last_reported: { utilization: 0.65, resets_at: null, as_of: "2026-09-14T11:00:00Z" },
    }),
    NOW,
  )
  assert.equal(d.percentage, null)
  assert.match(d.text, /last answer/)
  assert.match(d.text, /65\.0%/)
})

test("a scope refusal says why the allowance is unread, and asks for no other token", () => {
  const d = usage.describeAllowance(
    windowWith({
      kind: "refused",
      at: "2026-09-14T14:00:00Z",
      status: 403,
      words: "oauth_scope_insufficient: OAuth token does not meet scope requirement user:profile",
    }),
    NOW,
  )
  assert.equal(d.percentage, null)
  assert.match(d.text, /unread/)
  assert.match(d.text, /403 oauth_scope_insufficient/)
  assert.match(d.text, /setup-token/)
  assert.match(d.text, /far more than a number/)
})

test("any other refusal is said in its own words", () => {
  const d = usage.describeAllowance(
    windowWith({ kind: "refused", at: "2026-09-14T14:00:00Z", status: 500, words: "api_error: Internal" }),
    NOW,
  )
  assert.equal(d.percentage, null)
  assert.match(d.text, /refused this box's token \(500 api_error: Internal\)/)
  assert.ok(!d.text.includes("setup-token"))
})

test("unknown says it has not been read, and carries no percentage", () => {
  const d = usage.describeAllowance(windowWith({ kind: "unknown" }), NOW)
  assert.equal(d.percentage, null)
  assert.match(d.text, /has not been read yet/)
})

// The scope line: why a full allowance window sits beside a quiet totals card.

const scope = (counted: string[], notCounted: string[]): UsageScope => ({ counted, not_counted: notCounted })

test("the scope line names what was counted and what was not", () => {
  const line = usage.describeScope(scope(["-home-me", "-home-me-repo"], ["-tmp"]))
  assert.match(line, /counts 2 of 3 project directories/)
  assert.match(line, /-home-me, -home-me-repo/)
  assert.match(line, /not counted: -tmp/)
})

test("with nothing left out, the scope line names no exclusion", () => {
  const line = usage.describeScope(scope(["-home-me"], []))
  assert.match(line, /counts 1 of 1 project directory:/)
  assert.ok(!line.includes("not counted"), `should not mention an exclusion: ${line}`)
})

test("an empty scope draws nothing — an old server has no opinion to state", () => {
  assert.equal(usage.describeScope(scope([], [])), "")
})

const refusal = (overage: boolean, reason = ""): UsageRefusal => ({
  at: "2026-09-14T12:00:00Z",
  resets_at: null,
  overage_enabled: overage,
  overage_using: false,
  overage_reason: reason,
})

const genTokens = (r: Rng): UsageTokens => ({
  fresh: r.int(1000),
  cache_write: r.int(1000),
  cache_read: r.int(1000),
  output: r.int(1000),
})
const genCredits = (r: Rng): usage.credits.Credits =>
  r.pick([usage.credits.disabled(r.string("abc ", 4)), usage.credits.spent(r.string("0123456789", 8), genTokens(r)), usage.credits.unspent])

test("credits: disabled at the organisation wins, whatever was spent", () => {
  const w = { ...windowWith({ kind: "unknown" }), on_credits: { fresh: 1, cache_write: 0, cache_read: 0, output: 0 } }
  assert.deepEqual(usage.credits.of(w, refusal(false, "policy")), usage.credits.disabled("policy"))
})

test("credits: enabled and spent names the tokens and the refusal they follow", () => {
  const spent = { fresh: 5, cache_write: 0, cache_read: 0, output: 2 }
  const w = { ...windowWith({ kind: "unknown" }), on_credits: spent }
  assert.deepEqual(usage.credits.of(w, refusal(true)), usage.credits.spent("2026-09-14T12:00:00Z", spent))
  assert.deepEqual(usage.credits.of(windowWith({ kind: "unknown" }), refusal(true)), usage.credits.unspent)
})

test("law: folding credits with the constructors rebuilds them", () => {
  const alg = { disabled: usage.credits.disabled, spent: usage.credits.spent, unspent: () => usage.credits.unspent }
  forAll(200, r => {
    const c = genCredits(r)
    assert.deepEqual(usage.credits.fold(alg, c), c)
  })
})

// Known, pre-existing: compact(999_950) rounds to "1000k"; the values below avoid that band.
test("compact keeps small counts exact and never drops a unit as counts grow", () => {
  for (let n = 0; n < 1000; n += 37) assert.equal(usage.compact(n), String(n))
  const units = ["", "k", "M", "B"]
  const seen = [999, 1000, 9_999, 10_000, 999_000, 1_000_000, 999_000_000, 1_000_000_000].map(n =>
    units.indexOf(usage.compact(n).replace(/[\d.]/g, "")),
  )
  seen.reduce((last, unit, i) => (assert.ok(unit >= last, `unit ${unit} after ${last} at ${i}`), unit), 0)
})
