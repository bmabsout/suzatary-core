# Simplify: the types tell the story

The owner, 2026-09-28: "What the app and the prodrome does at a core level is
not that complex, in theory we should be able to write these programs without
the code being very large, comments can be sparse if the types are expressive
and the function names and vars are expressive. … it should be easy for a
mental model to be formed by the types and folder names."

The model is his scanman (`common/shared/{db,fire,lev,aggregates}.ts`) and
PULER (`TypeSystem.hs`):

1. **The data is one type.** The whole store written out as one nested type.
   A typed path into it and a few generic verbs are the persistence layer.
2. **One sum, one small algebra, reused.** Loading, error and value are one
   type with map, flatten, combine and default, drawn by one view.
3. **Logic is combinators.** Monoids and semilattices, not case analysis
   written out again for each caller.
4. **Effects are values.** A view or a function receives the effects it may
   use as an argument.
5. **Laws live in types and instances.** A comment records a decision the
   code cannot state, and nothing else.
6. **A file is its main type.** One concept per file, the file named for it,
   the type declared first. The layouts below name files by the type they
   open with.

## Where the weight is (read-only maps, 2026-09-28)

| Area | Lines now | Target | The repeated idea |
|---|---|---|---|
| `app/core` | 59k (19k tests, 10k comments) | ~42k | wire mirrors of domain types; hand-parsed request bodies twice; price, fields and edits declared 3–4 times; two verb lists; six store traits; two model loops; two renderers |
| `app/io` + `app/cli` | 44k (+25k tests) | ~20k + ~18k tests | seven hand-forwarded capability bundles; 74 near-identical handlers; six HTTP hooks; six subprocess loops; three token caches; twelve path functions; `Raw*` config copies |
| `web/src` | 21.5k | ~12k | the wire declared twice (guards and generated types); 46 fetch wrappers; seven loading machines; the route switched on in six places; three conversation wirings |

## The target

### Core: one data type, generic verbs over it

```rust
pub struct Database {
    chains: BTreeMap<ChainName, Area>,          // Area { events, proposals: Prodrome<Record> }
    inbox: Prodrome<Asked>,
    conversations: BTreeMap<ConversationId, Log<conversation::Entry>>,
    memory: Log<memory::Line>,
    triage: Doc<TriageState>,
    devices: BTreeMap<Login, Log<Device>>,
}
```

`Prodrome<P>` (objects, append), `Log<E>` (lines, push) and `Doc<T>` (get,
set) are the only store shapes. A fold over a log is `trait Fold`. One
`StoreError` converts into `Refusal` once.

```
core/src/
  store/        Database, Prodrome<P>, Log<E>, Doc<T>, Fold
  todo/         record, price (one serde Price), fields (TodoFields, TodoPatch = Patch<A> per field), author, view, tree
  proposal/     asked, submit, card, batch, diff
  conversation/ entry, stored, queue, round (one model loop), memory
  verb/         trait Verb (typed args, derived schema, EFFECT, NAME); a surface is a typed subset
  mail/         as now
  render/       escape, decoration, reading, answer, site (Typst)
  route/        pure functions returning domain types that derive TS
  capability/   the effect traits
```

### Effects as values

The seven forwarding bundles go. A bundle is a record of the effects a caller
may use, so swapping one effect is struct update syntax:

```rust
pub struct Agent<'a> { chain: &'a dyn Chain, corpus: &'a dyn Corpus, mail: &'a dyn Mailbox, render: &'a dyn Render, clock: &'a dyn Clock }
let bench = Agent { render: &typst, ..fakes.agent() };
```

The record for the triage agent has no send-capable field. A compile-fail
test states it.

### Routes and verbs are data

```rust
pub struct Route<P, O> { method: Method, path: &'static str, scope: Scope, guard: Guard, run: fn(&Server, &Who, P) -> Result<O, Refusal> }
```

One adapter mounts every route (bare and `/{chain}/`). The guard lives in the
route, not in a comment. A golden test lists every (method, path, guard).

### io: one of each mechanism

