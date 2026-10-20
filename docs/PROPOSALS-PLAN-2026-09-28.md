# Proposals as objects: a submit is idempotent by content — 2026-09-28

The owner, 2026-09-28, on the double tap: "the structure for a proposal should
be another thing that's content hashable, and literally it would be the exact
same proposal, so submitting it twice shouldn't run the effect twice … the
second time you submit you'd just get the same response", and "you can always
represent it as a prodrome object right? it just has a different type being a
proposal … with its own fpl value".

This replaces PR #1's claim: an `Outcome::Performing` line and a stage for it.
It builds on the Prodrome's empty term (`docs/design-empty-term.md` in
bmabsout/prodrome) and on MULTIPRODROME-PLAN §5b, the inbox prodrome.

## 1. The shape

- **A proposal is a Prodrome object** in the inbox store: the same envelope as
  a todo, with a proposal payload (the email draft, the changeset, the act),
  written by the secretary. Its id is the hash of the object that creates it,
  so proposing the same thing twice is the same object.
- **Its price is FPL.** Usually `Ref` to the todo it serves, so it is exactly
  as urgent as that todo, and composed with its own deadline when it has one.
  Unpriced is `Absent`. The Inbox orders by this value, and a push rings when
  it crosses into Problem (§5b.4).
- **Submitting is `submit(id) -> response`.**
  - The fold says whether the proposal is completed. If it is, the response is
    the one its completion carries, and nothing runs.
  - If not, the effect runs and its completion is appended with the response.
  - Dismissing is `Cancelled`, with the owner's reason.
- **What each response records:**
  - Sent, applied, acted: a completion. A resubmit returns it.
  - May have gone (`Failed::Unknown`): a completion that says so. A resubmit
    returns it, and nothing is sent again.
  - Nothing was sent (`Failed::NotSent`): no completion, only a note. The same
    object submitted again runs again, so "Try again" needs no stage of its
    own.
- **Two submits at the same instant.** The store's append lock serialises them.
  The second reads the first one's completion and returns it; while the first
  is still running, the second waits for it. That is a lock inside the
  interpreter, never a state anyone sees.
- **The conversation** keeps turns only. Its proposal line names the object's
  id, and the card is drawn from the fold.

## 2. What goes away

- `Outcome::Performing`, `Stage::Performing`, `Stage::Retry`, the 409 on a
  repeated tap, and `Conversations::append_if`. The conversation file stops
  being the place an effect is recorded.
- The decision record that points at a conversation line: the proposal object
  is the decision.

## 3. Tests: one law, fixtures that run the real code

The claim era pinned "a tap performs once" with about ten separate
status-code checks in four files, and one hand-written fixture passed only by
accident. The replacement:
- **One property.** Any interleaving of submits, resubmits and dismissals,
  including concurrent ones, performs each effect at most once, and every
  submit of a completed proposal returns its completion's response. It runs
  over a scripted transport that counts sends. The per-route checks it
  subsumes are deleted.
- **Fixtures built by running the real code.** A seeded conversation comes from
  a round driven by the scripted model, never from lines written by hand.
- **The empty-prompt fix** from PR #1 (a round with nothing waiting does not
  run, and a resume is never sent nothing) moves over unchanged, with its test.

## 4. The series

| # | Where | What |
|---|---|---|
| 1 | Prodrome | the empty term, per its design (SPEC §7, vectors, laws), then a release |
| 2 | Suzatary core | the proposal payload and `submit`, as a pure function of the fold and the transport's answer; the property of §3 |
| 3 | Suzatary cli | the inbox store (MULTIPRODROME rows 3 and 4, local only), the tap route calling `submit`, and the lock in the interpreter |
| 4 | web | cards drawn from the fold; the Inbox ordered by value; a data migration of standing proposals into objects |
| 5 | tests | delete the claim-era checks; fixtures through real rounds |

Row 1 is the only one that touches the Prodrome. Rows 2 to 5 are one pull
request if the cloud session can hold it, otherwise two (core and cli, then
web and tests).
