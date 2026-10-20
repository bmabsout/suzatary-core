# The web app to the Prodrome's standard — 2026-09-16

The owner, 2026-09-16: "I want the quality of the webapp to be the same" as the
Prodrome's, from a programming-languages perspective. Chain `webalgebra`
(project chain `suzatary`). Follows `chatframes`
(`docs/CHATSHAPE-PLAN-2026-09-15.md` §9), the first page rebuilt to this
standard. As built: WEB-SPEC, ARCHITECTURE §10.3 "The web app's contract".

## 1. What the Prodrome's quality IS

Five mechanisms, not style: (1) **a written contract with laws** (`SPEC.md`
types §1–§7, laws §9, vectors; tests are its sentences); (2) **sums over
records-with-a-bool, constructors as the only door** (`mk_*`, prose only
through `escape_markup`); (3) **folds as monoid actions** (`extend`, one
fold for every read); (4) **pure core, effectful shell** (a capability is a
trait, a fake a value); (5) **laws as properties, evidence frozen as
vectors** (a disagreement is a finding, never a refresh).

## 2. The audit, with numbers (2026-09-16)

`web/src`: 18,132 lines, 60 files. Matches: 35 PURE modules, node-tested
(309 tests, 34 files); the doors (`dom.ts`, `api.ts`'s guards, `wire.ts`'s
`Exactly<>` pins); sums `Route`, `Screen`, `ViewReference`,
`ComposeDecision`, `Token`, `Frame`; one evaluator, `prodrome-wasm`.

Does not:
- **No contract**: route round-trip, `screenFor` totality, `reconcile`'s
  minimality, the store's fold, the round boundary are relied on, unwritten.
- **The shells are where quality drops**: eight mount functions of 150–700
  lines (`pages/conversation.ts` 702, `graph` 434, `links` 357, `todos` 332,
  `inbox` 270, `history` 235, `proposalCard` 244, `todoview` 224) hold state
  as closure flags (`last`, `partialText`, `firstDrawDone`, `usingPoll`,
  `focused`); thirteen modules keep module-level `let` (`ask.ts` 7,
  `panel.ts` 6, `nav.ts` 5, `sync.ts` 5).
- **Second copies of state**: conversation, inbox, graph, links beside
  `store.ts`.
- **Escape hatches**: 339 `as` casts, 7 non-null assertions. CORRECTED by
  commit 2: 30 element casts and all 7 assertions gone (`el` was already
  typed by tag); ceiling 289, only moves down; data casts (`todos.ts`,
  `edit.ts`, `core.ts`) to become guards.
- **Properties**: four generator files, no shared generator, no Rust vector
  but `about-fixture.json` and the wire pins.

## 3. The design

**A page is a pure function of a state value, drawn through one
reconciler, over a source that is the only effect.**

```
type PageState<S>  = closed sum per page — Loading | Loaded(S) | Failed(reason) | …
step:  (state, event) -> state             pure; event = wire payload | stream frame | user act
draw:  (state, root) -> void               deterministic and idempotent: same state, same DOM
Source<S>: { state(): PageState<S>; subscribe(fn): unsubscribe; … effects … }
```

- **`step` is the fold**: every change is an event and a pure, node-tested
  transition; a closure flag becomes an arm of the state sum.
- **`draw` is idempotent**: keyed lists via `reconcile.ts`, stable nodes
  otherwise, no deciding DOM read; checked in a browser scene by node
  identity.
- **One `Source` shape**: a record of functions over a closed state, not a
  class, the page's only effects (`store.ts`, `conversation/source.ts`, then
  inbox, graph, links).
- **A cast ceiling** in `scripts/build-web.sh` (`as ` outside `api.ts`).
- **`docs/WEB-SPEC.md`**: §1 types, §2 three doors, §3 laws naming tests,
  §4 vectors. Properties over `web/test/gen.ts` (seeded, no npm) include
  `about`/window arithmetic and each page's `step` (no illegal state
  reachable); Rust fixtures under `app/core/tests/web/` must agree with the
  pure modules.

## 4. What does not change

`dom.ts` and `rich.ts` as the doors; `textContent` for the untrusted; the
wire generated and pinned; one evaluator in wasm; no framework, no npm, one
esbuild bundle. Not a rewrite.

## 5. The series

| # | Commit | Who |
|---|---|---|
| 1 | `docs/WEB-SPEC.md` §1–§4; `web/test/gen.ts`; property tests for route, screenFor, reconcile, store, rounds, window (no code change) | **main session** |
| 2 | `el` typed by tag; casts removed; cast ceiling in build-web.sh | Sonnet |
| 3 | `Source<S>` one shape; `store.ts`, `conversation/source.ts` conform; conversation `PageState` + `step` (after chatframes 15) | Sonnet |
| 4 | inbox and todos on PageState/step/draw and a Source | Sonnet |
| 5 | graph, links, history the same | Sonnet |
| 6 | vectors (Rust writes, TS agrees); module-level `let` gone from ask, panel, nav, sync | Sonnet |
| 7 | ARCHITECTURE §10.3/§8, APP.md, CLAUDE.md Code section; the chain | **main session** |

1 first (the standard the rest is checked against); 2 beside it; 3 after
chatframes; 4–5 after 3; 6 beside 4–5; 7 last.

## 6. Risks, and what not to do

- **Do not import a virtual DOM**: a page needing more than `reconcile.ts`
  and stable nodes has state that is not yet a value.
- **Do not test the DOM in node**: no jsdom, ever; `step` in node, `draw` in
  a browser scene by node identity.
- **Do not let the spec drift from the tests**: a law with no named test is
  a sentence, not a law.
- **Do not refactor a page and change its behaviour in one commit**: scenes
  unchanged on the move; behaviour change is its own commit.
