# A conversation is a prodrome

The owner, 2026-09-30: "how come the conversations aren't a prodrome
themselves, they are basically the same structure with things like proposals
waiting for a response, they can have an fpl value to represent how important
it is for the secretary to respond to it as opposed to doing other tasks. I'm
not sure that last part is necessary at all but I like the regularity of it
all."

Filed as `todo:suzatary/conversationprodrome`; the owner asked for all of it
on 2026-09-30, and it is built in the stages of section 6.

## 1. What this reverses, and why that is not the old mistake

ARCHITECTURE §10.3 decision 4 and `secretary/store.rs` say a conversation is
NOT on the Prodrome (2026-09-08: "the temporal FPL database, not a chat
log"). The argument was that a turn has no fulfillment, lifecycle or
register, so it would sit on the TODO chain as a generic payload, §10.1's
mistake pointed the other way.

Three things have changed since:

- **The Prodrome is generic over its payload** (`store::Prodrome<P>`). The
  inbox already is one, beside `life` and `suzatary`, with its own payload
  (`Asked`), and nothing about it is a todo (`PROPOSALS-PLAN-2026-09-28.md`).
  A conversation prodrome is a third payload, not turns on the todo chain.
  The objection was to the latter and still holds.
- **The file already IS one, without the guarantees.** `store.rs` says it
  in as many words: the JSON-lines file is append-only and its value is a
  fold, "the same shape as the chain's registers, one storage layer out and
  with none of its guarantees": nothing content-addressed, nothing verified,
  a truncated file a truncated conversation. The queue (`queue::delta`) is a
  derivation over it, which is a register read under another name.
- **Change identity (prodrome `b9dc00c`) makes a clean store possible.**
  Suzatary's todo stores cannot start with a `Genesis`, because the phone,
  the agent's replica and `merge` still write legacy envelopes. Nothing
  writes a conversation but the box, so a conversation store can be
  Genesis-rooted from its first object, the first store here with no legacy
  root.

## 2. The model

- **One store, keyed by conversation** (as built in stage 2; this bullet first
  said one prodrome per conversation). `secretary-conversations/` holds every
  conversation under one `Genesis`, and the line's own `conversation` field is
  the entity key the fold separates them by.
- **A turn is a `Change`** whose payload is today's `Turn` (Person, Model,
  Tool, Act), and whose deps are the transcript register's frontier. So a
  conversation is causally ordered, not ordered by line number. Two phones
  typing into one conversation without seeing each other's turn make two
  heads, which is what happened, and the next model turn rests on both.
- **The registers are the questions the code already asks.**
  - *transcript*: every turn, in causal order (what the page draws).
  - *owed*: the person turns no model turn descends from. This is
    `queue::delta`, read instead of derived by hand.
  - *standing*: the proposals still open, joined from the inbox by object
    name, as `Stored::standing` does now.
- **A streaming answer is not an object until it ends.** The frames the web
  shows while the model writes stay ephemeral, as today. Only the finished
  `Turn::Model` is written.

## 3. The price (optional, and the last stage)

The owner doubts this part is needed, and the design agrees it can wait. A
conversation can carry an FPL term on its `owed` register: how badly a
waiting person turn needs an answer. That matters only when jobs compete for
the secretary (a conversation, the batch review, the open sweep, the brief).
Today every conversation is answered at once, so there is nothing to order.

What is worth doing now is leaving room for it. A conversation's price is a
field that reads `Absent` (unpriced) until something sets it, as a proposal's
is. If a scheduler ever runs the secretary's jobs by urgency, conversations
join that ordering with no change to the store, which is the regularity the
owner likes. A plausible term is a `Decay` from the oldest owed turn, with an
anchor from the scale table (a question asked while waiting for a reply is
50–65, an idle chat 90+).

## 4. What it buys

- **Verification and content addressing** for the one store that has
  neither: a transcript can be rehashed, and a torn write is a missing object
  rather than a truncated line.
- **The phone gets conversations offline.** A replica already syncs
  prodromes (`since=`, the wasm verifier). Today `/events` reports chain
  tips and the chat page polls, because a conversation is not on a chain.
  After this, a conversation syncs like a chain, and reading it offline
  needs no new mechanism.
- **Less code.** `store.rs`'s `Entry`/`Stored::of` fold and `queue.rs`'s
  hand derivation become register reads. The polling path for chat can go.
- **One mental model.** Todos, proposals and conversations are all
  prodromes with their own payload, and "what is waiting on whom" is one
  kind of question.

## 5. What it costs, and the open questions

