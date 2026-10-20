// The titles a card names its sources by move with the fold on screen: a page read before the todos arrived names them
// once they have, and hears it.

import { test } from "node:test"
import assert from "node:assert/strict"

import type { Entry } from "../src/wire/generated/wire"
import { store } from "../src/app/store"
import lo from "../src/algebra/lo"
import onScreen from "../src/inbox/onscreen"

const entry = (todo: string, body: string): Entry => ({
  todo,
  state: "open",
  states: ["open"],
  at: "",
  claimed: "",
  value: 0.5,
  unconfirmed: false,
  recurring: false,
  conflicts: {},
  candidates: [],
  stream: [],
  authored: {
    rich: { body: null, detail: null, rationale: null, waiting_on: null, keys: { body: null, detail: null, rationale: null, waiting_on: null } },
    block: { kind: "todo", name: "todo" },
    created: "2026-09-01",
    body,
    text: body,
    detail: "",
    category: "home",
    waiting_on: "",
    rationale: [],
    spec: null,
    actor: "ada",
    at: "2026-09-01T00:00:00",
    source: null,
    subtodos: [],
  },
})

test("a cold link reads no titles, then the todos' titles once the fold arrives, and is woken for it", () => {
  store.dispatch({ type: "Reset" })
  const titles = (): ReadonlyMap<string, string> => lo.withDefault(onScreen.titles.now(), new Map())
  assert.equal(titles().size, 0)
  let woken = 0
  const stop = onScreen.titles.hear(() => (woken += 1))
  store.dispatch({ type: "Cached", fold: { at: "2026-09-02T12:00:00", fetchedAt: 0, entries: [entry("file-permit", "Renew the *permit*\nby Friday")] } })
  assert.equal(titles().get("file-permit"), "Renew the permit")
  assert.ok(woken > 0, "the page reading the titles is told they moved")
  const was = onScreen.titles.now()
  store.dispatch({ type: "Stale" })
  assert.equal(onScreen.titles.now(), was, "a fold that moved no title hands back the same titles")
  stop()
  store.dispatch({ type: "Reset" })
})
