# Many prodromes, many repositories — 2026-09-27

The owner, 2026-09-27: "collaboration with the prodrome … we should support
having multiple prodromes in multiple git repos that are connected, think of
for example the prodrome for the project, and the prodrome for the suzatary,
this notion of a multiprodrome app can instead be the generalization of what
we have here". A DESIGN for discussion, not a series yet: §6 lists the
decisions it needs.

## 1. What we already have is the special case

The app already hosts several stores and connects them, all inside one data
repository:
- **`life` and `suzatary`** (§10.2): two chains, each with its own trust
  policy and roster, named in `todo:<chain>/<id>`.
- **The phone** (§5): a replica. It seals on its own head, and sync is the
  union of content-addressed objects plus a `Woven` merge where heads diverged.
- **The triage agent** (§10.6): a replica of the chain it proposes to. A run is
  a branch, and accepting it is adoption, a merge.

Every one of these is the same idea: **a store is a grow-only set of
content-addressed objects, and a replica is another copy of that set that
syncs by union.** The multi-prodrome app says that once, and lets the copies
live anywhere, including other people's git repositories.

## 2. The claim that makes git the transport

A store on disk is `objects/<sha256>.py` (immutable, named by content) plus
`HEAD` (one mutable file) and `refs/` (one file per head while there are
several; SPEC §3).
- Two clones that only ADD object files merge in git with no conflict. The
  same name means the same bytes, and different names never touch.
- The only thing git can conflict on is `HEAD`.

**So make the heads DERIVED.** A store's tips are the objects no other object
names as a parent: a pure function of `objects/`, and a fold reads them all
anyway. `HEAD` and `refs/` stop being tracked; they become a local cache, or go
entirely. Then:

> A prodrome directory under git is a grow-only set of files, and git's
> merge of two clones IS the Prodrome's union. A `Woven` object is sealed
> the next time anyone writes, the same way the phone's divergence is today.

This is a Prodrome change (SPEC §3: "tips are derived"), and a small one. No
stored byte moves: an old store's `HEAD` names a tip the derivation also finds.

## 3. What a "prodrome" is, to the app

- **Identity.** A prodrome is named by its GENESIS object's hash, not by
  where it lives or what I call it. That is the stable, global name: two
  clones are the same prodrome exactly when they share a genesis.
- **A mount.** The host's config says where each prodrome lives and what to
  call it locally:
  ```toml
  [[prodromes]]
  name   = "life"                           # local, for URLs and ids
  repo   = "<the data repository>"          # a git checkout
  path   = "events"                         # the store inside it
  genesis = "dcc2e939…"                     # checked on open: a wrong mount stops the server
  role   = "personal"                       # briefed, notified; see §4
  sync   = "own"                            # this host commits and pushes

  [[prodromes]]
  name   = "fpl"
  repo   = "<a private research repository>"
  path   = ".prodrome"                      # the project's todos travel with its code
  genesis = "…"
  role   = "project"
  sync   = "branch"                         # see §5
  ```
- **What generalizes:**
  - `[[chains]]` becomes `[[prodromes]]`.
  - `ChainName` stays the local slug.
  - `todo:<name>/<id>` keeps its text form.

## 4. Connections between prodromes

- **Text references.** `todo:fpl/runablation` already parses, because the
  chain is part of a todo's name. Nothing new is needed, except that the name
  resolves through the mount table.
- **Composition across stores.** This is the part that makes it worth it:
  "submit the paper" (life) waits on "run the ablation" (fpl).
  - **Today:** `Ref(todo)` is store-local (SPEC §7.2).
  - **Proposed:** `Ref(todo)` gains a qualified form, `Ref(prodrome, todo)`,
    where `prodrome` is a GENESIS PREFIX, never a local name. The stored
    term must mean the same thing on every host.
  - `link` takes specs from every mounted store. An unmounted prodrome is
    `LinkError::Unknown`, which the app already shows as "unlinked", the same
    as a missing todo.
  - `Ref` means the other todo's whole function, so a completion there reads
    1.0 here from its instant. Time-indexing already works across stores.
- **One list across mounts.** The Todos page can show every mounted prodrome
  together, priced on one scale, grouped by prodrome or category, with p = −4
  across them. The briefing and push read `role = "personal"` mounts, where
  today they read "the first chain".

## 5. Collaboration: who may write, and how it arrives

There are two stages, because trust is the hard part.

**Stage A: collaborators' writes arrive as batches (no new crypto).**
- The shared project prodrome lives in the project's repository. A
  collaborator works in their own clone: their own app, or plain `suzatary`
  on the CLI.
- **The branch is the proposal.** They push a branch, or a PR. The host
  fetches it and sees the objects it lacks, exactly as it sees a triage batch
  today, and the Inbox shows it as a batch from that person.
- **Accepting is adoption.** It happens on the host's own branch, which it
  pushes (`sync = "branch"`).
- **Nothing binds without the owner's merge.** This is §10.6 with git as the
  replica, and it is PR review for todos.
- Actor names in those objects are CLAIMS: anyone with push access can type
  any name. Stage A is safe because the owner adopts by hand, never
  automatically.

**Stage B: a shared roster, signed (after `devicesignatures`).**
- **Signed writes.** Every object carries a signature by a device key, and
  the prodrome holds its own roster as events: `Joined(actor, key)` and
  `Left(actor)`, which only existing members bind.
- **Every host agrees.** A host's policy becomes "binds if signed by a
  current member", so every collaborator's app reads the same confirmed
  state, and pushes need no review.
