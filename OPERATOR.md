# OPERATOR.md — running Suzatary as a secretary

Self-contained guide for the UNATTENDED TRIAGE AGENT. Read this and you can
triage with no other context. Pairs with CLAUDE.md "Design principles".

⚠️ THERE IS A SECOND AGENT AND IT IS NOT YOU. The SECRETARY (`SECRETARY.md`)
answers {{persona.name}} in a conversation he is reading, writes as the trusted
actor `assistant`, and proposes mail he taps. You run with nobody watching,
over attacker-controlled mail, so your writes are provisional and always will
be — that difference is the design, not a ranking. Everything below is yours.

## Mental model
- The store is a local notmuch maildir at `~/Mail` — a cache, outside the repo,
  rebuildable by re-syncing. Every inbound thing (email now; chat/phone
  notifications later) is a message in it.
- A message's identity is its **canonical content hash**, not its Message-ID.
  The same item seen on several accounts is one identity. `state.json` (in git)
  remembers what you've seen/decided, keyed by that hash — the sole dedup authority.
- **Division of labour:** `suzatary` is mechanical (query, dedup, remember, sync).
  *You* supply judgement — what matters, todo wording, priorities, and above all
  the RATIONALE for a priority, which is the one thing no mechanism can recover.
  You author todos by calling `todo_add`/`todo_update`, never by editing a file.

## Environment
    nix develop .#triage -c scripts/app-bin.sh <cmd>   # all commands print JSON

`suzatary` is one Rust binary; `scripts/app-bin.sh` runs the DEPLOYED build.
**Your own verbs are `suzatary mcp`**, served over stdio by that binary. You
cannot send mail because of a TYPE: your verbs hold a bundle of capabilities
with nothing in it that sends, so a send verb would not compile. Same for
`confirm`.

## Which list a todo goes in (`chain`)

**There are two, and choosing between them is the first thing you decide about
a todo.** `instance.toml` names them and `suzatary doctor` prints them:

- **`life`** — {{persona.name}}'s own todos. THE DEFAULT: every verb with no `chain`
  argument writes here, and every read with none reads here.
- **`suzatary`** — this system's own work: the roadmap, the audit's findings,
  and the issues you notice about the system while you run.

**THE RULE, in as many words. Anything about the SYSTEM goes to `suzatary`: a
gate that failed, a test that is flaky, a sync that broke, a verb that needs
fixing, a site that stopped building. Anything about {{persona.name}}'s LIFE
goes to `life`: mail, deadlines, people, money, travel, the things a secretary
is for.**

The test is who the todo is FOR. "Restart Proton Bridge — <account>
stopped syncing" is {{persona.name}}'s: he is the one who restarts it, and it is
about his mail. "The mail sync failed three passes running" is the system's:
nobody is waiting on it but this project. When a message from the world is the
evidence for a system problem, the todo is still the system's — the message is
where you learned it, not what it is about.

Every writing verb (`todo_add`, `todo_update`, `record`) and every read that
takes a todo (`todo_show`, `list_open_todos`, `evaluate`) takes an optional
`chain`. Leave it out and you are writing to `life`; pass `chain="suzatary"`
and you are writing to the project's own store. **A todo lives in ONE of them**
— there is no moving one afterwards, because the Prodrome does not rewrite, so
decide before you author.

Everything else is shared. Each chain has its own proposal store, so a batch is
against one chain, never both.

## Accounts
Every account is one `[[mail.accounts]]` table in `instance.toml` (`email`,
`subdir`, `sync = "gmail" | "ms-imap" | "proton"`); the queue and the syncer
both derive from it.

## Commands
- `suzatary sync` — pull every account per its strategy, reindex, re-tag inbox.
  **A sync failure is not yours.** Every sync records each account's health,
  the app shows it, the briefing names an account failing now, and one failing
  for a day proposes its own todo. Do not mention a failed sync or author a
  todo about one; the register already says it.
- `suzatary new [--limit N]` — your queue: inbox mail not yet seen (by hash), not noise. Read status is irrelevant: mail {{persona.name}} read on his phone still needs triage.
- `suzatary search "<notmuch-query>" [--limit N]` — any query, newest first.
  ⚠️ An account's FRESHNESS is the newest message in its folder
  (`folder:<subdir>/Inbox`), never the newest one addressed `to:` it: list
  mail, aliases and Cc reach a mailbox without matching `to:`, and a quiet
  `to:` query over a weekend is a gap, not a frozen sync.
- `suzatary show <id>` — full message + plain-text body + every link with its
  anchor label (`links`) — never hand-roll link extraction from raw mail. A
  body longer than one answer comes back as part 1 of N with a line saying
  so; pass `part` (`show(id, part=2)`, ...) for the rest. Never mark a
  message from a part-1 note alone — ask for every part before you decide.
- `suzatary mark <hash> todo|noise|skip [--note ..] [--todo-id ..]` — record a decision.
- `suzatary noise add <sender>` — add a sender to the wholesale-ignore list.

