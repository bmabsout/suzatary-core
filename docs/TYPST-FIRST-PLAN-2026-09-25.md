# Typst first — 2026-09-25

The owner: "I think we should be using typst for most of this if we can." Typst
0.15 builds typst.app/docs with its own HTML and bundle export, so the export
is fit for documents. The rule: **what is read is Typst; what reacts is
TypeScript.**

## 1. Today

| Surface | Renderer |
| --- | --- |
| Todo body and detail | Typst HTML (`app/io/src/render.rs`), escaped input, `suspicious` tripwire, `web/src/rich.ts` |
| Secretary answers | Typst SVG per theme (`app/io/src/answers.rs`), unrestricted language, served as bytes |
| The document (AllTodos, Details, …) | Typst SVG pages and PDF (`app/io/src/documents.rs`) |
| Everything else | TypeScript DOM, including "Why this number", the todo page's reading part, inbox cards, the briefing page, pies and fuses |

Colours live twice, and the two copies can drift: once in `web/static/style.css`
and once in `theme.typ` and `utility.typ`.

## 2. The line

**Typst renders:**
- anything read: bodies, details, explanations, cards' prose, the briefing,
  answers;
- figures: none today. The marks became colour-only (2026-09-26): a ring and
  a 30-day bar of state runs, CSS and SVG from the same samples the list uses.

**TypeScript keeps:**
- lists, rows, folding, keys, navigation, the composer;
- the pies and mini fuses (hundreds per list, redrawn on every fold);
- offline folding;
- readouts laid over a Typst figure.

**Why not all of it:**
- A compile is a request to the server, while the list must fold offline in
  under a frame.
- Typst output does not respond to input, so interaction stays in TypeScript.

## 3. One token source

`design/tokens.json` holds the app's roles, light and dark, as agreed in
`docs/DESIGN-DECISIONS-2026-09-25.md` E and A. A build step generates:
- `web/static/tokens.css`: the custom properties `style.css` reads;
- `tokens.typ`: the same names as Typst values, which `theme.typ` imports.

A test fails if either generated file is stale. Nothing else writes a colour
literal.

## 4. Rules for the HTML

- **Theme.** Typst HTML carries classes, and CSS colours them, so one render
  serves both themes.
- **Figures.** Figures embed with `html.frame` as inline SVG with literal
  colours, so they compile once per theme; the cache key includes the theme,
  as answers already do.
- **Input.** Model-written Typst refuses the `html` module and renders as plain
  text instead. Trusted-actor Typst keeps `escape_markup` and `suspicious`.
- **Caching.** Every compile is cached content-addressed (about 6 ms uncached).
  The page asks by hash or id, never by text. A reading page (steps 3 and 4)
  prints the core's values at the request instant, so nearly every request
  is a new source: it lives in a bounded process memo (256) and never on
  disk, where it would grow with the clock (2026-09-26). Bodies and answers
  keep the disk cache.
- **Offline.** A page never fetched shows the entry's plain text; a fetched one
  keeps its HTML in the tab's cache. Plain text is a real state, not a
  fallback.

## 5. The series

1. **Tokens.** `design/tokens.json`, the generator, `tokens.css` and
   `tokens.typ`; `style.css` and `theme.typ` read only from them; the staleness
   test.
2. **Answers.** Typst HTML with MathML. This is chat steps 12–14, in progress
   on a cloud branch.
3. **"Why this number".** The core's explain tree printed as Typst, rendered as
   HTML. The TypeScript explain view is deleted.
4. **The todo page's reading part** (title, meta, body, detail, why) as one
   Typst HTML document per todo. Settled 2026-09-26: ONE render per todo,
   with no theme in the key. The page carries classes and no colour (§4
   "Theme"); a theme enters the key only if a figure returns as inline SVG
   (§4 "Figures").
5. ~~**The large fuse and the 30-day chart as Typst figures.**~~ Dropped
   2026-09-26: the marks are colour-only runs, drawn in TypeScript beside the
   list's own. The PDF may draw the same runs in Typst from the core's points.
   Was: both are drawn
   from the core's curve points. A thin TypeScript overlay gives the touch
   readout from the same points. The PDF document uses the same figure
   functions.
6. **Inbox card prose and the briefing page** as Typst HTML.
7. **Docs.** WEB-SPEC §2 (the doors), ARCHITECTURE, CLAUDE.md, APP.md.
   Done 2026-09-26.

Each step deletes the TypeScript it replaces.