- This is the `devicesignatures` stage (not before 2 October) applied to a
  store more than one person writes.
- Until Stage B exists, a project prodrome in Stage A has one binding writer
  per host: the owner's.

## 5b. The inbox is a prodrome too (the owner, 2026-09-27)

The owner: "the Secretary might want the ability to produce a proposal based
on an fpl value (something may depend on something else), which means it
might make sense to have a prodrome for the proposals with a fulfillment val
and all telling us also which notification to handle first".

**Half of it exists.**
- Every request for the owner's input is already a decision record on the
  chain (`kind: "decision"`, `authoring::build_decision`): a changeset, an
  email to send, a held batch, a secret, an act.
- It has a lifecycle (open, then settled or dismissed), a register and a
  price.
- CLAUDE.md keeps chat TURNS off the chain because a turn has none of those.
  A decision has all three, so it belongs on a prodrome.

**What is missing:**
- **Prices are flat.** A decision is 55% unless the secretary names a number,
  so the Inbox order and the push say nothing about urgency.
- **No dependence.** A decision cannot say "as urgent as the todo I serve",
  or "only once that other thing is done".
- **Pushes are indiscriminate.** `notify::needs_you` rings for every new
  decision, whatever its value.
- **The history is mixed.** Decisions live in `life`, so its history mixes
  the owner's todos with every proposal the secretary ever made.

**The design:**
1. **An `inbox` prodrome**, a third local mount (`role = "inbox"`), holds
   every decision.
   - Its policy: the secretary and triage write, and only the owner settles
     (a tap is a binding completion or cancellation).
   - `life` and `suzatary` keep only their own todos.
   - This is the first use of mounts that needs no git collaboration at all.
2. **A decision's price is FPL, and usually a `Ref`.**
   - "Reply to counsel about the renewal" is priced `Ref(life:renewal)`, so it
     is exactly as urgent as the todo it serves, and falls with it as the
     deadline nears.
   - A decision with its own clock composes: the power mean of the served
     todo and its own deadline.
   - This is §4's qualified `Ref`, across stores.
3. **Conditional proposals.** A decision can be filed DORMANT: shown and
   pushed only once its price falls below the Watch line. "When the passport
   todo drops under 40%, propose booking the appointment" is one decision
   whose price is `Ref(passport)`.
   - It needs no scheduler; the curve IS the schedule.
   - A decision that depends on another decision is `Ref` to it, the same way.
4. **Notifications follow the value.**
   - A decision rings when it enters the Problem band, whether it is new or
     already waiting and its curve crossed. It never rings merely for
     existing.
   - Several waiting ring as one push that names the most urgent.
   - The Inbox sorts by fulfillment, lowest first, with the same colour ring
     as the list.
   - The watcher already evaluates on a tick, and a crossing is a comparison
     of two instants.
5. **The Inbox is a view over the `inbox` mount**, grouped by what each
   decision serves (its `Ref`), so a todo and the decisions waiting on it
   read together.

## 6. Decisions this needs

1. **Git as the only collaboration transport.** Box-to-box HTTP stays for the
   phone. *Recommend: yes.* Git brings auditability, offline work, PRs and
   hosting the owner already has.
2. **Derived heads in the Prodrome** (§2): public Prodrome 0.9.0, with the
   SPEC change and vectors. *Recommend: yes. It is the enabling change.*
3. **Qualified `Ref`** (§4) in the same minor. *Recommend: yes. Name by
   genesis prefix.*
4. **Where `suzatary` (the project chain) lives.**
   - *Option a:* keep it in the data repo.
   - *Option b:* move it to `.prodrome/` in the Suzatary code repository,
     so the roadmap travels with the code.
   - *Recommend b later, not now.* The project chain's history still holds
     personal context. It moves when the code is made public, from a fresh
     genesis that references the old one's open todos (§4).
5. **Stage A now, Stage B after device signatures.** *Recommend: yes.*
6. **The inbox prodrome (§5b)** as the FIRST new mount, before any git
   collaboration. *Recommend: yes.* It needs only derived heads, local
   mounts and the qualified `Ref`, and it improves every day (which
   notification first), where collaboration waits on a collaborator.
7. **The first real second prodrome.** *Recommend: a private research
   repository*, because it has an obvious cross-store dependency (paper ↔
   experiments) and no collaborator yet. That makes it Stage A with one
   writer, the smallest real test.

## 7. The series, once §6 is settled

| # | Where | What |
|---|---|---|
| 1 | Prodrome | derived tips (SPEC §3), laws: tips of a union = the union's derived tips; git-merge of two clones folds as the Prodrome merge (property test over two random writers) |
| 2 | Prodrome | qualified `Ref(genesis, todo)`, link over many stores, `Unknown` for an unmounted one; vectors; 0.9.0 (publishing needs the owner's OK) |
| 3 | Suzatary core | `Mount { name, genesis, role, sync }`; `[[chains]]` → `[[prodromes]]` with a data migration of the instance file; genesis checked on open |
| 4 | Suzatary cli | per-mount git sync: `own` (commit + push its writes) and `branch` (fetch branches, list their objects as batches, accept = adopt + push); the triage agent's replica becomes one more branch |
| 5 | web | the list across mounts; a batch card says whose branch it came from; `todo:` links resolve through mounts |
| 6 | docs + box | mount the research repository; one cross-store `Ref` from a life todo; the owner's review |