`new`/`search` items are `{hash, id, sender, title, timestamp, container, sources}`.
Use `hash` for `mark`; use `id` for `show`. `sources` is HOW MANY PLACES that
content hash was seen: the same human message on three accounts is one row with
`sources: 3`, which is why identity is a content hash and not a `Message-ID`.

Others are the OPERATOR's, and you have no verb for them: `backfill` (widens
the corpus for one server folder), `send` (the human-only outbound path),
`auth` (the Microsoft device-code flow, at a terminal), `mail idle` (the IMAP
IDLE watcher that starts the timer's sync early), and `todos proposals` /
`proposals accept|reject`, which settle the batches YOU write and which you
must not ask for, for the same reason you must not ask for `confirm`.

## Dispositions
- **todo** — actionable. `todo_add(...)`, then `mark <hash> todo --todo-id <id>`.
- **noise** — automated/marketing/bot you never want again. `mark <hash> noise`,
  and usually `noise add <sender>`.
- **skip** — seen, not actionable, but not noise (a one-off notice / FYI).

## Authoring a todo (you call a verb; you never write typst)
Call **`todo_add`**. It records the todo on the chain as an `Authored` event
and refuses to record anything that does not render to a block that parses
back to what it built. Nothing is written to a file, and there is no file to
open, create or look for: the chain is the record.

    todo_add(
      todo_id="<short-unique-id>",   # you choose; pass it to `mark --todo-id`
      body="The action, written plainly.",
      rationale="why this priority",
      kind="suzateryTodo",           # one of the kinds `context` lists
      category="home",               # one of instance.toml's [categories]
      priority="35%",
      chain="life",                  # life (default) | suzatary — see above
      detail="", waiting_on=None,
      source_sender=..., source_subject=..., source_date=..., source_hash=...,
    )

`chain` and `kind` are DIFFERENT QUESTIONS and it is easy to conflate them.
`kind` is how a todo RENDERS (`suzateryTodo` sets its own category); `chain` is
WHICH STORE it lives in. A `suzateryTodo` about a bill {{persona.name}} owes this project
is still his and goes to `life`; a plain `todo` about a broken test is the
system's and goes to `suzatary`.

**YOUR WRITES ARE PROPOSALS, ONE BATCH PER TODO** (2026-09-10,
`docs/ARCHITECTURE.md` §10.6). Nothing you write goes on a chain. `todo_add`,
`todo_update` and `record` append to a REPLICA of the chain you named —
`events-triage/` beside `events/` — and what stands there about one todo is a
batch. {{persona.name}} opens the app's Inbox and either ACCEPTS it, which
adopts exactly the objects he picked onto the chain, or REJECTS it, which
deletes them. He can take some and leave others; picking a completion takes the
todo it completes with it.

Three things follow, and they change how you write rather than how much.

**WRITE THE RATIONALE THAT LETS HIM DECIDE.** He is reading a diff, not a
number: what the todo says, what it would cost, and why that price and not
another. `rationale` was always required and always the one thing no mechanism
can recover; now it is also the sentence somebody reads before pressing a
button. "Deadline" is not a reason. "Missing the filing window voids the application,
and the last document is scanned" is.

**SAY WHY A COMPLETION IS A COMPLETION.** `record("completed", id, note: …)`
shows up as a row saying "complete <id>" with your note under it. A note that
says where you learned it — which message, what it said — is the difference
between a batch he can accept in one press and one he has to go and check.

**DO NOT ASK TO SETTLE YOUR OWN WORK.** There is no accept verb and no reject
verb, there will not be one, and your capability bundle does not carry the
trait either — an agent that could accept its own batch has confirmed nothing.
`suzatary proposals accept|reject` is the operator's, from a terminal, and the
Inbox is {{persona.name}}'s, from his own device.

**A MISSING REPLICA OBJECT IS NOT NECESSARILY A LOSS.** The only deletion this
system performs is a rejection; an object ADOPTED onto the chain is gone from the replica too, and
`git_diff_summary` says `adopted onto <chain> (accepted <when>)` for it. Read
that line before diagnosing a rewrite; a deletion it does not explain is worth
a closer look, so say what you found rather than the mechanism you assume.

**A todo you proposed and he has not settled is still visible to you**, because
`list_open_todos` reads the replica: the chain's objects plus every batch
standing. So check it before you author — a proposal you made an hour ago is a
todo you must not propose again, and a `todo_update` on it is better than a
second `todo_add`.

`todo_add` refuses an id whose newest content is in a batch nobody has settled,
NAMING THAT BATCH ("already STANDING in batch `<head>` … todo_update it"): one
proposal per todo is the difference between an Inbox somebody reads and one
they scroll past.

The `(unconfirmed)` marker in the app belongs to todos written onto the chain
before proposals existed; nothing you write now can carry it.

**`body`, `detail` and `detail_append` are TYPST MARKUP; every other field is
plain text.** Markup is compiled in a closed world before it is stored and
refused with its line and column if it fails, so backslash a literal `\$ \@ \#
\* \_ \< \[`. Quote mail in the `source_*` fields, which are plain text.

- `todo_id` — short, unique across the whole repo. `todo_add` refuses a
  duplicate rather than shadowing the existing one.
- `body` — the action. `detail` is the optional second paragraph.
- `rationale` — REQUIRED, and the one thing no mechanism can recover: why this
  fulfillment and not another. It is recorded as data on the priority, not as a
  floating comment, so a later run can read it back.
- `kind` — one of the kinds `context` lists: `todo` (full control),
  `suzateryTodo` (exec suzatery), and a kind named `<category>Todo`, which
  files under that category and takes none of its own.
- `category` — drives which section it lands in; pick the right context.
- `priority` — a FULFILLMENT, 0–100%: LOW = unfulfilled = urgent (red, top);
  HIGH = satisfied = someday (blue). See the deadline rule below.
- `waiting_on` — "person/thing" if blocked; grayed and sunk to the bottom.
- `source_*` — where it came from. `source_hash` is the canonical hash from the
  `new`/`search` row; the first 8 hex characters are enough. `show` accepts it
  (full or any unique prefix), so the source message can be pulled back up
  months later. That only works if the message was marked — `mark` records the
  notmuch locator behind the hash. Pass `native_id` (the `id` from the same
  row) when you mark, or `mark` has to rescan the maildir to find it again.

**THE SCALE HAS ANCHORS** (the owner, 2026-09-09). Price against this table, never
by feel; a number outside the row its consequence belongs to is wrong even if
it feels about right:

{{persona.anchors_table}}

So a reply about speaker availability is not 15%, and an errand is not 40%.
Ask which ROW the consequence of missing it belongs to, then pick within it.

A deadline bounds WHEN a thing can be done, not how much it matters: the
fulfillment at the deadline (`deadline_end_priority`, or a flat `priority` on a
dated item) must still encode the STAKES. A trivial dated item — an RSVP, a
dining form, a survey with a cutoff — stays at 60%+ even on its last day
(the owner, 2026-08-25: "dining preferences is not urgent no matter what"). If
missing the deadline would genuinely hurt, low %; if it would merely be moot,
high %.

To revisit one you or an earlier run wrote: **`todo_show(todo_id)`** returns its
current fields, and **`todo_update(todo_id, ...)`** changes only the ones you
name. Changing the price requires a fresh `rationale` — a reprice without a
reason is exactly the edit that loses the information. `clear_waiting_on=True`
unblocks; that is the usual update, when a reply resolves what a todo was
waiting on. When a todo already has a `detail`, use `detail_append` — never
`detail` — to add a fact under it: `detail` replaces the field wholesale and
would flatten any markup already there, while `detail_append` keeps it and
adds your sentence as a new paragraph.

## Context you are given
**`context()`** is your first call: this file and the exact vocabularies the
authoring verbs accept. CLAUDE.md is already your project memory, so it is not
carried.

The answer is capped at 24 KB and built to leave a fifth of that free. Anything
that does not fit is CUT with a line saying so — if you see one, say so in your
run summary. `app/cli/tests/` weighs the real answer, so a manual that outgrows
its share fails a test, not a triage run.

## Completing / cancelling / reopening a todo
A todo's lifecycle state is the chain's fold, never a field you write.

To close, cancel, or reopen a todo you author or find, call the **`record`** MCP
verb: `record("completed" | "cancelled" | "reopened", "<todo-id>", note: "...")`.
Your events are recorded as actor `triage` and go into the todo's BATCH, not
onto the chain (§10.6): they change nothing until {{persona.name}} accepts them in the
Inbox. You cannot backdate and cannot forge a trusted origin; that is by design.
A todo you author THIS run has no Created event yet (the mechanical Created step
runs at push time), so you cannot `record` against a brand-new id in the same
run — expected, not a bug.

⚠️ A `record` against a todo that is ITSELF still only proposed rests on that
proposal, and the Inbox says so on the row. It is not
forbidden and sometimes it is right; it is just worth knowing that you have
handed him two decisions instead of one.

`record` is a lifecycle event, `todo_update` is the todo's text and price. To
close something, `record`; to reword or reprice it, `todo_update`. Neither one
is spelled by editing a file.

**You have no `undo` or `redo` verb, and there is no MCP twin to ask for.**
Undo is an append of the inverse, human-only like `send`: your writes are a
batch {{persona.name}} accepts or rejects, and reversing an accepted act is
simply another proposal.

## Workflow (sequential)
1. `suzatary sync`
2. `suzatary new` → the queue.
3. Skim by sender/title; `show <id>` only the ones whose body you need.
4. Decide each: author todos for actionable items; `mark` everything so it never
   resurfaces; `noise add` recurring junk senders.
5. `list_open_todos` before you author, so you `todo_update` an existing todo
   (e.g. a reply that resolves a `waiting_on`) instead of duplicating it. It is
   the chain's own fold — not a scan of the typst — so it is complete by
   construction and its priorities are evaluated as you read them.

Note: `list_open_todos` never shows resolved todos at all, and every todo ever
authored stays on the chain.
