# Suzatary 2.0: the whole project prodrome, in waves

The owner, 2026-10-03: "continue tackling everything needed to do 2.0 and I
want you to basically address everything we have in the project prodrome.
Don't wait for me to answer anything, use your best judgement", and
"remember to think like the Pijul maintainers, like edward kmett, like Conal
Eliott and to make sure our final version of the codebase is beautiful around
these frp and crdt concepts".

2.0 is the `suzatarytwo` item: anyone can run their own secretary, self-hosted
beautifully, on a small lawful Prodrome. Its shape is the Prodrome's
`docs/design-register-types.md`: the history is the free join-semilattice
(§1), a type supplies an order (§3), the application is a reading of history
∪ intents ∪ ui at a time (§6.0), replicas sync by union (§6.1), caches are
tabulations memoised over the data's own tree (§6.2), and nested histories
flatten by a monad's join (§6.3). Every item below is judged by one question:
does it make the code more that structure, or does it add a special case
beside it?

## How it runs

- Every item is a pull request from a cloud session (Prodrome items gate in
  that repository's CI; Suzatary items queue for the box's merge gate, run
  once per TRAIN of reviewed PRs rather than once per PR).
- Every brief carries the lenses Kmett, Conal Elliott and the Pijul
  maintainers, plus the Rust and git maintainers and the task's own.
- Live data migrations are run on a scratch copy first and then live, without
  asking, as the owner said; sending mail and making a repository public are
  still never done without him.
- Judgement calls are recorded in §4 so the owner can read them in one place.

## 1. The items, by wave

Status: `done`, `train` (merged on the integration branch), `session` (a
cloud session is working), `next` (waits on the wave before), `me` (the main
session does it), `held` (needs the owner).

### Already landed on the way (closed on the chain 2026-10-03)

changeidentity, stagingoverlay, conversationprodrome, verifyflakyparallel,
ciminutesdead; instancesplit cancelled as superseded by userrepository.

### Wave 0, in flight

| Item | What | Status |
|---|---|---|
| secretarycalendar | #48, the ICS plugin and its readers | done (1ae0c9b2) |
| (app-as-reading 1) | #47, every store a replica in the tab | done (1ae0c9b2) |
| (markup door) | #46, one door for every writer, the editor | done (1ae0c9b2) |
| (first visit) | #45, typst-wasm precompressed and streamed | done (1ae0c9b2) |
| (forgotten words 3) | the inbox card's words erasable | done (#51, train 4) |
| userrepository 1 | the data repository pins the app | done (data repository, 2026-10-03) |

### Wave 1, independent of the train

