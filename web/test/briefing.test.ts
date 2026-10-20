// A day's briefing: what the box sent, a 404 is an answer, any other failure is not.

import { test } from "node:test"
import assert from "node:assert/strict"

import type { BriefingPayload } from "../src/wire/generated/wire"
import http from "../src/wire/http"
import briefing from "../src/pages/briefing"
import type { Rng } from "./gen"
import { SLUG, day, forAll } from "./gen"

const genPayload = (r: Rng, date = day(r)): BriefingPayload => ({
  date,
  day: r.bool()
    ? { kind: "brief", conversation: r.string(SLUG, 1 + r.int(8)), turn: r.int(40), text: r.string(SLUG, r.int(30)), blocks: [] }
    : { kind: "saved", source: r.string(SLUG, r.int(30)), rich: r.bool() ? null : `<p>${r.string(SLUG, r.int(20))}</p>` },
})
const genDay = (r: Rng): briefing.Day => (r.bool() ? briefing.written(genPayload(r)) : briefing.none)

test("law: folding a day with the constructors rebuilds it", () => {
  const alg = { written: briefing.written, none: () => briefing.none }
  forAll(200, r => {
    const d = genDay(r)
    assert.deepEqual(briefing.fold(alg, d), d)
  })
})

test("law: a day the box answered is what it sent, whichever kind", async () => {
  const cases: { seed: number; payload: BriefingPayload }[] = []
  forAll(200, (r, seed) => cases.push({ seed, payload: genPayload(r) }))
  for (const c of cases)
    assert.deepEqual(await briefing.day(c.payload.date, () => Promise.resolve(c.payload)), briefing.written(c.payload), `seed ${c.seed}`)
})

test("a 404 is no briefing", async () => {
  assert.deepEqual(await briefing.day("2026-09-02", () => Promise.reject(new http.ApiError("gone", 404))), briefing.none)
})

test("any other failure is a failure, so the frame offers Retry", async () => {
  const outcome = await briefing.day("2026-09-02", () => Promise.reject(new http.ApiError("down", 502))).then(
    d => d,
    (e: unknown) => e,
  )
  assert.ok(outcome instanceof http.ApiError && outcome.status === 502, `rethrown: ${String(outcome)}`)
})
