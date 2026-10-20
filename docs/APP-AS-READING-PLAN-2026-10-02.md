# The app as a reading

The owner, 2026-10-02: "Think whole app as frp", and "even switching pages and
opening a todo and all of that can be put in this framework". The law is the
Prodrome's `docs/design-register-types.md` §6.0:

    screen(t) = view(reading(history ∪ intents), ui, t)

This is Suzatary's staging of it. It starts after the phone writes `Change`s
and reads head sets (the next task in flight), which is the sync every
replica below rests on.

## What the web is today, and what it becomes

| Today | A reading |
|---|---|
| each page loads JSON from a route (`fold`, `conversation`, `inbox`, …) and draws it | each page is a pure view of readings the tab already holds |
| the todo chain is a replica in the tab; the inbox and conversations are fetched | every store is a replica in the tab, synced the same way |
| a card keeps its own pending flags; a lost answer froze one (2026-09-30) | an act is an intent, pending until the history subsumes it |
| the chat polls while a round runs | the stream advances the replica; the view follows |
| the route is the URL, read at mount | the route is a register; a page switch writes it |
| prices are numbers fetched at a moment | prices are behaviours, read again when their observed value next changes |

## The view is Typst, compiled live, over a typed state

The owner, 2026-10-02: think about "how much of it can go in typst and whether
it should, if most of it can be compiled live with typst, and we have some
sort of wasm type for the state, it means we have a lot of flexibility in
allowing the secretary forming new pages and new popups and such live, so
lets not choose how this flexibility will be used but lets design the system
to allow it". The pieces exist: the Prodrome's `typst-wasm` (`compile_html` in
the browser) and its Typst package (the list by fulfillment, an item's page,
the pie and the thirty days), composed with the core's wasm in its viewer.

- **The state is a typed value.** The tab's module answers the reading as one
  value of a declared schema (the stores' readings, the intents, the ui
  registers, `t`), handed to Typst as data (`state.json` in the compile's
  files, read with `json`). Mail-borne text in it is a string, never markup,
  and nothing calls `eval`.
- **A view is a Typst function of the state**: `view(state) -> content`. The
  pages, the popups, the cards and the marks are a Typst kit (Suzatary's
  layer over the Prodrome's package), and the shell compiles the route's view
  to HTML in the tab on every change of the state.
- **Interaction is declared, not scripted.** The kit marks what a control
  does as data: an act becomes an element carrying its intent
  (`data-intent`), a field the ui register it writes (`data-ui`). The shell
  delegates every event, parses the intent against the CLOSED intent
  vocabulary at that boundary, and refuses anything else. A non-monotone
  intent is still a `Decision` request the owner confirms.
- **Pages are data.** A view is a Typst module named by a route, so a new page
  or popup is a new module, not a new build. Where modules may come from (the
  repository's kit, a store of pages the secretary writes, a page an owner
  drafts) is left open on purpose: the system allows each, and none is chosen
  here.
- **Safe by construction, whoever wrote the page.** The compile's world holds
  only the kit and the state (no files, no network, no plugins beyond the
  declared ones), its HTML passes the existing tripwire (`render::suspicious`:
  no script, no handler, no `javascript:`), and a control can only emit an
  intent the vocabulary names. A page the secretary wrote can do nothing a
  tap could not, and every irreversible act still asks the owner.
- **What stays DOM-owned.** Text being typed, a scroll container, a streaming
  answer and focus are islands (`frame.Island`) the shell keeps, so a
  keystroke never recompiles a page; the state recompiles on a sync, an
  intent, a route change and the clock's tick.
- **Desktop and phone are one view.** The kit lays out by container width
  (one column on a phone, a list beside its page on a desktop), and every
  stage's report carries screenshots at 390 px and 1280 px, with an e2e law
  that no page overflows sideways at either.
- **The box renders the same way.** The box's Typst HTML (bodies, a reading's
  "Why this number", briefings) becomes this pipeline's server half: the same
  kit, compiled where the tab lacks the fonts or the module.

## Stages (each a PR)

0. **A spike, measured.** The tab compiles the todo list from `state.json`
   with `typst-wasm`: latency on a desktop and on a phone-class CPU (Chromium's
   throttling), first compile and incremental, against a budget (50 ms for an
   incremental compile on a phone); screenshots at both widths. If the budget
   fails, the report says where the time goes and what the fallback is.

