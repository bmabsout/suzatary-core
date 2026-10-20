# `onechat` — implementation plan (2026-09-13)

Spec: `docs/APP.md`, Secretary, "There is ONE conversation, and it
remembers"; as built: ARCHITECTURE §10.3 "There is one conversation". Opus
plan against `edf3b46`, accepted by the main session; §2a added after a
spike.

## 0. Verified vs assumed (Claude Code 2.1.257, `claude --help` locally)

Verified: `--session-id <uuid>` lets **the box mint the id**; `-r,
--resume` takes it; `-c, --continue` is per cwd, unusable (conversations
share one); `--output-format stream-json` + `--include-partial-messages`
(`--print` only) stream; `--settings`, `--strict-mcp-config`, `--model`
are ordinary launch options; **`--system-prompt-snapshot` is on by
default**, reusing the first system prompt on every resume until
compaction (the containment fact, §5).

Assumed: (1) the output carries `session_id` — cross-check only, settled
by §2a. (2) A stdin message does NOT reach a turn in flight; the plan
batches at the next turn (optional #10 flips it after a spike). (3)
Compaction is lossy and stays resumable: `secretary/<id>.jsonl` is the
truth. (4) `--settings` is re-read per launch — **pinned by a test** (§5).

## 1. The type

**A new trait `SessionModel`, not a second method on `Model`**, beside it
in `app/core/src/capability/model.rs`:

```rust
pub struct SessionId(String);              // smart constructor: a UUID, nothing else
pub struct Session { pub id: SessionId, pub model: String, pub cwd: String,
                     pub created: String, pub last: String }
pub struct Sessioned { pub reply: Reply, pub session: Session }

pub trait SessionModel: Model {
    fn open(&self, id: &SessionId, conversation: &Conversation, tools: &Value)
        -> Result<Sessioned, ModelError>;
    fn resume(&self, session: &Session, delta: &[Turn], tools: &Value,
              on_delta: &dyn Fn(&str)) -> Result<Sessioned, ModelError>;
}
```

Not `Model::session_turn`: `FakeModel`, `Scripted` and the Messages-API
interpreter hold no session, so it would answer an invented id or a "not
that kind of model" error (records-with-a-bool). `ClaudeCli` implements
both; only the conversation worker takes the bound; `Model::turn` stays for
`secretary brief`'s cold path, `secretary review` and every test.

**Where it lives:** an `Entry::Session { at, session, model, cwd }` line in
`secretary/<id>.jsonl` (`app/core/src/secretary/store.rs`); `Stored.session:
Option<Session>` is the last one. No second store, no chain write.

**Recovery.** Minting before spawning means a crash between spawn and append
orphans a session, never loses one. `ModelError::Lost(String)` (resume's CLI
exits non-zero naming an unknown session) gets one cold `Model::turn` over
the whole transcript (`prompt_of`, unchanged) under a fresh `--session-id`
and a new `Entry::Session`. One `match` arm.

## 2. The queue

**The transcript is the queue**: undelivered = every
`Turn::Person`/`Turn::Act` after the last `Turn::Model`-with-no-calls. Pure
module `app/core/src/secretary/queue.rs`:

```rust
pub fn delta(stored: &Stored) -> &[Turn];      // what the next resume sends
pub fn pending(stored: &Stored) -> usize;      // how many lines are still queued
pub const MAX_PENDING: usize = 16;
```

Never sent twice is a property of the derivation, not a lock. Property test:
over any sequence of person turns and answers, each person turn is in
exactly one `delta`.

**The worker** (`app/cli/src/commands/serve/conversation_worker.rs`):
`State.workers: Arc<Workers>`, a `Mutex<HashMap<ConversationId,
mpsc::Sender<()>>>` of nudges, never payloads. `handlers::say` calls
`state.workers.nudge(&id)` instead of `spawn_blocking(answer)`; the task
spawns on first nudge, drops itself when idle and settled. Two `say`s during
a turn: one exchange, the second heads the next delta. Past `MAX_PENDING`,
`Refusal::bad_request`. On `ClaudeCli::DEFAULT_TIMEOUT` the worker writes
the "This did not finish" turn and drops the session (next turn cold).

**Wire.** `ConversationPayload.thinking: bool` becomes `state:
ConversationState` = `idle | queued | thinking | answering` (a partial is
streaming); `TurnRow.queued: bool`; `secretary::state(&Stored)` replaces
`secretary::thinking`, still derived from the last turn alone.

### 2a. Frame shapes (spiked by the main session, see the CHANGELOG entry)

A Haiku run under the fence (`--session-id <uuid> --output-format
stream-json --include-partial-messages --verbose`), captured in
`app/io/tests/fixtures/claude-stream-json.jsonl` for commit 2's parser:
every line is JSON with a top-level `type` and our `session_id`; `system`
×2, `rate_limit_event`, `stream_event`s (API events under `event`) mixed
with whole `assistant` lines, then `result`. Text = `event.delta.text` when
`event.type` is `content_block_delta` and `event.delta.type` is
`text_delta`; `thinking_delta`/`signature_delta` are NOT text (`delta_of` →
`None`). `result` has `stop_reason`, `usage` (input, cache write/read,
output, thinking), `duration_api_ms`, `total_cost_usd`: the per-turn log's
source.

## 3. Streaming

**A new SSE endpoint, not `/events`** (a chain tip; a conversation is not
on the chain): `GET /api/secretary/conversations/{id}/stream`, same read
gate. `data:` frames, closed on `kind`: `{"kind":"state","state":
"queued|thinking|answering|idle"}`; `{"kind":"delta","text":"…"}` (the
model's `say`, verbatim, never HTML); `{"kind":"turn"}` (appended: refetch
`GET …/{id}`, the one door for proposals, references, folded state).

The partial is a per-conversation `tokio::sync::broadcast` plus a
`Mutex<String>` in `Workers`, **never written to the jsonl**, fed through
`resume`'s `on_delta` by pure `fn delta_of(line: &str) -> Option<String>`
(`app/io/src/model.rs`, tested on fixture lines). It renders as one
`div.turn.turn-model.partial` (`textContent` via `dom.ts`), removed on
`turn`. **No card is drawn from a partial**: a tap indexes what was stored
and shown. Reconnect sends `state`, the whole partial as one `delta`, then
live (or `turn` if it ended); the poll is the fallback after three
EventSource failures. Under `typstanswers` the partial stays plain text; no
compiler per delta.

## 4. The default conversation

**`store::DEFAULT = ConversationId::new("default")`**, title "Secretary";
opened lazily, forwarded to by `pages/secretary.ts` (`#/secretary/default`),
the brief's home unless `--new` (ARCHITECTURE §10.3; settles
`briefwherethemorningstarts`). Not "the newest" (a side-topic `start` would
become tomorrow's default); not an `instance.toml` key (no deployment
variance); precedent `secretary::review::conversation_of`. `start` still
mints `YYYY-MM-DD-n`; `secretary review` **keeps its own `<date>-review`
conversation**: the agent's chatter, not the owner's chat.

