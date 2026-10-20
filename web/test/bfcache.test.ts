// A document closes what it keeps open when it is hidden, and opens it once again when it comes back from the cache.

import { test } from "node:test"
import assert from "node:assert/strict"

import bfcache from "../src/app/bfcache"
import { forAll } from "./gen"

type Ev = { readonly type: "pagehide" } | { readonly type: "pageshow"; readonly persisted: boolean }

test("law: hidden closes once, a return from the cache opens once, and a show that is no return opens nothing", () => {
  forAll(300, r => {
    const ls: { hide: (() => void)[]; show: ((e: { readonly persisted: boolean }) => void)[] } = { hide: [], show: [] }
    const w: bfcache.Window = {
      addEventListener: (type: string, l: (e: { readonly persisted: boolean }) => void) =>
        void (type === "pagehide" ? ls.hide.push(() => l({ persisted: false })) : ls.show.push(l)),
    }
    let open = true
    let opens = 0
    let closes = 0
    bfcache.across(w, { close: () => ((open = false), (closes += 1)), open: () => ((open = true), (opens += 1)) })
    const evs = r.array(r.int(20), (): Ev => (r.bool() ? { type: "pagehide" } : { type: "pageshow", persisted: r.bool() }))
    let expected = true
    let [eo, ec] = [0, 0]
    for (const e of evs) {
      if (e.type === "pagehide") ls.hide.forEach(l => l())
      else ls.show.forEach(l => l(e))
      if (e.type === "pagehide" && expected) (expected = false), (ec += 1)
      if (e.type === "pageshow" && e.persisted && !expected) (expected = true), (eo += 1)
      assert.equal(open, expected)
    }
    assert.deepEqual([opens, closes], [eo, ec])
  })
})