`run(Cmd) -> Ran` for every subprocess; one `Http` trait with a ureq
interpreter and a scripted fake; `Bearer<G: Grant>` for tokens; `Paths::of`
as the only place a box path is derived. `instance.rs` deserialises straight
into typed fields. `bench` and `fixture` move to a `suzatary-dev` binary.

### Web: the wire once, pages as a table

```
web/src/
  wire/     generated types and decoders (emitted by wire.rs), http.ts, the route table
  algebra/  lo, source, tree, reconcile, keyed
  app/      App (the shell's whole state as one type), kv, time
  prodrome/ core (wasm bridge), replica, sync, objects
  view/     dom, rich, frame, act, controls, icons, marks, toast
  todo/ inbox/ chat/ pages/ shell/
```

Every page is a `frame.Page`. An act button is one `act.T` sum with one
view that also takes the owner gate. `pages.table` replaces the six switches
over `Route`. One `chat.mount` replaces three wirings.

### Comments

A comment records a decision the types cannot state, in a sentence. Dates,
plan references, quotes and history go to CHANGELOG.md and docs/. Every
stage prunes the comments of what it touches; a last pass does the rest.

## Must not change

- Stored bytes: record literals, the inbox `Asked` object (its `act` is serde
  JSON hashed into `ProposalId`, so a unified `Price` must serialise exactly
  as `changes::Priority` does), conversation JSONL, memory, `state.json`,
  devices, oauth and imap state, `instance.toml`.
- Deployment: the live clone, the data repo, `SUZATARY_INSTANCE`, the verbs
  units and `promote.sh` call, the MCP verb surface, bare `/api/*` paths.
- Security: a `Login` only from whois; the agent can only propose; sending is
  a human tap or the CLI, once (`inbox::submit` under its lock); taint;
  `Escaped` as the only door to HTML; secrets never formatted.
- The web's service worker, the CSRF triple, "a missing field is a break",
  the owner-only acts, the e2e selectors.

## Stages

Each is one PR, gated in full, with at most two in flight. The 2.0 items
`wireonce`, `typedverbs` and `cleanuppasses` are stages of this plan.

1. **testkit and io mechanisms.** A `testkit` dev-crate for the shared test
   code; `run`, `Http`, `Bearer`, `Paths`. No behaviour change.
2. **Effects as records.** Records beside the bundles, migrated one at a
   time, forwarding code deleted last.
3. **wireonce.** Domain types derive TS; mirrors deleted; `wire.rs` emits
   decoders; the web switches to them and deletes its guards and `wire.ts`.
   Needs the Prodrome's term schema (its plan, stage 2) for the FPL codec.
4. **Routes as data.** The route table and one adapter; the web gets its
   route table from the same list.
5. **typedverbs.** `trait Verb`, schemas derived and matched byte for byte
   against today's snapshots; `form.rs` and the arg parsers deleted. (Verbs
   done 2026-09-30, the `typed-verbs-2nokpa` report; `form.rs` parses the
   `/api/author` body, not a verb, and goes with stage 4's routes.)
6. **One todo shape.** `Price`, `TodoFields`, `TodoPatch`; an inbox
   round-trip check against live data first. (Done 2026-09-30,
   the `todo-shape-unification-ebvuwh` report; `form.rs` went with it.)
7. **One model loop, one Round.** (Done 2026-09-30,
   the `conversation-round-loop-suy9g8` report.)
8. **The store algebra.** `Database`, `Prodrome<P>`, `Log<E>`, `Doc<T>`; the
   io stores become their interpreters with file layouts untouched. (Done
   2026-09-30, #26.)
9. **The web shell.** `app`, `frame` for every page, `act`, `chat.mount`,
   `form`, `pages.table`, `time`, `kv`, tags spelled `type`. (Done 2026-09-30,
   #17, #18, #20, #22, #23.)
10. **Typed config and World**, `suzatary-dev`, the site as Typst, the
    comment pass, and a CLAUDE.md section on this style. (Done 2026-09-30:
    #25, the site #29, the comments and Style section #28.)

The Prodrome's own plan is `docs/SIMPLIFY-PLAN.md` in its repository.
