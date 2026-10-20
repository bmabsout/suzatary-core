# One mechanism per idea — 2026-09-24

The owner asked for consistent, general abstractions ("like the way we show
pages") and for cleaning up whatever needs it along the way. Chain:
`generalize` (project chain `suzatary`).

## 1. What was measured

The core is well factored (capability traits and bundles, smart
constructors, a pure core with laws). The edges grew by copying:

- **A request is described in about seven places**: adding `Act::Web`
  touched its sentence, wire shape, verb answer, performer, TypeScript type
  and guard, and eight branches of `proposal.ts` (which asks "which kind?"
  26 times).
- **Pages come in two dialects**: 8 of 26 page modules follow `WEB-SPEC.md`
  (`PageState`, pure `step`, idempotent `draw` over one `Source`); about 10
  fetch, load, fail and title themselves (17 hand-built headings, 21
  hand-rolled error lines).
- **Handlers repeat five steps** (parse path, check gate, parse body, run
  blocking work, answer) across 40 handlers; the write gate appears 10
  times, `blocking(move ||` 36.
- **The test harness has 14 `start_*` functions**, each a copy with one
  difference.

## 2. The four mechanisms, in order

1. **A card is described once, by the box.** `wire::Card` carries a
   proposal's whole presentation: title, sentence, every field unclamped, a
   verbatim body, diff rows, the answers offered (verb, label, confirm
   question, yes label, done toast, an input when the tap carries a value)
   and the dismiss word. `secretary::card::of` derives it purely, tested.
   The web draws any card with one kind-agnostic function; a new request
   kind is one Rust arm. The verb word keeps every CSS class and browser
   selector (`act-send`, …).
2. **A page is declared once.** A `page` definition takes title, loader,
   step and draw and supplies heading, loading, error, stale and refresh;
   the ten hand-built pages move onto it one per commit, the browser suite
   unchanged as proof.
3. **A handler states its gate in its signature.** axum extractors for a
   writing login, a tap and a checked JSON body, plus one "run blocking and
   answer" helper, so no handler can forget the gate.
4. **The harness is one builder.** `Server::new(caps)`, then `.acting`,
   `.settings`, `.peer`, `.start()`; the 14 variants become calls to it.

## 3. Rules

- No behaviour change beyond removing inconsistencies; the browser, node
  and cargo suites prove each commit.
- Each mechanism is a series: mechanism with tests, callers moved, old
  paths deleted.
- Clean-ups found on the way are their own commits, named for what they
  fix.
