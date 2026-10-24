// THE APP'S COLOUR ROLES, each a sample of the owner's design system
// (typst-design): a ramp and a position, by the package's three rules (ink on
// a 15% grid, running text at medium or deeper, quiet tones near white), and
// on the dark ground the same jobs moved by `dark-stop` and `quiet-dark`.
// `scripts/gen-tokens.mjs` prints them as web/static/tokens.css and
// web/src/view/tokens.ts; no other file names a colour for these roles, and
// the documents and the kit import the package itself.
#import "@local/typst-design:0.1.0": dark-stop, fulfillment, palette, paper-dark, quiet, quiet-dark, ramps, shade, stops

// A neutral of the identity: the maroon ramp at `t`, all but grey.
#let neutral(t, chroma: 6%) = shade(ramps.maroon, t, chroma: chroma)

#let ground-dark = neutral(16%, chroma: 4%)

#let tokens = (
  light: (
    ground: quiet(ramps.sienna, 98%),
    sheet: palette.paper.transparentize(25%),
    sheet-solid: palette.paper,
    ink: palette.ink,
    ink-muted: palette.ink-muted,
    primary: palette.primary,
    on-primary: palette.paper,
    primary-soft: palette.secondary,
    rule: palette.rule,
    tint: palette.mark-fill,
    grid: neutral(92%),
    mark-fill: palette.mark-fill,
    mark-line: palette.mark-line,
    line: palette.mark-line,
    blue-35: ramps.blue.sample(35%),
    blue-wash: quiet(ramps.blue, stops.fill),
    blue-line: quiet(ramps.blue, stops.rule),
    // Something the box cannot show as it should (a proposal line with no card): the sienna
    // ramp, so a problem never reads as the maroon of what simply waits.
    problem: ramps.sienna.sample(35%),
    problem-wash: quiet(ramps.sienna, stops.rule),
    // A fraction that judges nothing (a sync's progress, a share of a whole):
    // the blue ramp, off the fulfillment scale's crimson-to-teal path, so it
    // never reads as a verdict.
    accent: ramps.blue.sample(35%),
  ),
  dark: (
    ground: ground-dark,
    sheet: paper-dark.transparentize(25%),
    sheet-solid: paper-dark,
    ink: neutral(dark-stop(stops.ink), chroma: 3%),
    ink-muted: neutral(dark-stop(stops.medium), chroma: 10%),
    primary: ramps.maroon.sample(dark-stop(stops.strong)),
    on-primary: ground-dark,
    primary-soft: ramps.maroon.sample(dark-stop(stops.medium)),
    rule: quiet-dark(ramps.maroon, stops.rule),
    tint: quiet-dark(ramps.maroon, stops.fill),
    grid: neutral(30%, chroma: 5%),
    mark-fill: palette.mark-fill,
    mark-line: palette.mark-line,
    line: quiet-dark(ramps.maroon, stops.rule),
    blue-35: ramps.blue.sample(dark-stop(35%)),
    blue-wash: quiet-dark(ramps.blue, stops.fill),
    blue-line: quiet-dark(ramps.blue, stops.rule),
    problem: ramps.sienna.sample(dark-stop(35%)),
    problem-wash: quiet-dark(ramps.sienna, stops.rule),
    accent: ramps.blue.sample(dark-stop(35%)),
  ),
  // A fulfillment's colour, the same in both themes: the package's one scale, bad to good.
  fulfillment: fulfillment,
)
