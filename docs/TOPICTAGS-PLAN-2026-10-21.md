# TOPICTAGS PLAN: one stream, with topic tags (2026-10-21)

*Status: stage 1 is built (this plan's pull request). Stages 2 to 5 are
designs.*

Sources read: `CLAUDE.md`; `AGENTS.md`; `docs/ARCHITECTURE.md` §10.3;
`docs/WORDS-PLAN-2026-10-09.md` §0 and §1; `app/core/src/secretary/`
(`line_event`, `transcript_registers`, `store/stored.rs`, `taint.rs`,
`memory_event`, `memory_registers`, `fact_life`, `two_phase`,
`verbs/draft_note.rs`, `salted_text`, `texts`); the web's
`web/src/prodrome/stores.ts`, `objects.ts`, `replica.ts`, `chat/rounds.ts`,
`pages/secretary.ts`; `android/` for every `/api/` call; and the Prodrome at
the pinned `de71ccc`, `docs/design-register-types.md` §1 to §6.1 with
`core/src/fold/{order,register,product}.rs` and `store/{replica,memory_store}.rs`.
The brief says a roadmap names a mergeable tag sets item. I searched
`ROADMAP.md` and every file under `docs/` here, and the Prodrome's `README.md`,
`SPEC.md`, `CHANGELOG.md` and `docs/` at `de71ccc` and at its newest commit
`fba46ae`, for "mergeable", "tag set", "OR-set" and "observed-remove", and found
no such item. This plan builds the set from what the Prodrome already has.

## 0. The decision

The owner decided (2026-10): the secretary is ONE stream, not separate
conversations. Rounds and notes carry topic tags. One round can carry several
tags. The page leads with a tag bar, and the tags in it combine into a filter,
so the tags look like threads but are topics. The tags on one target are a
mergeable set: two devices that add and remove tags merge to the same set.
A rename is an alias, so renaming a tag keeps every round it was on. The
secretary tags a round as it answers it, and the owner edits tags by tap.

Terms used below:

- A **round** is a question and its answer. It is named by the index of the
  turn that opened it, as `web/src/chat/rounds.ts` keys a round and as a
  remembered fact's `Origin::Turn` names its turn.
- A **note** is the turn that `draft_note` writes (a `Turn::Tool` with that
  verb).
- A **target** is a round or a note. A **membership** is one tag on one
  target. An **alias** says that one tag name stands for another.

## 1. The stored shape

### 1.1 Where tags live

Three places were considered.

1. **The conversation lines** (`secretary-lines/`). Rejected. A line's one
   register is the transcript, a grow-only set ordered by causality. A tag can
   be taken off again, so the lines' schema would need a second register of a
   different type. Every tag write would also rest on, and be ordered against,
   every line of its conversation, although tags of two rounds never interact.
   The stream also crosses conversations, so a tag's natural key is not a
   conversation.
2. **The inbox** (`inbox/`). Rejected. The inbox holds proposals that wait for
   a tap and climb a state machine. A tag is not a proposal, and the inbox's
   state register is inflationary, so it cannot take a tag off.
3. **Stores of their own.** Chosen. The Prodrome is generic over a schema, and
   each schema here is small. A tag write rests only on the earlier writes to
   the same membership, so writes to different memberships commute.

There are two stores, not one, because a Prodrome store is opened at one
schema and the two registers hold values of different types:

| Store | Schema | Key | Register and its order |
|---|---|---|---|
| `secretary-tags/` | `MembershipEvent`: `Tagged(target, tag, at, actor)`, `Untagged(target, tag, at, actor)` | `Membership { target, tag }` | `Presence`, total: `Off < On` |
| `secretary-tag-aliases/` | `AliasEvent`: `Aliased(tag, at, actor, to)`, `Unaliased(tag, at, actor)` | `Tag` | `Alias` (`Itself` or `As(tag)`), discrete |

One schema with a sum of the two keys would let a membership key carry an
alias value, which is a state that must not exist.

### 1.2 Box-local, vaulted, never pushed

A tag name is words: the migration (§4) makes tags from conversation titles,
and `docs/WORDS-PLAN-2026-10-09.md` §1.1 classes a title as locked words. A
pushed store carries no words (that plan, §0). So both tag stores are box-local
and never pushed, like `secretary-lines-keys/`. The daily vault carries them,
`safe-push.sh` refuses them, and a guard test pins both facts. The phone
syncs them from the box over the tailnet, as it syncs `lines` today.

The alternative is to name each tag by a salted `TextName` and keep its words
in a text store, as the memory does. That alternative makes a tag's words
erasable. It also has two costs. A salted name has a random salt, so the same
words written on two devices give two names, and only the box can mint a name
under a lock. So a phone offline could not make a new tag. And the merge law
would hold for names, while two tags with the same words could still exist.
The owner chose the box-local stores (Q1, answered 2026-10-22).

### 1.3 One spelling for each value

- A `Tag` is one or more words of lowercase letters and digits, in any script,
  joined by single hyphens, at most 48 characters, in Unicode NFC.
  `Tag::new` refuses every other text, so `Travel`, `travel ` and `travel`
  cannot be three tags. `Tag::slug` turns free text into the tag a person
  would write for it, and a tag's own text slugs to itself (a proptest).
- A `Target` prints and parses as `round:<conversation>/<turn>` or
  `note:<conversation>/<turn>`, with the index in decimal and no leading zero.
  The same text is the object's literal and, from stage 3, a `Reference`.
- A `Membership` is the pair, and it is the membership store's key, so a pair
  has exactly one register.
- `Alias::As(t)` is never the tag's own name: `AliasEvent::aliased` refuses
  it, and the parse goes through that constructor. "This tag is its own name"
  has one spelling, `Unaliased`.

## 2. The merge law

**A membership reads its frontier** (the Prodrome's design §2): the writes to
that pair that no later write to it descends from. The reading is the greatest
value among them, with `Off < On`. A write rests on the pair's heads in the
writer's replica, so it supersedes exactly the writes its writer saw.

- An add, then a removal on a device that saw the add: the frontier is the
  removal. The tag is off.
- A removal on one device and an add on another that the removal did not see:
  the frontier holds both, and the greatest is `On`. The tag is on.

This is the observed-remove set (the OR-set of Shapiro and others), with "add
wins" for concurrent writes. The Prodrome's causal deps are the set of
observed adds, so the store needs no unique tags of its own and no tombstones.

Why this law and no other:

- **Last writer wins by a clock** is refused by the constitution: no clock
  decides who wins (CLAUDE.md, "Order is causal, time is data").
- **A two-phase set** cannot put a tag back on once it is taken off.
- **Remove wins** loses a tag that a device added without seeing the removal,
  so an offline phone's tap would vanish without a sign.
- **A state-based set per target** (each write carries the whole set) makes two
  concurrent edits of one target a conflict that a person must settle. The
  OR-set settles it by the rule above.

**Every device computes the same reading, offline included.** The reading is
a function of the two object sets (`Reading::of`), and a merge is the union of
object sets (`prodrome_core::store::sync`). Union is commutative, associative
and idempotent, so devices that hold the same objects read the same tags,
whatever order the objects arrived in. A phone writes in its own replica with
the same planner the box uses (stage 2 compiles it to the wasm module) and
syncs later.

The membership register is not inflationary, because a removal goes down. No
append is refused, and the store needs no lock: every tag write is monotone in
the CALM sense (design §6), so no write needs a decision.

The tests in `app/core/tests/tags.rs` state each law over real in-memory
replicas: `merging_is_commutative_associative_and_idempotent`,
`an_add_and_a_remove_apart_merge_the_same_way_in_either_order` (against the
OR-set's model of each pair), and `a_second_run_writes_nothing`.

## 3. Aliases

A rename writes aliases and no membership. A membership keeps the name its
writer wrote, and the reading follows each name's alias to the tag it stands
for. So a rename keeps every round the tag was on, and a device that tagged
under the old name while another device renamed it lands in the renamed tag
after the merge (`a_tag_under_the_old_name_lands_in_the_renamed_tag`).

**The rule.** `Reading::rename(from, to)` makes every name of `from`'s tag,
and every name of the tag `to` stood for, an alias of `to`, and makes `to` its
own name. So:

- a rename to an unused name renames;
- a rename to a name of another tag merges the two tags under that name;
- a rename of `b` back to `a`, after `a` was renamed to `b`, undoes it;
- the same rename again writes nothing.

**Taking a tag off** writes `Untagged` for every name of the tag that the
writer sees on the target, so a membership written under an old name comes
off too (`a_second_run_writes_nothing`).

**Disputes.** Concurrent renames can disagree in two ways. The reading always
answers one tag for each name, by a rule every device applies alike, and lists
the dispute so the page can show it:

- *Two aliases of one tag* (`Dispute::Aliases`): device A renames `travel` to
  `trips` while device B renames it to `work`. The tag's register holds both,
  and the reading follows the alias whose write has the least object name.
  This is the house rule for a reading that can show only one record.
- *A ring* (`Dispute::Ring`): A merges `travel` into `trips` while B merges
  `trips` into `travel`. Every name of the ring reads as the least name of the
  ring.

The next rename of the tag settles either dispute, because each of its writes
supersedes what its writer held for that name
(`two_renames_into_each_other_read_as_the_least_name_until_one_settles_them`,
`two_renames_of_one_tag_apart_are_a_dispute_every_device_reads_alike`,
`a_rename_keeps_every_membership`).

## 4. From conversations to one stream

**The stream is a reading, and no line is rewritten.** The lines stay in the
conversations they were written in, with their words locked under their keys.
The WORDS plan keeps every pushed object immutable (§0), and a carried copy of
every line would need new keys for every word. The page reads the stream as
every live round of every conversation, ordered by the instant of the turn
that opened it. From stage 5, every new round is asked in the `default`
conversation, so the stream grows in one place.

**The carry verb.** `suzatary secretary tags carry` writes each conversation's
title as the first tag of its rounds and notes. It has the project's usual
shape for a one-time migration: an explicit verb, run once on the box with the
owner's yes, then deleted with its tests. It writes only into the new
membership store, and it changes no stored format, so no old reader or old
store needs deleting.

- For each conversation of kind `Topic`, the tag is `Tag::slug` of its latest
  title (`Titled`, else `Opened.title`). When the title has no letter or digit,
  the tag is `Tag::slug` of the conversation's id, which always starts with a
  letter or digit, so every topic gets a tag.
- A `Review` conversation gets the tag `review` or `sweep`, by its daily kind,
  so the box's own rounds file under two tags and not one per day.
- The `default` conversation gets no tag. **Question Q3** asks whether it
  should get one.
- It writes `Tagged(round:<conversation>/<turn>, tag)` for each live round
  (`Stored::live`) and `Tagged(note:<conversation>/<turn>, tag)` for each
  `draft_note` turn, as the actor `assistant`, through `Reading::tag`. A
  membership the store already reads on plans no write, so a second run
  writes nothing.
- `--dry-run` prints, per conversation, its tag and how many rounds and notes
  it would tag, and writes nothing.

**Run order.** Deploy stage 2. Show the owner the dry run. With his yes, run
the verb once on the box and check that a second run writes nothing. The PR of
stage 5 deletes the verb.

**The tests** prove it on fixtures only: a fixture instance under the test's
scratch directory with three conversations (a topic, a review, the default),
the dry run's text, the writes, the second run writing nothing, and a title
with no letter falling back to the id.

## 5. The secretary's verb

The `Secretary` record gains the membership store's writer, and the verb list
gains `tag`:

- `tag { tags: [text] }` puts at most three tags on the round the secretary is
  answering. The target is the current round, derived by the box and never an
  argument, so the model cannot tag another round. Each text goes through
  `Tag::slug` at the edge, and the answer names the tags in their spelling, so
  the model learns the one spelling.
- The verb takes nothing off and renames nothing: removal and rename are the
  owner's, by tap.
- The session opening tells the model the tags in use, so it reuses a name and
  does not make a near twin. A tag is at most 48 characters of letters, digits
  and hyphens, but it is still text that a round that read mail can choose.
  So the opening tells only the tags that carry a membership by the owner or
  by a round with no taint (`secretary::taint`), and tells the others by count.
  **Question Q4** asks whether that rule is right.
- `SECRETARY.md` gains a short section on when to tag. The verb snapshot
  (`app/core/tests/secretary-surface.json`) moves with it.

## 6. The page

- **The tag bar** stands above the stream: the tags in use, the most recent
  first. Tapping a tag adds it to the filter or takes it out. The filter holds
  one or more tags and reads as "every tag of the filter": a round shows when it
  carries all of them. With no tag chosen, the page shows the whole stream.
- The filter is a register of the route (`#/secretary?tags=a,b`), so a deep
  link prints it and Back moves it, as the Prodrome's design §6.0 says of
  navigation.
- **Each round shows its tags** as chips. A tap on a chip takes the tag off;
  an add control puts one on, offering the tags in use first. A rename is a
  sheet on a tag in the bar, and it shows the merge when the new name is a
  tag already.
- **Asking with a filter** tags the new round with the filter's tags, as the
  owner's writes, so a question asked inside a topic stays in it.
- **A dispute** shows a small mark on the tag in the bar, with the names in
  dispute and a Rename that settles it.
- The page folds the two stores with the wasm module's `Reading`, so it
  computes the filter offline and writes a tag offline. Every law lands in
  `docs/WEB-SPEC.md` with the node test that pins it, over `web/test/gen.ts`,
  and the Rust writes vectors under `app/core/tests/web/` that the page's
  filter must agree with. The page is written in `docs/TS-STYLE.md`'s style.

## 7. Stages

Each stage is one pull request, a series of commits, each commit buildable
and tested. Android needs nothing in any stage: the APK calls only
`/api/author`, `/api/act/device` and `/api/fold`, and no stage changes them.

1. **The core** (this pull request). `secretary::tags`: `Tag`, `Target`,
   `Membership` and `Presence`, the two schemas and their registers, `Dispute`,
   and `Reading` with its planners. The laws are proptests over in-memory
   replicas. No store on disk, no route and no page. No stored format changes,
   so this stage has no migration verb.
   *A phone on its first load after stage 1:* nothing changes. The box serves
   the same routes, the same bundle and the same contract (`CLIENT_MIN` stays
   5), and the phone's replicas (the chains, `inbox`, `lines`) read as before.
2. **The stores.** `[secretary] tags` in `instance.toml` names the two
   directories; `suzatary init` begins them; the box opens them; the vault
   carries them; `safe-push.sh` refuses them, with a guard test. The tab gets
   two more store names in `web/src/prodrome/stores.ts`, served by
   `/api/stores/{tags,tag-aliases}/objects` and `…/events`, and a write route
   that takes a replica's objects under the same gate as `POST /api/objects`
   (a named tailnet peer in `web.act_as`, the CSRF triple, a trusted device
   key, the actor the peer maps to). The wasm module gains the two schemas and
   `Reading`. The carry verb ships here, and the owner runs it (§4).
   *First load:* the box offers the new bundle (`newbuild.ts`). Before Reload,
   the old bundle never asks for the new stores, and its routes are unchanged,
   so it works as before. After Reload, the tab pulls the two stores into its
   database (a new store name, so the database version moves to 5 and a
   version 4 row is carried whole). The page draws nothing new.
3. **The secretary tags.** The `tag` verb, the session opening's list of
   tags, `SECRETARY.md`, and `Target` in the `Reference` grammar.
   *First load:* nothing new is drawn. The replica of `secretary-tags/`
   grows as the secretary answers.
4. **The page.** The tag bar, the filter, the chips, the rename sheet and the
   stream view of every conversation's rounds. New topic stays for this stage,
   so nothing an old bundle sends is refused.
   *First load:* the box offers the new bundle. After Reload, `#/secretary`
   opens on the stream with the tag bar, every round with its title's tag if
   the carry ran. An old bundle still lists conversations, because its
   routes are unchanged.
5. **One stream.** Every new round goes to `default`; `POST
   /api/secretary/conversations` and the conversation list are deleted, with
   their tests; the carry verb is deleted. `CLIENT_MIN` moves to 6, because an
   old page would offer New topic, which the box no longer has.
   *First load:* an old bundle's next request gets 426, and the page says
   "Suzatary <version> is ready" with Reload; the act it tried was not sent.
   After Reload, the page is the stream.

## 8. Questions for the owner

- **Q1. Box-local tag stores, or salted names?** Answered 2026-10-22:
  box-local and vaulted, never pushed. Tags merge by their words, a phone
  makes a tag offline, and a tag's words cannot be erased.
- **Q2. Does erasing a conversation erase its title's tag?** With box-local
  stores the tag stays on the stream's other rounds. A tag carried from a
  title is a copy of the title's words, as the WORDS plan §4.2 counts copies.
- **Q3. Should the `default` conversation's rounds get a tag in the carry?**
- **Q4. Which tags does the session opening tell the model?** The plan says
  the tags with a membership by the owner or by an untainted round.
