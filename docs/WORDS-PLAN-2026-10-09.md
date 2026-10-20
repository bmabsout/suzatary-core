# WORDS PLAN: pushed stores carry no free text (2026-10-09)

*Status (2026-10-12): stages 0 to 3 are built and run (the box carried the
conversations into `secretary-lines/` on 2026-10-11); stage 4 deletes the plain
store's reader and the digest's `Erased` (§6.4 says as built).* Written as a
design. Read against `origin/master` 2db5fedf with #111 (Kept and
Scoped lines, `Stamp`, `zoom`) and #119 (`Unheard`) taken as landed. It closes
finding 1 of both privacy audits (`priv-opus.md`, `priv-fable.md`), and the
`secretary-memory-erasable-*` reports' open question about the conversations.

## 0. The decisions this rests on

- **Fix forward** (owner, 2026-10-09). The conversations, and then the inbox,
  stop pushing words.
- **No history rewrite as part of the design** (owner, the same day: "maybe
  forgetting should be an append?"). Forgetting appends an event and destroys
  box-local material. History only grows. The rewrite is a separate option,
  kept apart in §7 for him to decide later.
- **Immutability and persistence stay** (owner: "I like the immutability and
  persistency of the datastructure"). Every object stays content-addressed,
  signed where it is signed today, and immutable. No migration rewrites or
  deletes a pushed object. The forward migration writes a NEW store whose
  objects point back at the old ones they carry. The old store stays where
  it is, frozen, and nothing reads it.
- The house rules apply: types first, sums over bools, no defensive code, no
  backward compatibility (migrate, then delete the reader), one concept per
  file, and delete more than you add.

What append-only cannot do, said once so the rest of the plan does not hedge:
**words already pushed in plaintext cannot be erased by appending.** They
stay in the private repository's history, on GitHub and in every clone until
the owner chooses the rewrite in §7. Everything below protects words written
after the cutover, and makes that rewrite, if it ever comes, a plain
path-drop that renames no surviving object.

## 1. What holds free text, field by field

"Words" here means free text from the owner, a model, mail, documents or
other people. "Shape" means what is left when the words are taken out: ids,
hashes, instants, counts, closed sums, actor names, verb names and object
names. In the class column, **leaves** means the field is pushed in the clear
on purpose, with the reason given. **locked** means the words go behind a
box-local key, using the mechanism in §2.

### 1.1 The conversations (`secretary-conversations/`, about 4,636 objects, pushed)

| Line / field | Words from | Class | Stage |
|---|---|---|---|
| `Said` `Turn::Person.text` | owner (and the daily sweep's composed ask, which embeds whole mail bodies, `sweep.rs` `user_turn`) | locked | 2 |
| `Turn::Person.attachments[].name` | owner's file names | locked | 2 |
| `Turn::Model.text` | model | locked | 2 |
| `Turn::Model.calls[].args` | model (a `remember` call's `fact`, the `propose_email` body, `draft_note`, search queries) | locked | 2 |
| `Turn::Tool.answer` | verbs: `show` and `search` mail, `drive_read`, `attachments`, `memory` (the main session's whole memory snapshot, `agent/reads.rs`), `recall`, `zoom` (#111: lines and digest summaries, which are box-local today and get re-pushed through this answer), card chips (`mod.rs`: a referenced card re-rendered whole, email draft included) | locked | 2 |
| `Turn::Tool.files[].name`, `.text` | the file's name and its text layer (`Enclosed.text`) | locked | 2 |
| `Turn::Act.text` | box sentences that quote headlines (recipients, subject) | locked | 2 |
| `Opened.title`, `Titled.title` | model (the title job) or owner | locked | 2 |
| `Reviewed.why` | secretary, about mail batches | locked | 2 |
| `ReviewFailed.reason` | the model's refusal, verbatim | locked | 2 |
| `Coding.session` `Dropped.reason`, `Reported.report` | owner; coding agent | locked | 2 |
| `Session`/`Kept.cwd`, `.model`, `.surface`; `Scoped`; `Unheard`; `Stopped.by`; `Rewound`; `MemorySeen`; `Proposed`; `Said.by` (`Stamp`); `Turn` `references`, attachment ids, verb names, `refused`, file ids, media and size; `Reviewed.chain`, `.batch`, `.decision`, `.objects` | shape | leaves: the fold, taint, `owed`, `standing`, the digest's labels and the tab's `state` all read the shape and must keep working with no key. `cwd` is a box path and counts as configuration, not words | - |

### 1.2 The inbox (`inbox/`, pushed)

| Field | Class | Why / stage |
|---|---|---|
| `Asked.act` `Email(Draft)` `to`, `cc`, `subject`, `body`, attachment names | locked | other people's addresses and the owner's whole draft. The tap opens it on the box, so stored text = sent text still holds. Stage 5 |
| `Asked.act` `Changes` (todo bodies the change would write) | locked | words go to the chain only when tapped. A dismissed card must not keep them. Stage 5 |
| `Act::Secret.why`, `Patch.message` and `patch`, `Web.request`, `DriveRead.path`, `CodingSession.task`, `ClaudeSignIn.since`, `SignInCode.url` | locked | model words, document names, a one-time link. Stage 5 |
| `Decide(Draft)` title, body, rationale | locked while a card | they become the decision's own words when tapped (§1.4). Stage 5 |
| `Asked.why` | locked | the secretary's words, often about mail. Stage 5 |
| `NotSent`/`MayHaveGone`/`Dismissed.reason`, `Acted.what`, `Sent.recipients` | locked | owner's words, box sentences quoting headlines, addresses. Stage 5 |
| `Remember{text}` | shape | already a salted `TextName` (A, 2026-10-03) |
| `sources`, `serves`, `spec`, ids, instants | shape | leaves |

### 1.3 The other stores

| Store / field | Class | Why / stage |
|---|---|---|
| `memory/` | shape | already names only. Words in `secretary-memory-texts/` |
| `digests/` | shape | already names only. Words in `secretary-digest-texts/` |
| chains `events*/`: body, detail, rationale, `waiting_on`, notes, subtodos, `note` | leaves | the owner's record. The phone writes it signed, the registers resolve conflicts over it, the document is built from it, and triage replicas propose against it. Cancelling a commitment is a lifecycle act, not forgetting |
| chains `Source.sender`, `Source.subject` | leaves, flagged | other people's data, but `Source` is part of the frozen twelve-field `Record` kind. Moving it is a chain-wide migration of its own, and `hash`/`message_id` already let the box re-read the header. A decision for the owner (§10, Q3), outside these stages |
| `decisions/` title, body, rationale | leaves | the owner's recorded designs. They are superseded, never forgotten, and he taps each one |
| `state.json` `seen[].note` | box-local | the agent's line about a message. Stage 6 |
| `state.json` `noise_senders` | box-local | other people's addresses. Stage 6 |
| `state.json` `seen` keys (canonical hash), `native_id`, `last_thread_hash`, dispositions, `todo_id` | box-local | bookkeeping. Pushed today only so a restore does not re-triage. On restore, dedup falls back to the chains' `Source.hash`, and the vault has it. The whole file leaves the push. Stage 6 |
| `requests/*.json` `target` | path leaves, query box-local | a query string can hold a search term. Stage 6 |
| `briefings/` | box-local | the day's delta in words, mail summaries included. Each brief is a conversation already (locked by stage 2), so the file is a second copy. Stage 6 |
| `notes/`, `instance.toml`, `mbsyncrc` | leaves | the owner's own human-edited files (the persona prose is audit F5, a separate decision) |
| retired `secretary-memory/` (11), `secretary-inbox-proposals/` (207) | frozen, words in history | nothing writes or reads them. Only §7 can remove them |

## 2. The mechanism

### 2.1 The three options

**(A) Salted names, words box-local (memory and digest today).** The object
holds `TextName = SHA-256(salt ‖ words)`. The words live in a box-local file
under that name, are vaulted, and the sweep deletes them.

**(B) Per-text keys, crypto-shredding.** The object holds the ciphertext and
the key's name. Each text has its own random key, kept box-local and
vaulted. Forgetting appends an event and the sweep deletes the key.

**(C) Whole objects encrypted to one box key.**

| | A: salted words | B: locked words | C: one box key |
|---|---|---|---|
| Erasure | delete the file | delete the key | impossible per text: one key opens everything, and re-encrypting would rewrite objects |
| What the push carries | a name | a name and ciphertext (length visible, padded to buckets) | ciphertext of everything, the shape included |
| Box dies between vault runs | **the words written since the last vault are lost**. Weekly today, so up to a week of lines, model answers included, which nothing can rebuild | the keys written since the last vault are lost. Keys are 32 bytes, so the vault can run daily, and a tab that synced holds the keys it saw (§5.4): close to nothing lost | nothing lost if the one key is vaulted |
| Copies in backups | every vault snapshot holds the words | every vault snapshot holds the keys. Words also need the repository | one key in every snapshot |
| Phone replica | a new words channel, plus a purge | ciphertext rides the existing object replication. Keys are a small field on the same pull. Purge keys | the phone needs the box key, so a stolen phone reads all history forever |
| Verify, signatures | content-addressed. The words file checks against its name | content-addressed, and a signature covers the ciphertext. The key file checks against its name. AEAD checks the open | the Prodrome cannot read keys, parents or registers without the box key: fold and verify break for every replica |
| The data structure as a checkpoint | incomplete: the words exist only on the box and in the vault | complete: every object carries its content, and only access can be revoked | complete, but nothing can be erased |
| Prodrome change | none (an app schema) | none (an app schema) | breaks the Prodrome's own reading |
| New code | reuses `salted_text`, `texts`, `Naming`, the sweep | the same naming and sweep with a key in place of the words file, plus one AEAD dependency and the tab's open | small, but it fails the task |

**C is rejected.** It answers "GitHub must not read the words", not
"forgetting erases". It breaks the Prodrome's fold for every replica that
lacks the key. And forgetting under C means rewriting objects.

**Recommendation: B.** B and A have the same structure: a history that
names, a box-local thing the sweep deletes, and the same `Naming` lattice.
They differ only in where the bulk lives. B keeps the immutable,
content-addressed data structure whole, which is the owner's stated taste,
and moves the bulk off the box within five minutes of every write, through
the push that already runs. A leaves up to a week of words with no copy
outside the box. B reaches the tab through the replication it already uses.
B's erasure is as strong as A's. Against a snapshot thief it is stronger:
under A an old vault snapshot alone yields the words, under B it yields keys
that are useless without the repository too. B's honest costs:

- the ciphertext stays forever on GitHub and in every clone, so erasure
  rests entirely on destroying the key everywhere (§2.3);
- lengths leak, so pad to buckets;
- each object grows by about a third (base64 and padding);
- one new dependency.

### 2.2 The crypto, against what the code already uses

What exists today:

- names are SHA-256 (`sha2`, the same crate the Prodrome names objects with);
- signatures are Ed25519 (`ed25519-dalek` in `prodrome_core::sign`; WebCrypto
  Ed25519 on the tab's signer);
- randomness is `ring::rand::SystemRandom` (`io::texts::SystemSalts`);
- the vault is GPG to `VAULT_RECIPIENT`;
- there is **no AEAD and no KDF** anywhere.

B adds exactly one primitive and no KDF:

- **AEAD: ChaCha20-Poly1305 (RFC 8439)**, from RustCrypto's
  `chacha20poly1305`. It is pure Rust and `no_std`, it builds for `wasm32`,
  and it is constant-time in software (phones without AES instructions).
  It lives in `core`, so the box and the tab's module (`app/wasm`) open with
  the same function, as the closed typst world already does.
- **Not WebCrypto AES-GCM.** It would mean a second implementation in
  TypeScript, and reading belongs in the core (`feedback-typst-first`). The
  signer's precedent, a non-extractable key, matters for a signing key,
  which could forge writes forever. A decryption key on a page that shows
  the words anyway gains little from it.
- **Not `ring::aead`.** `ring` is in `io`, not in the pure `core`, and its
  `wasm32` build needs a C toolchain the nix wasm build does not have.
- **The key** is 32 bytes from the same `SystemRandom` as the salts, drawn
  through the capability (`capability::Salts` generalised to `Entropy`,
  which hands out a salt or a key). It is never derived. A derived key could
  be derived again from its master, which is not shredding.
- **The key's name** is `TextName = SHA-256(key)`, reusing the type. The key
  is random, so the name confirms nothing, and no salt is needed. The key
  file is the 32 bytes and nothing else. Its codec refuses bytes whose
  SHA-256 is not the name, the same discipline as `io::texts`: a swapped
  file reads as absent.
- **The nonce** is 12 zero bytes, because each key locks exactly one text,
  and the type enforces it: `TextKey` is neither `Clone` nor `Copy`, and
  `lock` consumes it.
- **The AAD** is a domain tag per store's words, `b"suzatary lines v1"` for
  the conversations and `b"suzatary cards v1"` for the inbox, so one store's
  text cannot be opened as another's.
- **Padding.** Before locking, the words are padded ISO/IEC 7816-4 style
  (`0x80`, then zeros) to a multiple of 256 bytes. The push then leaks
  length only to the bucket.
- **Encoding.** The ciphertext is base64 (the workspace's `base64`) inside the
  JSON field the line already spells its turn in.

### 2.3 A key's life

1. **Made at the door.** The one writer of a line (`transcript::push`)
   splits the entry into shape and words (§3.2), draws a key, locks the
   words, writes the key file, then appends the event. If the append is
   refused, it removes the key, exactly as `Held::remember` does. The locked
   event is built once, so a retry appends the same bytes, and the Prodrome
   writes nothing twice. A crash between the key write and the append leaves
   a key that nothing names. It opens nothing that exists, so it is
   harmless, and nothing deletes it.
2. **Kept** in `secretary-lines-keys/` (beside `[secretary] conversations`,
   like `-memory-texts`), mode 0700. It is in the data repository's
   `.gitignore`, refused by `safe-push.sh`'s pattern, and in
   `vault-backup.sh`'s tar list.
3. **Vaulted.** Today the vault is weekly and **never pruned**. That is
   already a finding: a forgotten fact's words live on in every earlier
   snapshot. Stage 0 makes it **daily, keeping the last `VAULT_KEEP`
   snapshots (7)**. It deletes older snapshots from Drive and empties
   Drive's trash, and checks the listing afterwards as it checks uploads.
   **Destroying a key therefore means**: gone from the box at once, from
   every tab at its next sync, and from backups when the last snapshot taken
   before the erasure is pruned, within `VAULT_KEEP` days. The memory and
   cards pages say so in one line. A snapshot that could restore a key can
   always restore it. Retention is the only remedy short of the stronger
   option, which is the owner's to take by hand: rotating `VAULT_RECIPIENT`
   and destroying the old private key, which shreds every snapshot under it
   at once.
4. **Restored.** `bootstrap-box.sh` restores the keys from the newest
   snapshot. Lines whose keys came later read as `Missing` (§3.1), never as
   an error, until a device gives the keys back (§5.4).
5. **Destroyed** by the sweep (§4.3), and by nothing else.

## 3. Types

### 3.1 New files in `core`

- **`secretary/locked_text.rs`**. `TextKey` (32 bytes, not `Clone`) and
  `Locked<T: Words>`, which holds `{ key: TextName, sealed: Vec<u8> }` and
  prints as `{"key": "<hex>", "box": "<base64>"}`. It exposes
  `Locked::lock(words: &T, key: TextKey) -> Locked<T>` and
  `Locked::open(&self, key: &TextKey) -> Option<T>`. `open` answers `None`
  for a wrong key or tampered bytes, and its words go through `T::read`, the
  `Words` door. A phantom domain per store gives the AAD. It reuses
  `salted_text::Words` and `TextName`. (The type is not named "Sealed": the
  Prodrome's envelope already uses that name, which
  `no_constructor_is_a_term_or_an_envelope` guards.)
- **`secretary/words.rs`**. One sum for every reading of words that may be
  gone, generalising `inbox_reading::Words` and `wire::RememberWords`:
  `Words<T> = Kept(T) | Erased(Erasure) | Missing`. `Missing` means no key
  here: the box lost it in a restore, or a tab has not been given it. The
  inbox's and the memory card's sums are folded into this one in the same
  series.
- **`secretary/erasure.rs`**. Why a line's words went:
  `Erasure = Asked { by: Actor } | Forgot { fact: FactId }`. A sum, so the
  page and the model's window say which, in the box's words.
- **`secretary/carried.rs`**. A line's provenance, in the head of every new
  event: `Carried = Written | From(Hash)`. `From` names the object of the
  old store that the migration carried. It is the "supersede by reference"
  the owner asked for, and the migration law pairs lines through it. It is
  never read for meaning.

### 3.2 The line: shape and words, one split

The conversations' schema moves to a new store, **`secretary-lines/`**
(`Genesis` labelled `lines`, `beside("-lines")`), at
`secretary::line_event::LineEvent`. Its head is
`conversation, at, actor, held, carried`, followed by one constructor per
kind, as today, with #111's `Kept` and `Scoped` and #119's `Unheard` among
them. Every constructor that has words holds `words: Locked<LineWords>`,
and the rest of its fields are shape.

`Entry` and `Turn` stay the MEANING types that every reader uses, so the 57
files that match on `Turn` change only where §3.3 adds an arm. The schema
file owns one pure pair:

```rust
/// What a line is without its words, and the words.
fn split(entry: &Entry) -> (Shape, Option<LineWords>);
/// The line as read: its words where kept, its shape where not.
fn join(shape: Shape, words: Words<LineWords>) -> Entry;
// law: join(split(e).0, Kept(split(e).1)) == e, for every entry (proptest)
```

`LineWords` is a sum per kind, and `Words` prints it as canonical JSON and
reads it back through the redaction door (`Scrubbed`):

- `Person { text, names }`
- `Model { text, args }` (one argument map per call, in order)
- `Tool { answer, names, texts }`
- `Act { text }`
- `Title { title }`
- `Why { why }`
- `Reason { reason }`
- `Coding { reason | report }`

The rule that every line goes through the door before it is an object
becomes: every text goes through the door before it is locked, and again
when it is opened. `from_value` can no longer scrub ciphertext, so the
scrub moves into `LineWords::read`.

### 3.3 Reading erased words

- `Turn::Erased { shape: TurnShape, erasure: Erasure }`, where `TurnShape`
  is who spoke, the verb names, the call verbs, `refused`, the references
  and the file ids. Taint, `state` ("a model turn with no calls"), `owed`
  and the digest's labels read the shape, so they read an erased turn as
  they read it whole. An erased turn keeps its index, so `Origin::Turn` and
  `proposal:<conversation>/<n>` still resolve.
- `Stored.title: Words<Title>`, `Review.why: Words<String>` and
  `ReviewFailure.reason: Words<String>`.
- When a model is sent an erased or `Missing` turn, the box composes one
  sentence for it ("the owner erased this turn's words", "this turn's words
  are lost"), and never a stored sentinel.

### 3.4 New constructor: `Erased`

`Erased { lines: BTreeSet<Hash>, erasure: Erasure }` names lines of its own
conversation by object name. It is not a `Said`, so turn indices do not
move. The store's door accepts it only from the box (`assistant`), because
the box writes it on the owner's gated act or on his forget. No verb
reaches it, so the secretary cannot erase the evidence of an injection.

### 3.5 Capabilities and io

- `capability::Entropy` replaces `Salts` (salts and keys from one source).
- `store::Doc<TextName, TextKey>` is the key store. Its interpreter is
  `io/src/text_keys.rs`, built on `Docs` with the digest codec, sitting
  beside `io/src/texts.rs`.
- `secretary::read(lines, keys, id)` takes the key store. The box keeps an
  in-process memo of opened words by `TextName`, sound because the words
  are immutable, and the sweep drops its entries.

## 4. Erasure: what forgetting erases

### 4.1 The owner's own act

"Erase" on a line, or on a whole conversation (every line it holds), is a
gated act through the same door as a `Tap`: an owner on the
`digest_policy::Owners` allowlist, a signed request. The box appends
`Erased { erasure: Asked { by } }` and sweeps.

### 4.2 A forget

A forget appends the memory's `Forgotten` as today. Then, under the
memory's hold, it appends to the fact's conversation an `Erased { erasure:
Forgot { fact } }` naming three things:

1. **By causation** (a fact of the history, not a search): the origin
   round's lines. `Origin::Turn` names the owner's opening turn. The round
   runs from that turn to the next person turn. The erasure takes the
   opening turn, the model turn whose call was the `remember`, and that
   call's answer. For `Origin::Tapped`, it takes the card's chip turns
   instead.
2. **By verbatim copy, in every conversation**: the box opens every
   standing `Tool` answer and erases each one that contains the fact's
   words exactly. That covers `recall`, `zoom`, the `memory` verb, receipts
   and card chips: machine copies, which quote verbatim. Forgets are rare,
   and one pass costs one open of every answer.
3. **Not found, and said so**: the model's prose or the owner's other turns
   that paraphrase the fact. The page says that forgetting erased N lines
   and that a line restating it in other words can be erased by hand. This
   is the prior report's point, and it is honest. A paraphrase cannot be
   found as a fact of the history.

If the conversation append fails after `Forgotten` stood, the forget
stands, and the next sweep appends the owed erasures. "Owed" is derived
over the object set: a forgotten fact whose origin lines no `Forgot`
erasure names.

### 4.3 The sweep

The lines' key naming is `Naming<LineWords>`. Each line's key is `Live`
unless an `Erased` names the line, in which case it is `Past`. Lines and
erasures only grow, and keys are never shared, so **dead only grows and
the lines' sweep needs no hold**. It runs after every erasure, every
forget, and on the triage timer. (The memory's sweep keeps its hold,
because a card can come to name an existing text.) A sweep that cannot
read the store whole deletes nothing, as `memories::whole` does.

### 4.4 The digest

An erased line leaves the digest's live lines. The nodes above it rename,
their summaries become past in the digest's `Naming<Line>`, and the
existing sweep deletes those summaries' words. The compactor
re-summarises without the line. That makes the digest's own `Erased`
(exclusion, which no page offers) redundant. Stage 4 deletes it, provided
the migration's check reports that none exists live. Otherwise it stays,
and the reading resolves the lines it names through `Carried::From`.

## 5. The tab

### 5.1 The pull carries keys

`/api/stores/<store>/objects?since=` gains `keys: { <name>: <hex> }`, the
keys of the texts named by the objects in that answer that stand
unerased. The gate is the same as the objects' (a viewer login gets what it
may read). A tab that holds objects but lacks a key asks
`/api/stores/lines/keys?names=`, answered per name with
`kept { key } | gone {}`.

### 5.2 One row, objects and keys together

The IndexedDB row for the lines store is `{ objects, keys }`. Its join is
`objects ∪`, with `keys = (keysA ∪ keysB) \ dead(objectsA ∪ objectsB)`.
`dead` is the core's reading in wasm, the same function the box sweeps
with. Because objects only grow and hold every `Erased` once any tab
synced it, a tab that wrote a key before it heard the erasure cannot bring
that key back: every read and every write of the row filters it out.

### 5.3 Purge

- The keys of every dead name go from the row.
- The rendered-block cache (`app/rendered.ts`) is re-keyed to carry the
  `TextName` each block came from, and loses the blocks of dead names.
  *(As built: the purge finds a block by the key its words name under the
  running build (`Conversations.erased`), and the device keeps blocks of that
  build's functions alone, so no block of another build is left for a purge
  to miss; 2026-10-12.)*
- The objects stay. They are ciphertext.

### 5.4 Key return, B's restore aid (stage 3b, optional)

*Decided for stage 2 (amendment 1): no key return in stage 2. The keys'
durability is the daily vault (#131), which carries `secretary-lines-keys/`
beside the memory's and the digest's texts; losing the box and the vault
together loses the words, and that is accepted. B is chosen for the owner's
taste and because a vault thief also needs the repository; 3b stays
optional.*

After a restore, the box answers `Missing` for keys newer than its vault.
A tab that holds such a key posts it back. The box accepts it only if
SHA-256(key) is a name some standing, unerased line names, and the key
opens that line. A forged key therefore cannot be planted.

### 5.5 Upgrade, the existing plaintext replica

The tab's current `store/conversations` row holds plaintext objects, and
"only climbs". The new build reads `store/lines` and **deletes the old row
and clears the rendered cache** in `app/upgrade.ts`. That is deleting a
device cache, not a pushed object.

### 5.6 A revoked or lost phone

Revocation stops the box giving that key more keys (reads must start
checking revocation, audit 6). The keys it already holds stay readable on
it, exactly as its plaintext does today. A remote wipe is audit 6's, not
this plan's.

## 6. The forward migration (append-only)

### 6.1 The verb

`suzatary secretary carry-lines [--check]` holds the old store's lock and
the new store's. For each conversation, in the old reading's order
(`transcript` causal order, ties by instant then name), it appends one
`LineEvent` per old object:

- the same `conversation`, `at`, `actor` and `held`;
- `carried: From(<old object name>)`;
- the words locked under fresh keys.

Appended one after another, the new lines form a single causal chain per
conversation, so no tie remains and the order is the old reading's exactly.
The cost is that concurrency between two replicas' lines is not carried.
For one writer nothing changes, and none of the readings asks. The verb
keeps every summary by reading a carried line as the leaf of the line it
carries (`Carried::From`), so the digest of the carried store is the same
tree and nothing is re-appended or re-summarised. *(As built: the tree is
chunked by node NAMES, `memo::balance`, so new names would have given a new
shape and no one-to-one mapping; the leaf identity is what makes it one.)* It
refuses if the old store moved after its read, as #32 and #34 did.

### 6.2 The law (`--check`, and the property test in §8)

Every reading is the same before and after, except that the words come
from the key store:

- for every conversation, `Stored::of(old)` equals `secretary::read(new,
  keys)`;
- the `ConversationPayload` JSON is byte-equal, modulo object names, which
  are mapped through `From`;
- `owed`, `standing`, `state`, taint per round, the list, search results,
  the digest's tree shape and its standing summaries by position are all
  equal;
- `From` is a bijection onto the old objects;
- the old store's files are byte-identical after the run.

### 6.3 Order of running

*As built (stage 2's report, replacing the order first written here, which
restarted the web onto an empty store before the carry):*

1. Land the rev (`train.sh land`) and let it publish: the rev that goes live
   is the suzatary-core commit whose `Source:` trailer names the landed
   commit (`git fetch core`, then `git log core/master --grep "Source:
   <landed sha>"`). Its tree's store path is that commit fetched by its sha,
   the same path `deploy.sh` reads from the lock it pins (`source.outPath`):
   `nix eval --raw --impure --expr '(builtins.fetchTree { type = "git"; url
   = "https://github.com/bmabsout/suzatary-core"; ref = "master"; rev =
   "<core sha>"; }).outPath'`.
2. Build that tree without moving anything: `scripts/app-builds.sh <store
   path> ~/.local/state/suzatary/roots/deploy-`, the prefix `deploy.sh`
   builds under, so its own build finds them. The operator's shell does not
   set `$SUZATARY_ROOTS`; the units' value is in `systemctl --user show -p
   Environment --value suzatary-web.service`.
3. On a scratch copy (`SUZATARY_INSTANCE` at a copy of the data tree, never
   the live one), from that build: `SUZATARY_REPO=<store path>
   <prefix>bin/bin/suzatary --instance <scratch instance.toml> secretary
   carry-lines`, then the same with `--check`. Both must print no finding.
4. The owner's yes: this is a live data migration.
5. Commit to the data repository's `.gitignore`, before the carry:
   `/secretary-lines-keys/`, `/secretary-lines/.lock` and
   `/secretary-lines/lines.lock`. The live file names each lock file (it is
   not the template's `*.lock`), `safe-push.sh` stages `secretary-lines/`
   whole and its pattern refuses a lock file, so without these every push
   after the carry commits nothing.
6. Stop everything that writes a conversation: `systemctl --user stop
   suzatary-web suzatary-mail-idle suzatary-triage.timer
   suzatary-plugins.timer suzatary-vault.timer`, then wait until
   `systemctl --user is-active suzatary-triage.service` answers `inactive`
   or `failed` (`mail-idle` starts triage directly, and a review it runs
   writes into `secretary-conversations/`). There is no review unit of its
   own.
7. The carry, from that build, against the live instance:
   `SUZATARY_REPO=<store path> <prefix>bin/bin/suzatary --instance <live
   instance.toml> secretary carry-lines`, then the same with `--check`. The
   check proves the digest is the old tree, and counts the kept summaries
   that were already dead; the first sweep deletes them.
8. `scripts/deploy.sh <rev>`: it builds (finding step 2's builds), verifies
   with the new binary, moves the lock and restarts `suzatary-web` onto the
   carried store. If it fails before the web restarts, it reverts the lock to
   the old app: do NOT start the old units until BOTH `secretary-lines/` and
   `secretary-lines-keys/` are removed, since the old app writes into
   `secretary-conversations/` and a later carry would leave those lines out.
9. Push (`safe-push.sh` from this build admits `secretary-lines/`).
10. Start `suzatary-mail-idle` and the timers again (`systemctl --user start
    suzatary-mail-idle suzatary-triage.timer suzatary-plugins.timer
    suzatary-vault.timer`), and run the vault once right away (`systemctl
    --user start suzatary-vault.service`): until it has run, the keys exist
    only on the box. Force-stop the APK so the tab upgrades.

If a carry dies part way, or its check finds anything, stop: remove BOTH
`secretary-lines/` and `secretary-lines-keys/` and run step 7 again. The
plain store is only read, so nothing else needs undoing.

Once the web has restarted on the lines, only fix forward: never deploy a
rev from before the lines. Its app writes `secretary-conversations/`, which
nothing carries again, and the first startup's erasures have already
destroyed keys. `deploy.sh`'s hint to go back says the same of any rev that
migrated a store.

The first startup owes every erasure a forget would have made:
`erase_forgotten` runs over every fact ever forgotten, erases each one's
`remember` call and answer in the carried lines, and files one `Erase` card
per forgotten fact with turns left to offer; then the lines' sweep and the
memory's sweep run (the already-dead summaries go there), each whatever the
other did.

### 6.4 Then the old reader is deleted

*As built (stage 4, 2026-10-12, the owner: "if we're migrating to a new
format, we move everything correctly and delete the old stuff"): the schema,
the verb, its law and the plain fixture are deleted, and so is the digest's
`Erased`, which no route, verb or tool had ever written. `safe-push.sh`
neither admits nor walks `secretary-conversations/`, and the data repository
removes the store in one plain commit (`git rm -r`, under the push lock, a new
commit and not a rewrite), so its earlier commits keep it beside the code that
read it. The paragraph below is the plan as first written. `Carried` stays: a
carried line keeps the old object's name as its digest leaf.*

Stage 4 deletes the plaintext schema (kept under the verb's module until
then), the verb, and the digest `Erased` (§4.4). `safe-push.sh` stops
admitting new files under `secretary-conversations/`, but its append-only
walk keeps covering that path, so the frozen store cannot be deleted
either. The data repository's earlier commit keeps the old store beside
the code that read it, as the no-compat rule wants.

### 6.5 Why a new store and not supersession in place

Appending "resealed" lines into `secretary-conversations/` would make the
new reader parse the old plaintext schema forever, which the no-compat rule
forbids. It would also leave the words in a path the new code still owns,
so the optional rewrite (§7) could not be a path-drop. The inbox (stage 5)
follows the same rule: a new store (`secretary-cards/`) that keeps each
card's `ProposalId`, so `Proposed.object` still resolves. For cards asked
after the cutover, the id is the hash of the locked act. That is stable,
and confirms nothing. The rule that the same email asked twice is one card
becomes the box opening its standing cards under the inbox's hold, as
`Held::keep_card` does today.

## 7. What stays readable without a rewrite, and the rewrite as an option

### 7.1 Stays readable in `bmabsout/suzatary-data`

These stay in its history, on GitHub and in every clone (the box's
checkout, and any cloud session that cloned it since 2026-10-02) unless the
owner rewrites. Items 1 to 3 stay at the tip too, because they are frozen
in place.

1. `secretary-conversations/objects/`: every line from the 2026-10-01
   carry-over up to the cutover. That means:
   - person turns;
   - model text;
   - call arguments, every `remember`'s fact among them;
   - tool answers: `show` and `search` mail bodies, `drive_read` and
     attachment text layers, the `memory` verb's snapshots of the main
     session's memory directory, `zoom` answers once #111 is live;
   - the daily sweep's mail bodies;
   - titles, review whys and refusals.
2. `secretary-inbox-proposals/objects/` (207): retired cards, their acts'
   words included.
3. `secretary-memory/objects/` (11): retired facts with their words.
4. `inbox/objects/` up to stage 5: email drafts, whys and reasons.
5. Every committed version of `state.json` (notes, `noise_senders`,
   Message-IDs), of `briefings/`, and of `requests/` targets.
6. Off the repository: Drive vault snapshots, never pruned until stage 0,
   which hold memory words as they stood; the tab's IndexedDB rows until
   the stage 2 upgrade; and the provider's session transcripts
   (`~/.claude/projects`), which are not the box's to erase.

### 7.2 The rewrite, if the owner ever wants it

Because every word-holding store is migrated to a new path, the rewrite is
a path-drop that changes no surviving byte:

```
git filter-repo --invert-paths \
  --path secretary-conversations/ --path secretary-inbox-proposals/ \
  --path secretary-memory/ --path inbox/ --path state.json --path briefings/
```

`inbox/` is included only after stage 5. Requests' queries would need a
blob callback, and are not worth it.

What changes and what does not:

- **Object names**: unchanged. An object's name is the SHA-256 of its
  bytes, not of a commit, and no surviving blob is touched. The chains'
  signatures (over object names), `app/conformance/literals.json`,
  `<data>/conformance/` (chain folds) and the phone's chain replicas are
  all unaffected.
- **Rewriting blobs instead** of dropping paths (for example, scrubbing
  words out of `inbox/` in place) would rename every touched object and
  break `Proposed.object`, signatures and replicas. Do not.
- **Commit SHAs change.** The data repository's own commits are renamed,
  "Deploy <rev>" lock commits included, though the lock's content and the
  app revisions it pins do not move. Any note naming a data commit SHA
  dangles. Every clone must be re-cloned, and `safe-push.sh` expects
  linear history.
- **Old checkpoints lose their conversations.** An earlier checkpoint
  would run code that reads `secretary-conversations/` and find it empty.
  That conflicts with "we can always pull out an earlier checkpoint", and
  it is the cost the owner weighs.
- **Force-push is not erasure on GitHub.** Unreachable objects stay
  cached, and are reachable by SHA, until GitHub runs gc. The sure purge
  is deleting and recreating the private repository (or a GitHub Support
  purge request), then pushing the rewritten history.

**The owner confirms:**

- the exact path list;
- that no pre-rewrite bundle is kept (one kept defeats it);
- the delete-and-recreate on GitHub;
- that the box, codespaces and cloud sessions re-clone;
- the instant, with serve and the timers stopped.

Separately, check the code repository's history for data paths from
before 2026-09-26. The conversations were box-local JSON lines then, so
the words should not be there, but `state.json` may be.

## 8. Laws and tests

1. **`app/core/tests/line_words.rs`**, a proptest that generalises
   `card_words.rs`. Random scripts of rounds with sentinel words in every
   field of §1.1 (person, model text and arguments, answers, file names
   and texts, acts, titles, review whys and refusals, coding reasons),
   interleaved with owner erasures, remembers, forgets, card asks, taps
   and dismissals. After every step:
   1. no printed object of ANY wordless store (`lines`, `memory`,
      `digests`, and `cards` from stage 5) contains a sentinel;
   2. the key store holds exactly the keys of lines named and unerased;
   3. every unerased line opens to its words, and an erased line reads
      `Turn::Erased` with its shape intact;
   4. dead only grows;
   5. `secretary::read` equals a plaintext reference model, `Stored::of`
      over the plain entries with erased turns replaced by their shape;
   6. after a forget, the origin round's three lines and every answer
      quoting the fact verbatim are erased.

   `card_words.rs`'s own law (no object holds any words) is kept, folded
   into this file's harness.
2. **`split`/`join`**: `join(split(e)) == e` for every `Entry` (proptest
   over the entry generator). `Words::read(print(w)) == Some(w)` for
   `LineWords`.
3. **The AEAD vector**: a fixed key and fixed words give pinned bytes. It
   is FROZEN, like the conformance vectors, and run by the core and by
   `wasm.test.ts`, so box and tab open alike. A tampered byte, a wrong key
   and a wrong domain each open to `None`. A key file whose SHA-256 is not
   its name reads as absent.
4. **`app/cli/tests/carry_lines.rs`**: the §6.2 law over generated old
   stores (with Kept, Scoped and Unheard lines, rewinds, stops, reviews and
   coding lines), and, on the box, over the scratch copy (`--check`).
5. **`web/test/linekeys.test.ts`**: after a step holding an `Erased` for t,
   the row has no key for t and the rendered cache no block from t. A
   stale tab's write cannot resurrect t. After upgrade, no
   `store/conversations` row exists.
6. **The guard** (`app/cli/tests/guards.rs`,
   `every_store_safe_push_admits_is_classified`). It parses `safe-push.sh`'s
   `ALLOWED_PATTERN`, `ADD_PATHS` and `DATA_EXTRA_PATTERN` and asserts that
   the admitted set is exactly the union of three named tables in the test:
   - `WORDLESS`: each entry names its law test in (1);
   - `BY_DESIGN`: chains, `decisions/`, `notes/`, `instance.toml`,
     `mbsyncrc`, each with its one-line reason from §1;
   - `PENDING`: `inbox/` until stage 5; `state.json`, `requests/` and
     `briefings/` until stage 6. This table must be empty when stage 6
     lands.

   It also asserts that every key and text directory
   (`-lines-keys`, `-memory-texts`, `-digest-texts`, `-cards-keys`) is
   refused by the pattern and ignored by the data `.gitignore`, and that
   the frozen word-holding paths (`secretary-conversations/`,
   `secretary-inbox-proposals/`, `secretary-memory/`) admit no new file
   but stay under the append-only walk. A new store cannot be pushed
   without being classified.
7. **Vault** (`scripts/` test, beside `deploy-guard`). After a run, the
   remote listing holds at most `VAULT_KEEP` snapshots, and the tar list
   holds every keys directory the instance declares.

## 9. Stages

Each stage is one PR, landed as a series, one logical change per commit,
each commit buildable. #111 and #119 land first. Every stage that touches
the conversations branches from a master holding both.

| # | PR | Series | Order |
|---|---|---|---|
| 0 | **The vault keeps a week** | daily timer; `VAULT_KEEP` pruning and trash; keys dirs in the tar list (tolerant `ls -d`, as now); `secretary-digest-texts` added or explicitly left out with a reason (it is missing today); bootstrap note; test (§8.7) | independent: any time, even before #111 |
| 1 | **Locked text** | `chacha20poly1305` and `base64` into `core`; `locked_text.rs`; `words.rs` (folding `inbox_reading::Words` in); `erasure.rs`; `Entropy` replacing `Salts`; `io/text_keys.rs`; the vector (§8.3); docs | new files only, so it can be built alongside #111 and #119; lands after them |
| 2 | **The conversations lock their words** | `line_event.rs` with `carried.rs` and split/join (§8.2); the door in `transcript::push`; readers through keys and `Turn::Erased`; `Erased` and the gated route; the lines' sweep; forget's erasures (§4.2); the digest reads erased lines out; the pull carries keys; the tab's row, purge and upgrade; `carry-lines` with `--check`; `line_words.rs` and `carry_lines.rs`; the guard with its PENDING table; `safe-push.sh` admits `secretary-lines/`; the data `.gitignore`; CLAUDE.md, ARCHITECTURE §10.3 and AGENT.md (the false ".gitignore says box-local" note goes) | after #111 and #119, and after 1. The box and the tab must change together, so it is one PR |
| 3 | **Run it** (no PR) | scratch `--check`; owner's yes; deploy; live carry and `--check`; push; force-stop the APK so the tab upgrades | after 2 |
| 3b | *optional* **Devices give keys back** | the route (§5.4), test | any time after 3 |
| 4 | **Delete the plain conversations** | the old schema and the verb; the digest's `Erased` and its policy arm, if none exists live; `safe-push.sh` stops admitting the frozen path; docs. Net negative lines | after 3 |
| 5 | **The inbox locks its words** | `secretary-cards/` with locked acts, whys and reasons; dedup by opening under the inbox's hold; tap opens to send; dismissal leaves the words, an owner erase removes them; `carry-cards` with `--check` keeping `ProposalId`s; the tab's `store/inbox` row renamed and purged; the guard's PENDING loses `inbox/` | after 4 |
| 5b | **Delete the plain inbox** | old schema and verb | after 5 runs |
| 6 | **Bookkeeping stays home** | `state.json` box-local and vaulted (one commit removes it from the tree, since it is a rewritten file, not an object store); `requests/` record path only; `briefings/` no longer staged (files already there stay); the guard's PENDING empties | after 2; independent of 5 |
| 7 | *later, the owner's choice* **One mechanism** | the memory's and the digest's words move from salted texts to locked ones (new history stores carried by reference, as in §6), then `salted_text.rs` and `io/texts.rs` are deleted | after 5b, if he wants the durability for facts |
| 8 | *optional, the owner's choice* **The rewrite** | §7.2 | any time after 5b and 6 |

## 10. Lenses and open questions

- **Kleppmann (crypto-shredding).** After an erasure, what still holds the
  words? The tabs until they sync, the vault until `VAULT_KEEP`, the
  provider's transcripts, the filesystem's freed blocks, and paraphrases.
  All of them are named on the page or in §7.1, and none is hidden.
- **Hellerstein (CALM).** Which questions are non-monotone? "Does a card
  already keep these words" (under the inbox's hold) and the memory's
  four. The lines' sweep is monotone and takes no hold.
- **SPJ and Kmett.** Is erasure a type? Yes: `Words<T>`, `Turn::Erased`
  and `Erasure` are sums, and split/join is a lawful pair, with no
  sentinel string anywhere.
- **git maintainers.** Is the rewrite a path filter that renames no
  object? Yes, because every migration moves words to a new path.

Open questions:

1. **`VAULT_KEEP`.** Seven daily snapshots, so erasure completes within a
   week and at most a day is lost? Or rotate the vault key now and then?
2. **Whole rounds or single lines.** A forget erases the owner's whole
   opening turn and the model turn that remembered the fact. Is that over-
   or under-erasure for him?
3. **`Source.sender` and `Source.subject`.** Keep them pushed as part of
   the record, or open a chain-kind migration of their own?
4. **The `memory` verb.** Its answer is the main session's whole memory
   directory, which also goes to the provider on every triage pass. Locking
   it closes the push, but should the verb exist at all? (Audit 8.)
5. **Stage 7.** Is moving facts to locked words worth the durability, or
   should salted texts stay for the memory alone?

---

## 11. Amendments from the independent review (Fable 5.1, 2026-10-09) — BINDING

1. Durability: B's off-box advantage holds only if the tabs return keys (3b). Make 3b part of stage 2, or drop the durability argument. B is chosen for the owner's taste (a complete immutable structure; words ride the existing replication) and because a vault-snapshot thief also needs the repo.
2. Erasure copies the plan missed:
   - the box's render cache (io/src/rendered.rs, `secretary-render/`, keyed by source bytes): erasing a line computes its blocks' render keys BEFORE destroying the key, and deletes those files;
   - name the push notifications (fixed separately by privacy package A), briefings/ until stage 6, and the in-process word memo;
   - say on the page what an Erased line itself reveals (which lines, when, which mail references quoted the fact).
3. Crypto:
   - split the key types: `FreshKey` (from Entropy, consumed by `lock`, never stored) and `TextKey` (read from the store, can only `open`), so a zero nonce is safe by type;
   - the AAD is the store tag plus the canonical print of the line's Shape (conversation, kind, held, carried, every shape field), so a ciphertext moved to another slot does not open;
   - `KeyName` is a newtype distinct from TextName;
   - visible metadata is stated in §1.1: lengths in 256-byte buckets, counts, `at`, verb names, model names, and the confirmable `mail:`/`thread:`/attachment hashes. HMAC-ing those is a later decision.
4. Digest after an erasure: keep the erased leaf in `live_order` with its Label marked erased, never dropped, so only its ancestors rename (log n) and the rest of the tree's summaries survive.
5. Grain of a forget: lock a Model turn's `text` and EACH call's `args` under separate keys. A forget erases by causation the provable copies (the remember call's fact and its answer). The opening turn and the prose are OFFERED with a one-tap erase, never silently over-erased.
6. Migration:
   - `--check` asserts that the digest's naming is Live for every reused TextName;
   - build the new chain in memory and accept it ONCE, then sign; never 4,636 sequential appends (quadratic);
   - run the carry from the deploy's built binary BEFORE the web restarts, with suzatary-web and the timers stopped, so no window reads an empty store.
7. Stages:
   - stage 0 also adds secretary-digest-texts to the vault's tar list now;
   - split stage 2 into a box PR and a tab PR landed in ONE train;
   - DROP stage 7: memory and digest keep salted texts.
Answers: Q1 VAULT_KEEP=7 daily, prune and empty the trash, test the listing, and write a VAULT_RECIPIENT rotation recipe. Q2 per-call keys, as in 5. Q3 leave Source.sender and subject for one later decision together with `references`. Q4 and Q5: see the owner's answers.

## 12. The owner's answers (2026-10-09)
- Forget grain: provable copies are erased automatically; the opening turn and the surrounding prose are offered with a one-tap erase (amendment 5).
- The `memory` verb is DELETED (the secretary keeps only its own approved memory; it no longer reads the main session's notes). Triage gets a curated rules file (privacy package A).
