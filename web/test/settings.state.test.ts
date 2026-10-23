// The Settings page's fold: the frame's step over its value. Only a new reading moves `overview`.

import { test } from "node:test";
import assert from "node:assert/strict";

import type { Overview } from "../src/wire/generated/wire";
import settingsState from "../src/pages/settings.state";
import frame from "../src/view/frame";
import { forAll, overviewOf, rng, type Rng } from "./gen";
import lo from "../src/algebra/lo";

type Loaded = settingsState.T;
type Event = frame.Event<Loaded, settingsState.Event>;
const step = frame.step(settingsState.step);
const answer = (overview: Overview, me: string | null): Event => frame.read(lo.val({ overview, rows: [], me, writes: me !== null, device: null }));

const OVERVIEW: Overview = overviewOf(rng(3));

function event(r: Rng): Event {
  switch (r.int(3)) {
    case 0:
      return answer(overviewOf(r), r.bool() ? null : r.string("abc", 4));
    case 1:
      return frame.read(lo.error(r.string("abc ", 8)));
    default:
      return frame.own({ type: "Writes", me: r.bool() ? null : r.string("abc", 4), writes: r.bool() });
  }
}

test("arrived: reads the overview as given", () => {
  assert.equal(lo.withDefault(step(lo.loading, answer(OVERVIEW, null)), null)?.overview, OVERVIEW);
});

test("failed: fails the page when nothing has been read, and leaves it where it stood once something has", () => {
  assert.equal(step(lo.loading, frame.read(lo.error("no answer"))).type, "Error");
  const read = step(lo.loading, answer(OVERVIEW, null));
  assert.equal(step(read, frame.read(lo.error("no answer"))), read);
});

test("me: set once, a no-op on repeat", () => {
  const known = step(step(lo.loading, answer(OVERVIEW, null)), frame.own({ type: "Writes", me: "ada", writes: true }));
  assert.equal(lo.withDefault(known, null)?.me, "ada");
  assert.equal(step(known, frame.own({ type: "Writes", me: "ada", writes: true })), known);
});

test("a key trusted since moves the page with the same writer, so its cards draw their acts", () => {
  const untrusted = step(step(lo.loading, answer(OVERVIEW, "ada")), frame.own({ type: "Writes", me: "ada", writes: false }));
  const trusted = step(untrusted, frame.own({ type: "Writes", me: "ada", writes: true }));
  assert.notEqual(trusted, untrusted);
  assert.equal(lo.withDefault(trusted, null)?.writes, true);
});

test("a later reading replaces the overview whole", () => {
  const later = overviewOf(rng(9));
  assert.equal(lo.withDefault(step(step(lo.loading, answer(OVERVIEW, "ada")), answer(later, "ada")), null)?.overview, later);
});

test("law: the state after any walk is the fold of its events", () => {
  forAll(300, (r) => {
    const events = r.array(r.int(40), () => event(r));
    let state: lo.T<Loaded> = lo.loading;
    for (const e of events) {
      state = step(state, e);
    }
    assert.deepEqual(state, events.reduce(step, lo.loading));
  });
});