1. **Where the store lives: the data repository** (the owner, 2026-09-30).
   It is pushed like the other prodromes, so it is backed up with every
   triage push. Transcripts quote mail, and a scan of the live files found
   no pasted secret but did find sign-in links read from mail: a password
   reset, a portal invite and verification token (JWTs), Instagram links with
   login tokens. So a line is stored only through redaction: the identity
   numbers `redact` already removes, and now a link's credential query
   parameters and any JWT, at the one door a turn is written through. The
   original mail keeps the working link. Until the switch, the weekly vault
   holds the JSON-lines files (`vault-backup.sh`, since 2026-09-30).
2. **Object count.** Every turn becomes a file, and tool answers can be
   large. The measure before building is how many turns the live
   conversations hold and how the store's verify time scales with that.
3. **Migration is one-way.** The JSON-lines files become objects once, on a
   scratch copy first, with every conversation's `Stored` reading pinned
   identical before and after except the redacted spans, each of which the
   pin names (as `legacy_stores.rs` did for change identity). The files are
   kept until the owner says to delete them.
4. **The at-most-once rule stays the inbox's.** A tap's effect is memoised
   by the proposal's hash in the inbox prodrome. The conversation only names
   the proposal object; it does not become a second place that decides
   whether something ran.

## 6. Stages (each a PR)

1. ✅ **The core** (2026-09-30, the `conversation-prodrome-core-jrzwvi` report).
   `Turn` as a prodrome payload with its literal vocabulary;
   a conversation store in the core and its registers (transcript, owed,
   standing) with laws, among them that the owed register equals
   `queue::delta` on generated transcripts; link-credential redaction in
   `redact.rs`, and a type that says a line went through it. Nothing wired.
   As built:
   - The payload is `secretary::line::Line`: one `Entry`, of every kind and
     not only turns, since the fold is positional across kinds (a proposal's
     `after_turn`, a stop ending the session); `rewound` is a line the fold
     reads. As the inbox does, the entry is its JSON inside a literal; its
     `at` is the record's. `held` (how many lines the writer held) keeps the
     same words said twice two objects, because the Prodrome writes nothing
     for an event it already holds.
   - Every line writes one register, the todo `transcript`, so a line's deps
     are its frontier. `store::Prodromes` is the store shape: prodromes keyed
     by the label of their genesis. The registers are reads of
     `secretary::transcript::Transcript`; under two heads, `owed` keeps a
     question the answer did not rest on.
   - `redact::Scrubbed<T>` is the door: every string through the identity
     numbers and the link pass. `redact()` itself is unchanged, so a document
     read keeps its links.
   - Measured (1,000 lines of the live shape, release, on disk): a fold in
     6 ms, a read in 56 ms, verify in 0.28 s, and an append that grows with
     the conversation, 5 ms at first and 76 ms by the thousandth line, since
     the Prodrome's `append` reloads and reprints every object to look for a
     twin. Stage 2 should weigh that before the server writes every turn.
2. ✅ **The switch** (2026-10-01, the `conversation-schema-migration-miif7a` report;
   built, not live until the owner runs the migration). A migration verb over the JSON-lines files, redacting as
   it converts, gated on a scratch copy of the live files with the reading
   pins; the server writes new turns as changes through the redaction door;
   `safe-push.sh` commits the store; the JSON-lines path is deleted in the
   same series (no dual path). As built:
   - The conversations are a schema of their own,
     `secretary::conversation_event::ConversationEvent`, one constructor per
     kind of line, replacing stage 1's todo-shaped stand-in. ONE store keyed by
     conversation (`secretary-conversations/`), not a genesis per
     conversation (§2's first bullet is superseded): the key already separates
     conversations in the fold, and the Prodrome's append reads the whole
     directory either way, so a genesis per entity bought nothing.
   - The transcript is the one register, a grow-only set under inclusion; the
     readings and their laws are stage 1's, plus convergence of two replicas.
   - `suzatary secretary migrate-store` names every redacted span by place and
     compares every `Stored`; `serve` refuses until it has run. Measured at
     the live size (3,500 lines in one store): an append about 250 ms, a fold
     of every conversation 54 ms, verify 260 ms.
3. **The web.** A conversation syncs as a replica, and the chat's polling
   goes.
4. (Optional) the price on `owed`, only once something schedules the
   secretary's jobs by urgency.

Docs to change when built: ARCHITECTURE §10.3 decision 4, `secretary/store.rs`
and `queue.rs`'s headers, CLAUDE.md's "No JSON for todos" paragraph (which
names the conversations as the JSON that is not todos) and its Log line "A
conversation is not an event".
