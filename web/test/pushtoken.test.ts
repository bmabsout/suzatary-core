// pushtoken's laws: only the app's own message carries a token, and a token is reported again only when it is new or stale.

import { test } from "node:test"
import assert from "node:assert/strict"

import pushtoken from "../src/app/pushtoken"
import { SLUG, forAll } from "./gen"

test("only a message with no source window and an empty origin, of the app's shape, carries a token", () => {
  const sent = JSON.stringify({ pushToken: "a-token" })
  const window = {}
  assert.equal(pushtoken.fromApp(null, ""), true)
  for (const [source, origin] of [
    [window, ""],
    [null, "https://elsewhere.example"],
    [window, "https://elsewhere.example"],
  ] as const)
    assert.equal(pushtoken.fromApp(source, origin), false, `${String(source)} ${origin}`)
  assert.equal(pushtoken.heard(sent, true), "a-token")
  assert.equal(pushtoken.heard(sent, false), null)
  for (const data of [
    { pushToken: "a-token" },
    "not json",
    JSON.stringify({ pushToken: "" }),
    JSON.stringify({ pushToken: 7 }),
    JSON.stringify({ pushToken: "x".repeat(pushtoken.MOST + 1) }),
  ])
    assert.equal(pushtoken.heard(data, true), null, JSON.stringify(data))
})

test("law: a kept report reads back, and is due again only for another token, a week later, or a clock gone back", () =>
  forAll(300, r => {
    const token = r.string(SLUG, 1 + r.int(20))
    const at = r.int(1_000_000_000)
    const kept = pushtoken.read(pushtoken.print(token, at))
    assert.deepEqual(kept, { type: "Reported", token, at })
    const later = at + r.int(2 * pushtoken.FRESH_MS)
    assert.equal(pushtoken.due(kept, token, later), later - at >= pushtoken.FRESH_MS)
    assert.equal(pushtoken.due(kept, token + "x", later), true)
    assert.equal(pushtoken.due(kept, token, at - 1), true)
    assert.equal(pushtoken.due(pushtoken.none, token, at), true)
    assert.deepEqual(pushtoken.read(r.bool() ? null : "{"), pushtoken.none)
  }))