| Item | Repo | What |
|---|---|---|
| durablestore, nodeofmixedstore | Prodrome | fsync before rename, random temp names, fsck quarantines; the DAG the only way to nodes; done (Prodrome #35) |
| scheduletype | Prodrome | `Schedule<A>`, the step function over time, as the behaviour type `observe::next_change` and every reimplementation share; one fold where there are two; done (Prodrome #34) |
| quietpasscommits | Suzatary | a quiet triage pass commits what the server wrote; train (#49) |
| (a mail sync failure), allowancescope | box | the sync failure done (provider-side, transient); allowancescope: say the refusal (wave 2, with accounthealth) |

### Wave 2, on the train's master

| Item | Repo | What |
|---|---|---|
| prodromemain | Suzatary | the agent's batches as overlays accepted by `store::accept` (a restricted join); delete the legacy writer, the eventless join and every one-head reader; derived tips; `Absent` for spec `None` |
| (app-as-reading 2) | Suzatary | every act an intent, one set; a card's state derived, never kept |
| typedverbs, secretarysyncnow | Suzatary | the secretary's verbs as a typed `Verb → Answer` with the schema derived; a sync-now verb |
| wireonce | both | the wire declared once, decoders generated, a serde feature for terms |
| decisionitems | Suzatary | a decision schema: plans as items with a supersedes link |
| receiptsignature | Suzatary | the TDX quote chain; a report cache keyed by keyset digest; train (#50) |
| prodromesets | Prodrome then Suzatary | a set of prodromes is a prodrome: identity by genesis, disjointness, a qualified `Ref`; Prodrome done (#37), Suzatary half PR #69 in review |
| (plugins 3) | Prodrome then Suzatary | declarative schemas, admitted by law checks; done (Prodrome #36, Suzatary #56) |
| userrepository 2 | both | units run the app from the lock; deploy is a lock commit; PR #68 in review |

### Wave 3

| Item | What |
|---|---|
| (app-as-reading 3, 4, 5, 6) | ui as registers; pages as Typst views over the generic memoised fold (fixes #43's slower draw); the clock observed with `next_change`; the box's work as rules over registers |
| proposalactliteral | a proposal's act as a literal vocabulary (after the card-words migration) |
| devicesignatures | device-key signatures on objects |
| repoattachpr | attach a file from the owner's repositories; propose a patch as a PR |
| (plugins 4, 5), portsuites | the actor world through the tap; plugins from the user repository; Drive, mail and the model as components with conformance suites; session (ports as traits with laws and suites; the actor world through the tap, `actorinbox`, done; bundles from the user repository, pinned by sha256; the routine's half waits) |
| userrepository 3 to 5, onboarding, tsnetportal | the portal joins the tailnet itself by embedding tsnet (no separate VPN app; spike first: APK size, WebView through its proxy); the app as a library and a host crate; a template a new user starts from; `suzatary init`; docs rendered by Typst with a demo instance |
| teststrust, syncdivergedflake, unpricedfixture | one law one test, consolidated test binaries, fixtures from real code |
| triagebench | commits 2 and 3: the stratified scorer |

### Last

| Item | What |
|---|---|
| cleanuppasses | a deletion pass over the whole tree, judged by the size line |
| cutonepointzero | tag v1.0.0 at the train's master as the soaked baseline, then v2.0.0 at the end |
| suzatarytwo | closed when every row above is done |
| finalizepublic | held: only when the owner says so |

## 2. Order and overlap

Sessions run in parallel when their files do not overlap; each brief names
the files other sessions own. Suzatary PRs queue into trains; a red train is
bisected by its merges.

## 3. What 2.0 deletes

The legacy writer and the eventless join (prodromemain), every per-page load
and pending flag (app-as-reading 2 and 4), the ticking clock (5), the four
box workers as separate mechanisms (6), hand-written wire mirrors (wireonce),
the live clone and `promote.sh` (userrepository 2), JSON inside a string
field (proposalactliteral).

## 4. Judgement calls

- 2026-10-03: `instancesplit` cancelled; `userrepository` is the same idea with
  the dependency the right way round (private pins public).
- 2026-10-03: v1.0.0 is tagged at the soaked system before 2.0's breaking
  changes, rather than skipped, so the checkpoint the soak earned has a name.
- 2026-10-03: the calendar's links are declared under one name per calendar
  the owner keeps (the names live in the data repository's instance file;
  a Google Workspace calendar's link comes from calendar.google.com, an
  allowed origin). A name with no link set fetches nothing and says so on
  the Settings page; only the owner pastes links.
- 2026-10-03: the agent's context fits its 24,576-byte cap by 94 bytes after
  the first train. A new wave-2 item, `contextbudget`: the context carries
  less inline (calendar titles and notes already go through verbs) so a
  manual can grow without a silent cut.
- 2026-10-03: `accounthealth` (from the mail sync failure item): each mail
  account's last sync result is a register the app reads, and a failure that
  persists for a day proposes a todo, instead of the agent repeating it in
  prose every pass. With it, `allowancescope` only says the refusal: the
  usage endpoint needs a token with `user:profile`, which the box's
  setup-token cannot have, and handing the server the claude.ai login's token
  would give it far more than a number to show.
- 2026-10-03: `tsnetportal` (owner: yes): the Android portal embeds Tailscale's
  tsnet so a phone needs no separate VPN app and a second user installs one
  app; the box is unchanged (whois maps the app's node to its login). Spike
  first; signing an APK on the box still asks.
- 2026-10-03: NEVER a public endpoint on the box (owner: "this box has all my
  personal info"); caches and runners live off it. The merge gate's heavy half
  moves to a Codespace on Suzatary; only the live pins run on the box.
- 2026-10-03: Suzatary's quick CI no longer runs on pull requests (928 of 3,000
  private minutes in three days); sessions and trains gate instead.
- 2026-10-03: the account-health register is a box-local document holding the
  JOIN of every sync's results (latest success, latest failure, first result:
  three semilattices), not a log of them: the reading is a function of the set
  of results, so storing the join is storing the set as far as anything reads
  it, at constant size. The proposal goes on the project's chain, as the brief
  said, though CLAUDE.md's routing example files a stopped sync under `life`.
  The agent's `last_sync_status` verb is removed rather than reworded, so not
  mentioning a sync failure is what the agent can do, not what it must
  remember. The secretary's `context` drops the calendar's times too.
- 2026-10-04: the weekly `nix flake update` is a pull request that rides a
  train, never a push to master: one landed on 2026-10-04 with a toolchain
  whose lints failed the gate on code already merged.
- 2026-10-04: the gate's browser takes its fonts from the shell
  (`FONTCONFIG_FILE`, DejaVu Sans alone, the one font the box holds). The
  Codespace had none, chromium died in Skia's font manager, and the verdict
  table named the disagreement a finding, as it should: a gate that read the
  machine was not a function of the tree.
- 2026-10-04: every PR is reviewed by a reader who did not write it, and the
  fixes are verified against the code before the PR rides a train. Round
  two caught a reading that failed whole when one chain failed (#69), a
  revert that could leave the units on an unnamed app (#68), and private
  names the term lists could not see (#67, #71).
- 2026-10-04: the public tree holds no deployment's names: categories are
  the instance's vocabulary (the cast's are work, study, project, home,
  other), the triage notes that name the owner's work are the data
  repository's `triage-notes.md`, and frozen legacy fixtures that carried a
  category word are rewritten mechanically (the word, then every hash that
  named a changed object), their readings equal under that map. The owner's
  first name as a code value (the memory kind, the command line's default
  actor) is not a leak, but it is wrong for a system anyone can run: a new
  item, `ownerkind`, makes it `Owner` with a migration of the memory store.
- 2026-10-04: considered and not adopted: Turso's sync (rows pushed and
  settled last-push-wins, so a clock picks a winner, and the cloud is the
  authority) and casita (a content-addressed repository close in spirit,
  0.1.0, whose mutable named roots are the pointer this design derives
  instead). Plain SQLite stays the candidate if the memo's tabulation ever
  outgrows files.
- 2026-10-12: the version is a reading of this chain. `suzatarytwo` is the
  2.0 release and its sub-todos its items; 2.1 and 2.2 are the todos
  `suzatarytwoone` and `suzatarytwotwo` (onboarding moves to 2.2).
  MINOR counts completed items only: `instancesplit`, cancelled, counts for
  nothing. An item is every todo a release's spec has ever named, so
  narrowing 2.0's scope never lowers the version. `cutonepointzero` is
  answered by the rule: master's hand-set 1.10.0 is untagged and folds into
  the first release a train cuts from the chain, and the stray `v1.0.0` tag
  names a release commit (fd8d6c6a) that never reached master.
