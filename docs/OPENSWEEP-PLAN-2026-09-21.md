# The open list is never swept — 2026-09-21

The owner, 2026-09-19: many open todos are known, from context, to be done.
Chain: `opensweep` (project chain `suzatary`). Measured: 68 open on `life`;
of six read by hand, a departed flight, a settled batch, a job filed twice.

## 1. The cause is one sentence

**Nothing reads the open list.** Closing is triggered only by arriving mail
that triage can match to an id within its five-minute window. Evidence that
arrives unmatched is lost: `flightcheckin` had five messages from the
airline, each read once, none connected, because an airline's mail never
names `todo:life/flightcheckin`. Decay then hides it: a stale todo drifts
to 90%+ and off the top, so parked and done look identical.

## 2. What the sweep is

A secretary verb (it has the context, corpus and trusted actor) asking per
open todo: *does mail that arrived after it was authored show it done or
moot?* It answers with a `Changeset` the owner confirms or denies through the
existing machinery (`secretary::changes::apply`, `Proposal::Changes`,
`Outcome::Applied`). It only proposes; nothing closes without a tap. Three
rules, each a mechanism:

1. **Match by what mail carries, never by id.** Search through the same
   `search` verb the model has, for messages after the todo's `created`
   sharing its `source` sender, thread or subject, or naming its referents
   (confirmation number, order id, flight, person). `source.hash` is where
   the search starts, not what it looks for.
2. **A completion is a claim with a source.** Every proposed `Completed` or
   `Cancelled` carries `why` and at least one `mail:<hash>` (the `Reasons`
   smart constructor refuses less), so the card shows the proving message,
   checkable from the phone.
3. **The quiet majority stays quiet.** No new mail since authoring, no
   proposal: absence of evidence is not evidence, and cancelling the old is
   decay with a button. Age alone never enters a proposal.

## 3. What it is NOT

Not an auto-close: triage batches are settled by the secretary alone
(SECRETARY.md, "Reviewing the agent's batches") because they are
provisional records by an untrusted actor; a completion binds a todo
the owner authored, so it is a tap. Not a second matcher: `Corpus::search` and
the existing changeset card. Not a re-pricer: never `todo_update`; a
mispriced todo is a note in `why`. Not triage's job: triage still closes
what the message in front of it plainly closes.

## 4. The series

| # | Commit | Who |
|---|---|---|
| 1 | core: `sweep::candidates`, pure: open entries + corpus reader → (todo, plausibly bearing later messages), match reason typed (`SameThread`, `SameSender`, `NamesReferent`); no model, no writes; laws: no later mail yields nothing; a match carries its message | Sonnet |
| 2 | secretary: `sweep` as a round with the usual manual and verbs; its only write is `propose_changeset` of `Completed`/`Cancelled` with `Reasons`; `suzatary secretary sweep --json`, exit zero when nothing to propose | Sonnet |
| 3 | wrapper: `hourly-triage.sh` runs `secretary sweep` once a day after `review`, never per pass (the corpus is slow; one changeset a day gets read) | Sonnet |
| 4 | e2e: open todo + later message in its thread → proposal citing it → tap applies; a todo with no later mail is untouched | Sonnet |
| 5 | docs: CLAUDE.md Evidence gains "the open list is swept, by proposal"; SECRETARY.md gains the rule; the chain | **main session** |

## 5. Risks, and what not to do

- **Do not let the sweep write.** A direct `Completed` binds with no
  confirmation. A test pins that the sweep round's verbs hold no `record`.
- Do not match on age (§2 rule 3), skip the source (rule 2), or run it per
  pass (commit 3).