## 5. Containment

`--settings <fence>`, `--strict-mcp-config` and `claudeMdExcludes` ride on
`--resume` as on `--session-id`; the `PreToolUse` guard fences a resumed
turn identically. **The one real hazard**: the snapshot (§0) keeps a first
prompt's memory-file content, so a session opened unfenced stays unfenced.
So the box mints every id, `ClaudeCli::open` (always fenced) is the only
creator, and `app/cli/tests/guards.rs` asserts the argv of **both** `open`
and `resume` holds `--settings <secretary-settings.json>`,
`--strict-mcp-config`, `--session-id`/`--resume` and names no tool (`fn
argv(...) -> Vec<String>`, pure, extracted in commit 2).

## 6. The series

| # | Commit | Tests | Who |
|---|---|---|---|
| 1 | core: `SessionId`/`Session`/`SessionModel`; `Entry::Session`; `Stored.session`; `ConversationState` + `secretary::state` | pure tests in `store.rs`, `mod.rs` | Sonnet verbatim |
| 2 | io: `fn argv(...)` extracted; `ClaudeCli::open`/`resume`; `delta_of` line parser; `ModelError::Lost` | `model.rs` unit tests, no network; `guards.rs` argv case | Sonnet, with §2a's frames |
| 3 | core: `secretary/queue.rs` (`delta`, `pending`, `MAX_PENDING`) + the session/cold-recovery arm in `answer` | property test: every person turn delivered exactly once | Sonnet verbatim |
| 4 | cli: `serve/conversation_worker.rs`, `State.workers`, `say` nudges | two `say`s during one turn ⇒ one exchange, both lines delivered; `MAX_PENDING` refusal | main session (concurrency shape) |
| 5 | cli+wire: SSE `…/stream`, partial broadcast, `ConversationPayload.state`, `TurnRow.queued` | route test: deltas then a `turn` frame from a scripted model; reconnect replays the partial | main session |
| 6 | web: `api.ts` types+guards; `conversation.ts` — Send never disabled, queued styling, partial node, EventSource + poll fallback | e2e scene + shot | Sonnet after #5's wire is frozen |
| 7 | the default conversation: `store::DEFAULT`, open-if-absent, `pages/secretary.ts` forwards | e2e, cli tests | Sonnet verbatim |
| 8 | `secretary brief` into the default (+ `--new`); the push deep link | cli tests, `briefings.rs` | Sonnet verbatim |
| 9 | docs: ARCHITECTURE §10.3 amendment, CLAUDE.md Log, `onechat` closed on the chain | changelog test | main session |

Optional #10, only if a spike proves assumption (2): mid-turn delivery over
a held-open `--input-format stream-json`.

## 7. Risks and what NOT to do

Beyond §2–§5 (never deliver twice; no partial in the jsonl or as a card; no
`/events`; no unfenced session):
- The session id never goes on the chain or into a third file.
- Never render a proposal through whatever `typstanswers` lands.
- A crash mid-turn recovers by a cold re-send into a fresh session.
- Never a task per `say`; never hold the `Workers` mutex across a
  subprocess. Never add `--allowedTools`, `--permission-mode` or an MCP
  config to make resume "work"; never treat the lossy session as the record.
- Today's real defect: a second `say` mid-turn spawns a second `answer`
  loop on one file (Send is disabled only for the 202). Commit 4 fixes it;
  commit 6 is cosmetics.
