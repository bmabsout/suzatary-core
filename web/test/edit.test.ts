/**
 * The edit form's own laws: seeded from what the chain holds, and a diff that
 * names only what moved. `draft.test.ts` covers the price algebra
 * (`price.ts`) that this form shares with the add sheet's.
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import type { Authored, SpecNode } from "../src/wire/generated/wire";
import { diffOf, formOf } from "../src/todo/edit";
import form from "../src/view/form";
import price from "../src/todo/price";
import recur from "../src/todo/recur";
import cast from "./cast";

const VOCABULARY = ["work", "study", "project", "home", "other"];

const { owner, todos: { high } } = cast.world;

function authored(over: Partial<Authored> = {}): Authored {
  return {
    rich: { body: null, detail: null, rationale: null, waiting_on: null, keys: { body: null, detail: null, rationale: null, waiting_on: null } },
    block: { kind: "todo", name: "todo" },
    created: "2026-09-01",
    body: high.body,
    text: high.body,
    detail: "",
    category: high.category,
    waiting_on: "",
    rationale: ["low enough"],
    spec: null,
    actor: owner.actor,
    at: "2026-09-01T00:00:00",
    source: null,
    subtodos: [],
    ...over,
  };
}

// --- reading a cycle back off the spec (`recur.priceOf`) ---------------------

const curve = (from: number, to: number, at: string, until: string): SpecNode => ({
  kind: "curve",
  points: [
    { at, value: from },
    { at: until, value: to },
  ],
});

const recurTerm = (from: number, to: number): SpecNode => ({
  kind: "recur",
  todo: high.id,
  anchor: "2026-09-01T00:00:00",
  term: curve(from, to, "2026-09-01T00:00:00", "2026-10-31T00:00:00"),
  pending: curve(from, to, "2026-09-01T00:00:00", "2026-10-31T00:00:00"),
});

const periodicTerm = (from: number, to: number): SpecNode => ({
  kind: "periodic",
  periodHours: 720,
  anchor: "2026-09-01T00:00:00",
  term: curve(from, to, "2026-09-01T00:00:00", "2026-10-01T00:00:00"),
});

test("priceOf reads a Recur's own two-point ramp as a Cycle", () => {
  assert.deepEqual(recur.priceOf(recurTerm(1, 0.3)), price.recur({ days: 60, from: 1, to: 0.3 }));
});

test("priceOf reads a Periodic's ramp too, tagged Periodic and not Recur", () => {
  assert.deepEqual(recur.priceOf(periodicTerm(0.9, 0.2)), price.periodic({ days: 30, from: 0.9, to: 0.2 }));
});

test("priceOf finds a cycle behind a deferral's Piecewise", () => {
  const deferred: SpecNode = {
    kind: "piecewise",
    head: { kind: "flat", value: 1 },
    pieces: [{ at: "2026-09-21T00:00:00", term: recurTerm(1, 0.3) }],
  };
  assert.deepEqual(recur.priceOf(deferred), price.recur({ days: 60, from: 1, to: 0.3 }));
});

test("priceOf finds a cycle behind a parent's Conj over its subtodos", () => {
  const composed: SpecNode = {
    kind: "conj",
    p: -4,
    terms: [periodicTerm(1, 0.5), { kind: "ref", todo: "buyfood" }],
  };
  assert.deepEqual(recur.priceOf(composed), price.periodic({ days: 30, from: 1, to: 0.5 }));
});

test("priceOf is null for a price with no cycle anywhere in it", () => {
  assert.equal(recur.priceOf({ kind: "flat", value: 0.4 }), null);
  assert.equal(
    recur.priceOf({ kind: "decay", start: 0.6, end: 0.05, endDate: "2026-09-20", leadUpHours: 168 }),
    null,
  );
});

// --- the edit form opens with a recurring todo's cycle filled in -------------

test("formOf seeds a recurring todo's cycle, read off the chain's own spec", () => {
  const fields = formOf(authored({ spec: recurTerm(1, 0.3) }), VOCABULARY);
  assert.deepEqual(fields.price.v, price.recur({ days: 60, from: 1, to: 0.3 }));
  assert.equal(fields.reprice.v, false, "the reprice controls stay collapsed until asked for");
});

test("formOf defaults to Flat when the current price carries no cycle", () => {
  assert.deepEqual(formOf(authored({ spec: { kind: "flat", value: 0.4 } }), VOCABULARY).price.v, price.EMPTY);
});

// --- diffOf sends the wire shape a reprice means, and nothing else -----------

test("a reprice to a cycle is refused without a rationale, in the box's own words", () => {
  const before = authored();
  const fields = form.fill(formOf(before, VOCABULARY), { reprice: true, price: price.recur({ days: 60, from: 1, to: 0.3 }) });
  const diffed = diffOf(high.id, before, fields);
  assert.equal(diffed.type, "Refused");
  if (diffed.type === "Refused") assert.match(diffed.reason, /why/i);
});

test("a reprice to a cycle sends exactly the wire shape, and nothing untouched", () => {
  const before = authored();
  const fields = form.fill(formOf(before, VOCABULARY), {
    reprice: true,
    price: price.periodic({ days: 30, from: 1, to: 0.4 }),
    rationale: "rent is due monthly",
  });
  const diffed = diffOf(high.id, before, fields);
  assert.equal(diffed.type, "Ok");
  if (diffed.type !== "Ok") return;
  assert.deepEqual(diffed.v, {
    todo: high.id,
    priority: { how: "periodic", days: 30, from_percent: 100, to_percent: 40 },
    rationale: "rent is due monthly",
  });
});

test("an edit that moves nothing is sent as the todo alone, so the box's refusal is the one shown", () => {
  const before = authored();
  const diffed = diffOf(high.id, before, formOf(before, VOCABULARY));
  assert.equal(diffed.type, "Ok");
  if (diffed.type === "Ok") assert.deepEqual(diffed.v, { todo: high.id });
});
