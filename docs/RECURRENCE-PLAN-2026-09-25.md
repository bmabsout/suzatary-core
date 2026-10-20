# Recurrence in the term algebra — 2026-09-25

Todos: `recurringfpl`, and the periodic half of `fulfillmentexpr` (its other
half, the live reference, shipped as `Ref` in Prodrome 0.7.0). The prompting
case is `life/toothbrushvinegar`: vinegar the toothbrush every two months.
Today that is one dated decay, re-dated by hand after every pass.

## 1. The algebra

- **Tending is an event, not a completion.** `Tended(todo, at, actor, note)`
  joins §4's database kinds. A recurring todo is never done, so recording a
  pass as `Completed` would drop it from the open list. `Tended` leaves the
  todo's state alone.
- **Tendings are a grow-only set.** The environment gains
  `tended: TodoId → set of Instant` beside the outcomes map. Folding events
  unions it: it is a G-set, so two replicas never conflict. The standing policy
  decides which tendings bind, as it does for completions.
- **Reading is as of `now`.** `last_tended(env, todo, now)` is the latest
  tending at or before `now`, the same guard `bound` applies. A later pass can
  never rewrite an earlier moment.
- **`Recur { todo, anchor, term, pending }`** re-anchors its body to the last
  tending, exactly as `After` re-anchors to a completion. No tending yet gives
  `pending`. A last tending at `s` gives `term` evaluated at
  `now − (s − anchor)`. The body is any term authored against `anchor`. For
  the toothbrush it is a `Decay` from 1.0 at `anchor` to 0.3 sixty days later.
- **`Periodic { period, anchor, term }`** repeats on the calendar whatever you
  do: `term` is evaluated at `anchor + ((now − anchor) mod period)`, with a
  Euclidean mod so moments before `anchor` work too. Rent and quarterly taxes
  are this.
- **`flatten` gains no case.** Neither constructor stores a curve, and a
  `Tended` event creates no piece. The terms read the environment, so no
  stored object changes meaning. This is evolution by adding kinds.

## 2. Laws (conformance vectors and property tests)

1. **Recur re-anchors:** with the last tending at `s`, `Recur` at `s + d`
   equals `term` at `anchor + d`.
2. **Recur ignores the future:** tendings after `now` change nothing at
   `now`. Adding a tending at `s` changes no value before `s`.
3. **Recur waits:** with no tending at or before `now`, it equals `pending`.
4. **Periodic repeats:** `Periodic` at `now + period` equals `Periodic` at
   `now`, and it equals `term` on `[anchor, anchor + period)`.
5. **Tending leaves state alone:** a `Tended` event changes no entry's state,
   spec or content, and the todo stays in the open list.
6. **Tendings merge by union:** folding a merge of two histories gives the
   union of their tendings. There is no conflict register for them.

## 3. Layers

- **Prodrome (public; minor release 0.8.0):**
  - The `Tended` kind, with its parse and print.
  - The environment's tended set, and `last_tended`.
  - `Recur` and `Periodic` in `TermF`, with `map`, evaluation, explanation
    ("tended 12 Aug, 44 days ago") and normalisation (both are temporal like
    `After` and `Shift`, never pushed pointwise).
  - Literals, SPEC §4, §6, §7 and §9, conformance vectors, and wasm exports.
- **Suzatary:**
  - A `tend` act, on the CLI, as an agent and secretary verb, and as a web
    button.
  - A recurring row's Done becomes "Did it", which records a tending.
  - The todo page's "Why" reads "tended 12 Aug".
  - Re-author `toothbrushvinegar` as a `Recur`, through a changeset the owner
    confirms.

## 4. The series

1. prodrome: `Tended`, the tended set, `last_tended`, laws 2, 5 and 6.
2. prodrome: `Recur`, laws 1 and 3; `Periodic`, law 4; literals and
   explanation.
3. prodrome: SPEC, vectors, wasm, CHANGELOG. **Pushing the tag waits for
   the owner's OK** (a public repository).
4. suzatary: pin 0.8.0; the `tend` act across the CLI, the verbs and the web.
5. suzatary: the toothbrush changeset; docs. **Done 2026-09-26**, as
   authoring: `authoring::Price` is a sum with `Recur(Cycle)` and
   `Periodic(Cycle)`, a cycle's body a two-point `Curve` from the authoring
   instant (not a `Decay`: that reads as a deadline, and caps at 98%).
   `suzatary todo add|update` take `--recur-days N` / `--periodic-days N`
   with `--from P --to Q`; the verbs and a changeset's `reprice` take
   `recur_every_days` / `periodic_every_days` with `cycle_from_priority` and
   `cycle_to_priority`; a proposal's diff says "every 60 days after a pass,
   from 100% to 30%". The toothbrush changeset itself is the owner's to
   propose and confirm, not written by the series.
