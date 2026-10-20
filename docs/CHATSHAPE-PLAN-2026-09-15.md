# The chat, tightened — 2026-09-15

The owner, 2026-09-15: "map out the interactions … make everything tighter."
Chain `chatshape` (project chain `suzatary`), on
`docs/ONECHAT-PLAN-2026-09-13.md`, `docs/SECRETARYACTS-PLAN-2026-09-14.md`
and `docs/APP.md` ("typstanswers"). As built: ARCHITECTURE §10.3 "The chat,
tightened". Changes nothing about the store, trust or the verb surface —
only what a person SEES and what WAKES whom.

## 1. The map

Parties: the owner (phone mostly), the SECRETARY (answers, proposes,
reviews), the BOX (performs what was tapped), the TRIAGE AGENT. Surfaces:
the CONVERSATION page (`#/secretary` → default), the INBOX (held batches,
standing proposals, a count), "Ask the secretary" on a
todo/message/thread/briefing. PUSH speaks only for the fold, never the chat.

| loop | the owner | the secretary | the box | wakes |
|---|---|---|---|---|
| ASK | types, optionally with chips | calls verbs, then answers | runs the verbs, streams, stores every line | `say` nudges the worker; nothing wakes the phone |
| PROPOSE | reads a card; taps, dismisses with a reason, or leaves it | writes the proposal with why/sources | stores it; on a tap sends/applies/runs; records the outcome | a tap or a dismissal wakes NOTHING; the model learns of it only on his next question |
| ACT (box's own) | taps Run | proposes the act | runs the device flow, writes two act turns | the completion wakes nothing either |
| BRIEF | opens the chat in the morning | writes the brief as the first turn of the day | `secretary brief` at 10:03 UTC into the default conversation | no push says the brief exists |
| REFERENCE | "Ask" from a page, or Reply on a turn/card | names things by their text form | loads what a chip points at through the verbs | — |

## 2. What is loose, with the evidence

`secretary/default.jsonl`, 2026-09-14: three exchanges, 40 stored lines (11
model turns, 20 verb answers), 35 blocks drawn. (1) The record is drawn as
the conversation. (2) The answer is the last of several drafts, then a
flicker. (3) Cards are drawn under the transcript, not where said. (4) An
act wakes nobody: a dismissal's correction sat unread 3 hours. (5) Nothing
wakes the phone, not even for the brief. (6) The legend names the model.
(7) The transcript is not a place: no times, no day marks, rebuilt per
`turn` frame (scroll lost, images re-fetched), not opened at the bottom.
(8) Enter does nothing.

## 3. The rule

**THE TRANSCRIPT IS THE RECORD; THE PAGE IS THE CONVERSATION.** Every round
stays stored, append-only (evidence, the model's memory); the page shows
what was said, answered, proposed (where), done; work is a TRACE folded
under the answer, openable to the exact lines. Two tests for every state:

- **Legible (gulf of evaluation)**: the page always says which of idle ·
  working (on what) · answered · waiting on you (a card) · done (a receipt)
  holds. Never a model name, a JSON body, a draft.
- **One gesture (gulf of execution)**: one to ask about what is in front of
  you, one to answer any card; the receipt is the card.

## 4. The design, loop by loop

### 4.1 Ask: rounds, status, answer

**Protocol (SECRETARY.md).** A model turn WITH calls is STATUS: one short
line naming what it checks, never a draft. The answer is written ONCE, in
the turn with no calls; an empty `say` with no calls is legal ("nothing to
add"; nothing drawn). `Turn::Model { calls }` already tells them apart: a
prompt and rendering change, no schema.

**Page.** The unit is a ROUND: a person turn (or act), a TRACE, the ANSWER.
The trace folds to one line — "Checked mail (3), todos (2) · 41 s" — and
opens to the status lines and verb answers (monospace, capped); a refused
verb is marked in the fold line itself.

**Streaming.** A status line ("Reading mail…") from a new `working` frame
per verb; the answer's partial streams IN THE PLACE it will land. The legend
drops the model name: "Thinking · 12 s", "Working · reading mail", "Idle ·
opened 14 Sep".

### 4.2 Propose: cards where they were said, and an answer that is heard

**Inline.** A card draws AFTER the turn that proposed it: each `Standing`
gets `after_turn` (index of the last turn before its `proposed` line), on
the wire. The trailing block goes. The card's state is the receipt — Sent
to …, Dismissed: …, Running/Done. The Inbox is unchanged.

**Heard.** A tap, a dismissal and a performed act's completion NUDGE the
worker like `say`. The model reads what the owner did and answers only if
there is something to do (empty-`say` rule), so a dismissal correcting who
handles what gets "Understood, I'll route that to Diego" within the minute.

### 4.3 Wake: push when nobody is watching

A round ending in an answer while the conversation's stream has NO
subscriber (the receiver count) pushes once through the fold's `Notify`
channels: title "The secretary answered", body the answer's first line
(plain text, capped at 120 characters), click `#/secretary`; the brief
likewise ("Your brief for Tuesday is ready"). At most one per round; never
for a status turn or a verb.

### 4.4 The transcript as a place

Turns carry `at` on the wire; DAY separators ("Monday 14 September · the
brief"). WINDOWED: the last 60 turns, `?before=<index>` and an "Earlier…"
control for older; proposal indices stay absolute. The DOM is KEYED by turn
index (`reconcile.ts`): a `turn` frame appends, never rebuilds. Opens at the
bottom and stays there unless scrolled up, then a "↓ new answer" pill.

### 4.5 Composer

Ctrl/⌘-Enter sends everywhere. At 62rem and up Enter sends and Shift-Enter
breaks a line; on a phone Enter is a newline. Chips unchanged.

## 5. What does not change

The store and its fold; `textContent` and the one door; `Tap` and the
no-send shape; the verb surface (`secretary::VERBS`, snapshot); the Inbox as
the queue; the protocol's shape (`say` + `calls`). Proposal ordering (by
index, the tap's) is untouched; a position is an ADDITIONAL fact.

## 6. The series

| # | Commit | Who |
|---|---|---|
| 1 | core/wire: turns carry `at`; a `Standing` carries `after_turn`; `TurnRow.at`, `ProposalRow.turn`; store tests | Sonnet |
| 2 | web: rounds — status and verb turns fold into a trace under the answer; cards inline after their turn; trailing block removed; node tests on the grouping (pure) | Sonnet |
| 3 | worker/wire/web: `working` frame with the verb name; legend without the model name; partial drawn in the answer's place | Sonnet |
| 4 | handlers + SECRETARY.md: tap, dismiss and a performed act nudge the worker; status-vs-answer and empty-`say` rules; the worker treats an empty answer as the round's end | **main session** |
| 5 | worker: push when the stream has no subscriber; once per round; brief included | Sonnet |
| 6 | core/web: windowed payload with `before`; day separators; keyed DOM; scroll rules | Sonnet |
| 7 | web: composer keys | Sonnet |
| 8 | docs: ARCHITECTURE §10.3, APP.md, CLAUDE.md log; the chain | **main session** |

1 first; 2 and 6 after it; 3, 5 and 7 beside. Each commit buildable and
tested alone; a series merges `--no-ff`.

## 7. Risks, and what not to do

- **An empty answer ends the round**: it is still "a model turn with no
  calls", so `state` goes idle; `turnBlock` returns null for empty model
  text and `queue::delta` moves past it.
- **Folding must not hide a refusal** (§4.1). **Push never carries a verb
  answer or mail** (§4.3).
- **Never renumber** a proposal's `index` (the tap's) or a `before` cursor
  (a TURN index); a `turn` frame refetches the window, not the file.
- **Never move the model's work out of the store** to quiet the page: the
  trace is a fold of stored lines, not a second record.

## 8. The secretary is a layer, not a page

The owner, later that day: "have the chat available everywhere with context
relevant to where I click from." Acts and words lived on different pages,
so the chain held acts without a why and the conversation whys without an
act.

### 8.1 Three rules

**Context is where you are, never typed.** What is in view (todo, message,
thread, card, briefing) is the attached reference; dropping the chip is one
tap, adding it none: "Ask" is the composer's DEFAULT state everywhere.

**An act can carry words, and the words go to both records.** Done, Cancel,
Reopen, Save, a tap and a dismissal take one optional line: the event's
`note` plus an act turn ("You did: cancelled todo:life/x — the meeting moved
to October") that nudges the worker (§4.2). The act is his, trusted; the
words are the secretary's to carry out, as `assistant` or a proposal.

**One conversation, many windows.** The one default conversation (onechat)
is INDEXED BY REFERENCE: person/act turns carry references, and every model
turn names things in prose the tokeniser finds, so "the conversation about
todo:life/x" is a QUERY over it, never a second chat.

### 8.2 The surface

**Phone (under 62rem).** A floating "Ask" button above the tab bar opens a
bottom SHEET: the chip attached, the slice about that thing (newest first),
the composer, a "Whole conversation" link to `#/secretary`. It is
`conversation.ts` in a narrower frame with a filter (one implementation),
closing back with scroll intact.

**Laptop (62rem and up).** A right-hand PANEL, open by default and
collapsible: the filtered slice, or the whole conversation when nothing is
selected — `layout.ts`'s `Screen` detail-pane pattern one column further:
list · detail · secretary.

**Acts with a line.** Todo acts, Save, and a card's Send/Confirm/Run and
Dismiss gain a one-line "why, or what next" field, empty by default,
required only where it already is (a dismissal). Empty means the act alone.

### 8.3 What it is made of

- `wire::ConversationPayload` takes `?about=<reference>`: the rounds whose
  person/act turn references the thing or whose answer names it (the box's
  own `Reference` grammar). Indices stay absolute.
- Act routes (`/api/act/*`, tap, dismiss) take an optional `note`: into the
  chain event's `note` and a `Turn::Act` with the reference and words; the
  worker nudged. One function builds every act turn, so wording cannot fork.
- `web/src/sheet.ts`'s `Sheet` mounts `conversation.ts` with an `about`
  reference in a compact frame, opened by each page's "Ask" with its own
  reference; `layout.ts`'s `Screen` gains a `secretary` column at 62rem+.
- SECRETARY.md: an act turn with words is an INSTRUCTION about its thing —
  done directly as `assistant` when plainly asked, a proposal when a
  judgement, or answered in a sentence; never ignored, never re-asked.

### 8.4 What this is not

Not multiple chats, not a bubble per todo, not a trust change (a noted act
is his; the follow-up is `assistant`'s write or a card he taps).

### 8.5 The series, continued

| # | Commit | Who |
|---|---|---|
| 9 | core/wire: `?about=<reference>` on the conversation payload — the rounds that touch a thing; tests on the slicing (pure) | Sonnet |
| 10 | core/cli: acts carry a line — `note` on the act routes and the tap/dismiss bodies, into the chain event and an act turn with the reference, one builder; the worker nudged | Sonnet |
| 11 | web: the sheet on the phone and the panel at 62rem; the "Ask" button on every page with the page's reference; `conversation.ts` mounts filtered | Sonnet |
| 12 | web: the one-line field beside Done/Cancel/Reopen/Save and on the cards | Sonnet |
| 13 | SECRETARY.md: an act's words are an instruction; docs; the chain | **main session** |

9 and 10 beside each other after 1; 11 after 2 and 9; 12 after 10 and 11.

## 9. The frames, done properly (2026-09-16)

The owner, the morning after: "Todos page is landing me at the bottom not
top", "… I think you didn't write this in a good modular way." Commit 11
had mounted a thousand-line page, via a `frame` flag, in a sheet and a
panel. So its jump-to-newest scrolled the DOCUMENT (the list behind landed
at the bottom), the composer scrolled off-screen with the turns, and an
empty slice drew only a header.

### 9.1 The parts

Three modules under `web/src/conversation/`, none knowing its page:

- **`source.ts`**, the DATA: fetch with window and `about`; the stream
  (state, delta, working, turn); queue state; "Earlier" paging; the
  empty-slice fallback. Pure state plus subscribers like `store.ts`, no
  DOM; node-tested through a fake fetch.
- **`transcript.ts`**, the VIEW: in the scroll box it is handed, draws rounds
  keyed by turn index (reconcile), day separators, trace folds, cards, the
  partial, Earlier, the new-answer pill, and owns the scroll rules AGAINST
  THAT BOX; never the document. Pure decisions stay in `rounds.ts`,
  `days.ts`, `scrollpos.ts`.
- **`composer.ts`**: chips, the box, Send, the keys (`compose.ts`), the
  focus rule; emits a `say` with references; no fetch of its own.

### 9.2 The frames

Each a pinned header, a scrolling transcript, a pinned composer:

- **page** (`pages/conversation.ts`, now short): the viewport scrolls; the
  heading "Conversation" and legend are the header.
- **sheet** (`ask.ts`): `position: fixed` at the bottom, at most 80vh;
  header = about line, legend, close; no page heading.
- **panel** (`panel.ts`): the same rows in a fixed column at 62rem+,
  collapse control in the header.

### 9.3 The rules the symptoms name

- **A frame scrolls its own box, never the document** (only the page frame's
  box is the document).
- **The composer is always on screen** in a sheet or panel: outside the
  transcript's box, visible the moment the sheet opens.
- **An empty slice falls back to the whole conversation**: the newest
  window, header "Nothing about todo:… yet · whole conversation", chip kept;
  the first question about the thing then filters.
- **The Ask button has its own room**: pages under 62rem get bottom padding
  of its height; it is not drawn on the conversation page.

### 9.4 The series

| # | Commit | Who |
|---|---|---|
| 14 | `conversation/source.ts` + `transcript.ts` + `composer.ts` extracted from the page, the page rebuilt on them, behaviour unchanged, node tests on the source's fallback and paging | Sonnet |
| 15 | the sheet and the panel rebuilt as frames on the three parts: pinned header and composer, own scroll box, empty-slice fallback, the button's room; e2e: sheet opens with the composer visible and the list behind at the top; empty slice shows the whole conversation's tail | Sonnet |
| 16 | docs; the chain | **main session** |
