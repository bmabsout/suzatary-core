# A decision is a prodrome — 2026-09-17

The owner, 2026-09-17: things already addressed stay in the Inbox; represent
what waits on him as prodromes. Chain `inboxprodrome` (project chain
`suzatary`). Follows `docs/CHATSHAPE-PLAN-2026-09-15.md` and
`docs/SECRETARYACTS-PLAN-2026-09-14.md`. The constitution's rule: anything
with a lifecycle and stakes belongs on the database.

## 1. What is wrong, with the evidence

On 2026-09-16 three pairs of proposals were the same message proposed twice
minutes apart (a password reset, a letter, a third vendor message); one
of each was answered, the other stayed STANDING, because nothing links a
revised draft to the one it replaces. What waits on the owner lives in three
places (standing proposals in `secretary/<id>.jsonl` with `Standing.outcome
== None`, held batches from `proposals::batches`, the Inbox's hand-written
query over both); none is priced, has a curve, can be revised or undone, or
is read by the folds. A proposal is todo-shaped.

## 2. The design

**A decision is a record of kind `decision` on the chain the thing belongs
to.** A new kind in `decoration::KINDS` (SPEC §1: evolution adds kinds).
`body`: one sentence to decide on ("Apply 3 changes to life", "Adopt the
triage run of 08:05 (4 objects)"). `rationale`: the proposal's `why`,
verbatim. `detail`: the full text read before deciding, the performing
reference on its FIRST LINE (`proposal:default/10`, `batch:life/2fdf1691`;
found by `reference::mentions`, `refs.ts`). `spec`: a curve the secretary
prices (`propose_*` gains optional `priority`, default 55; within days
35–45). `category`, `source` (mail hash if any), `created`, `waiting_on =
none`, `subtodos = []`, `notes = []`.

**The lifecycle IS the answer.** Tap → the box performs (send, apply, run,
adopt), then appends `Completed` by the tap's actor, the outcome as note
("sent to …; message-id …"). Dismiss → `Cancelled`, reason as note.
Withdraw (new) → `Cancelled` by `assistant`. A revised draft updates the
SAME record: `propose_*` take `revises: <index>`, which withdraws the old
record and marks the old conversation line superseded.

**The Inbox is a fold.** `view::entries` over every chain, kept to `kind ==
decision` and `state == open`, ascending in fulfillment; the count is its
length. Cards: the existing proposal card (via the reference) or batch diff;
"Settled today" is the same fold's decisions settled today. The page is a
`PageState` over the store (WEB-SPEC §1), no second copy of state.

**The conversation keeps the reasoning and the reference.** `Standing`
gains `decision: Option<TodoId>`; the inline card reads its state from the
record; `Performed`/`dismissed` lines stay as the receipt.

**The exact-text rule is unchanged.** What is sent is the stored proposal;
`detail` is a reading of it. The tap route resolves the reference and
performs THAT.

**A held batch is a decision.** A review HOLD authors one (reference
`batch:<chain>/<head>`); accept completes it and adopts, reject cancels it
and deletes, a later secretary settle cancels it with the review's `why`.

## 3. What is refused, and where

- **A sealed lifecycle event on a decision is refused at adoption**: a
  client-sealed `Completed`/`Cancelled` on a `decision` via `POST
  /api/objects` is refused ("a decision is settled through its tap"), so
  nothing looks done that was not done. The card offers tap and dismiss,
  never the replica's Done.
- **Only the tap route completes a decision** (`Tap::from_gate`); the box
  appends the event after the performance succeeded, as the tapping actor.
- **The reference on `detail`'s first line is parsed, never trusted**: an
  unparseable one draws as text with no controls and a problem line.

## 4. What does not change

Event vocabulary and shipped fields (SPEC §1), conformance vectors, `Tap`,
the stored text as the thing performed, conversation files, the todo replica
protocol. `suzatary pdf` renders decisions under "Waiting on you".

## 5. The series

| # | Commit | Who |
|---|---|---|
| 1 | `decision` in `KINDS`; `#decision(...)` in `utility.typ`; decoration round-trip law; site and document "Waiting on you"; `Reference::Batch { chain, head }` as `batch:<chain>/<head>` (print/parse/proptest, `refs.ts`) | Sonnet |
| 2 | pure `authoring::build_decision(...)` with laws; `Standing.decision`; `propose_*` author the record as `assistant`, take `priority`, `revises`; `withdraw_proposal(index, why)` | Sonnet |
| 3 | tap/dismiss append `Completed`/`Cancelled`; `POST /api/objects` refuses sealed events on a decision; HOLD authors a batch decision, accept/reject/later settle close it | Sonnet |
| 4 | `Entry` carries `kind`; Inbox `PageState` over open decisions across chains, cards via reference, "Settled today"; chat cards read the record; the count | Sonnet |
| 5 | serve-start migration (a standing proposal with no record gets one, idempotent); fixture proposals produce records; scenes: Inbox shows it, tap completes and count drops, revision withdraws, sealed Completed refused | Sonnet |
| 6 | SECRETARY.md (revise, never duplicate; price; withdraw), ARCHITECTURE §10.3/§10.6, CLAUDE.md, APP.md; chain | **main session** |

Order: 1; 2; 3 and 4 in parallel; 5; 6. Each commit builds and passes the
Rust, web and browser gates alone.

## 6. Risks, and what not to do

- **Do not put the proposal's fields into the record**: the twelve fields
  are frozen, `notes` a closed vocabulary; the exact text stays stored.
- **Do not let adoption perform**: a late-synced `Completed` would send a
  late email; §3's refusal is the rule.
- **Do not make the Inbox a second fold**: `view::entries`, one kind.
- **Later, if `detail`'s first line proves too thin:** a `Decided` payload
  in prodrome-core (`Payload::KIND` per arm), a versioned change to the
  public crate. Not now.