1. **Every store a replica in the tab.** The inbox and the conversations join
   the todo chains: synced by head sets into the tab's module (`Todos`,
   `Proposals`, and a `Conversations` class), read with the module's
   readings. The routes that only served a reading of a store go when no page
   reads them. BUILT (2026-10-03,
   the `store-replicas-tab-rxijul` report): `GET
   /api/stores/<store>/{objects,events}`, `secretary::readings` as the one
   reading both sides apply, `stores.ts`/`storesync.ts` in the tab, and what a
   replica cannot say asked by content (a block by its sha, a proposal drawn
   against its chain, the memory's words). `GET /api/inbox`, the conversation
   list, a conversation's window and the search are gone. Writes still go
   through their routes; the report lists them for stage 2.
2. **Intents, one set.** Every act a page takes is an intent: a monotone one
   (complete, edit, add, say) sealed into the tab's replica and synced; a
   non-monotone one (send, perform) a request the box answers under its
   `Decision`. A card's state is derived (standing, pending, done), never
   kept. The proposal card is the first page through it. BUILT (2026-10-03,
   the `intents-one-set-s09poa` report): `web/src/prodrome/intents.ts`
   (the set, a 2P-set keyed by an act's print, with its laws),
   `asks.ts` (the closed vocabulary: every todo act a `Seal` into the tab's
   replica, every secretary act a request, with what of the replicas holds
   each) and `intentsync.ts` (the shell). The proposal card, the coding
   session's card, the transcript's rounds and the composer read the set;
   their pending flags, the card's recovery read and every `after` went.
   Every write route stays as its request's transport, and `say` answers a
   question asked twice with its first line. The conversations and the inbox
   take no posted object: the box authors each line (the redaction door, the
   queue's bound, the worker's round, the inbox's `Decision`).
3. **The ui as registers.** The route, the open entity, selection, focus, the
   open sheet, scroll and drafts become registers of the in-memory tier;
   navigation writes the route and reads again; "back" moves a pointer over
   the route log; deep links print a route. Per register, decide whether it
   syncs (a draft may follow the owner across devices; a route does not).

   BUILT (2026-10-06, the `ui-registers-stage-3-2xa4o9` report):
   `web/src/algebra/register.ts` is the register (the frontier of its
   writes, a write's past a version vector, joined by union, a conflict
   read whole), `app/ui.ts` its effectful half and `app/registers.ts` the
   one declaration of every ui register with its tier (Tab, Device, Owner),
   WEB-SPEC §3.17's table. The route register's log is App's
   (`shell/routelog.ts`); `shell/address.ts` is the one writer of the
   address, so the browser's back is the log's, and a place read again is
   put back with its scroll and focus (`view/kept.ts`). A draft to the
   secretary is an Owner register whose conflict the composer shows; every
   register in the tab, a store's and the ui's, is read by one reading
   (`register.of`) over two representations of the order. An Owner
   register NEVER enters a pushed store: if drafts travel between devices,
   it is through a box-local, unpushed tier, coalesced to each register's
   frontier, and unsent words never reach git. Not yet: that tier (today a
   draft joins this browser's tabs), and the add sheet's typed todo, which
   is still its form's island.
4. **Pages as Typst views, memoised as tabulations.** `frame.Page` loses
   `load`: a page is a Typst view of the readings and the ui, and its draw
   stays idempotent. Every per-page load, retry and "latest load lands" guard
   goes with it. The draw is the Prodrome's §6.2, and its caching is
   HIERARCHICAL, always (the owner, 2026-10-02: "so the results make sense
   to me"; "this is fully generic"). No levels are designed: the state is a
   content-addressed tree, the view a fold over it, and the cache ONE
   generic combinator, the memoised fold (`memoCata`), keyed at every node by
   (kit, the node's name), whatever the tree's shape. Its output is a tree
   of the same names placed as keyed DOM nodes, so a change compiles and
   places the spine from the changed subtree to the root and nothing else,
   and the keyed DOM keeps scroll and focus; a long sequence is a balanced
   tree, so its spine is logarithmic. Every redraw reports, per node and by
   depth, what hit and what was recomputed, with times, so a cost is
   explained by the spines that moved. The spike's whole-page compile on a phone-class CPU was 60 ms
   at 250 rows with one value changed and 13 ms with nothing changed: typst's
   own memo hits the rows, but the page above them is rebuilt and
   serialised whole each time, which is the cost this removes.
   The caches today (the box's rendered bodies and answers under
   `$XDG_CACHE_HOME`, the tab's `rendered.ts`) become that one structure:
   a store with one value per key, synced by union, evictable, never in a
   checkpoint, and a key's two values reported as a finding.

   FIRST PART BUILT (2026-10-03, the Typst list,
   the `typst-memoised-fold-t796uy` report):
   - `memo` (`web/src/algebra/memo.ts`) is the Prodrome's `memo` in
     TypeScript: `fold`, `Report`, `byDepth`, an `lru` cache and `balance`,
     with law 8 tested in node over generated trees named by the core's
     `memo_name` (the Prodrome's `memo::name`, exported by `app/wasm`). The
     Rust fold is not called per node across the boundary: its algebra
     would call back into the tab at every node it computes. Two
     generalisations, both law-tested: an algebra applies to a WAVE of
     layers (the nodes whose children are known), so a Typst compile's
     fixed cost is paid once per wave; and `balance` may cut by an
     element's identity, so a row's new content moves no cut and renames
     exactly its path.
   - The state is a tree (`viewtree.of`): the view, its list (the count,
     the rows as a balanced run cut by reference), the open todo in place
     under its row and in the pane. The kit draws one element per node
     (`list.typ`'s `draw`), compiled in a world kept between compiles; a
     node's own element and every name are tabulated too, so a spine node
     renamed only by its children compiles nothing.
   - The output is placed in keyed DOM (`place.into`), and each redraw's
     User Timing measure reports, per node and by depth, what hit, what was
     computed, what was compiled and what was placed.
   - Measured on this container at CPU rate 4, before and after over the
     same states: a one-row change 98 to 14–20 ms at 250 rows and 46 to
     7–10 ms at 53 (three runs); an Open intent on a row never opened 61 to
     14 ms and 22 to 7 ms; one opened before hits the whole page (6 and 2
     ms). The first draw is slower, 198 to 261–270 ms at 250 rows and 56 to
     62–65 at 53: the naming of every node and the keyed placement are new
     work a cold cache cannot skip.
   - Not yet: `frame.Page` still has `load` (the other pages are not Typst
     views), and the caches above are not yet this one structure.

   THE REST BUILT (2026-10-07, the `app-reading-stage-four-mgh9h8` report):
   - `frame.Page` loses `load`, `refreshed` and `frame.latest`: a page names
     what it `reads` (`web/src/algebra/reading.ts`, a functor and an
     applicative heard when any part moves), and what the tab asks the box it
     holds no replica of is `asked` (`web/src/app/asked.ts`), one register per
     question in the total order of the asks, so "the latest lands" is the
     register's order, not a guard. `Mounted.refresh` goes: the box moving asks
     every read question again.
   - `typstpage` is the list's drawer made generic; the briefings, the
     calendar's steps and the running version are Typst views through it,
     each redraw a `typst:draw` measure naming its page. The other pages keep
     their DOM views: their controls, chips, cards and editors are the open
     work (the report lists them).
   - The caches are one structure: the Prodrome's `memo::Cache` as
     `web/src/algebra/table.ts` (the fold's, the namer's, the device's rendered
     HTML synced from the box by union) and `suzatary_core::table` with
     `suzatary_io::rendered` (the box's bodies, answers and pages, in the
     process over a cache directory); a key holding two values is a finding,
     never chosen.
5. **The clock as a source, observed.** The list, the pies and the thirty
   days are views of the OBSERVED value (the whole percent, the drawn angle),
   keyed by it, and the core answers each row's next observed change from its
   fulfillment term, so the tab redraws at those instants (push-pull), not on
   a ticking clock, and a redraw recompiles only the rows whose observed
   value moved.

   BUILT (2026-10-06, the `clock-observed-source-17srek` report):
   `ChainSet::next_change` and `ChainSet::next` in the core, through the one
   set the rows are priced by, carried as `next` by `/api/fold` and
   `Todos.list`; the store holds the fold observed (`fulfillment.observed`,
   the core's `Observation::PERCENT`), and `clock.wake` reads it again at
   `next` or midnight. There was no ticking clock to delete for prices:
   the list did not move with time at all until a sync. The transcript's
   one-second ticker became a wake at its label's next second. The box's
   reading page prints through `reading::SHOWN`. Laws: `chain_set`'s
   property (every row reads the same percent until `next`, and one moves
   there), `reading`'s (a page is the same bytes until its observed value
   moves), and `web/test/clock.test.ts` over the core's fold of the fixture
   (the drawn list is byte-identical and computes nothing between changes,
   and at each change exactly the rows whose percent moved compute).
6. **The box's work as rules.** The secretary's worker, the queue, the tap's
   perform and the triage trigger become rules over registers ("when a reading
   has property P, perform E under a `Decision`, record the outcome"), one
   mechanism instead of four.
   BUILT (2026-10-05, the `app-reading-stage-6-rules-zh8mkb` report):
   `suzatary_core::rule` (`Rule`, `Memo`, `Engine`, `ReadingName`); the
   answer and the title job are `secretary::rules`, run by the server's one
   engine; the tap is `secretary::tapped`, whose witness is the `Tap`; the
   triage trigger is `mail::triage`. Not yet: the engine is told a
   conversation moved by the routes that write it (`State::heard`), not by
   the store's heads, because the owed register cannot yet tell a line
   that asks for an answer from a notice the box wrote (the report says
   why).

Each stage reports its `size:` line; stages 4 and 6 are where the web and the
box get smaller.

Later, and NOT yet thought through (the owner, 2026-10-02: "keep it in mind
as a step once our view becomes pure functions"): **the secretary appends
state of its own.** Pages as data give it new views over the schemas that
exist; this would give it new registers and stores, live, perhaps as wasm or
WASI modules beside the shipped schemas, admitted only when their laws pass
(the order is an order, an inflationary register only climbs, the reading is
a function of the object set, intents stay in the vocabulary). The Prodrome's
§6.0 records the open questions. Stages 0 to 4 must not close the door on it:
the state the views read stays one declared, extensible schema value, not a
shape fixed in TypeScript.

## What stays off the replica

Mail (notmuch is the box's cache, read through its routes), Typst rendering
(a pure function of content, memoised on the box by hash), secrets and
settings. Each is a reading the box computes, asked by content, never a store
the tab holds.
