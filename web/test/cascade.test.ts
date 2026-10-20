// The stylesheet's cascade order: a property a conditional block (`@media`, `@supports`, `@container`) sets is never set
// again, for the same selector, by a later rule outside any at-rule, since that later rule wins at equal specificity and
// the conditional one never applies. A shorthand sets its longhands, so either one setting the other's counts.

import { test } from "node:test"
import assert from "node:assert/strict"
import { readFileSync } from "node:fs"

namespace sheet {
  export type Rule = { selectors: readonly string[]; line: number; at: readonly string[]; decls: readonly Decl[] }
  export type Decl = { property: string; line: number }
  export type Lost = { selector: string; property: string; base: string; wide: number; at: number }

  type Open = { head: string; line: number; decls: Decl[] }

  const blanked = (css: string): string => css.replace(/\/\*[\s\S]*?\*\//g, c => c.replace(/[^\n]/g, " "))

  const decl = (text: string, line: number): Decl[] => {
    const colon = text.indexOf(":")
    return colon <= 0 ? [] : [{ property: text.slice(0, colon).trim().toLowerCase(), line }]
  }

  // Every style rule, with the at-rules around it; strings are read whole, so a brace in `content` is no block.
  export const rules = (css: string): Rule[] => {
    const src = blanked(css)
    const out: Rule[] = []
    const open: Open[] = []
    let text = ""
    let start = 1
    let line = 1
    let quote: string | null = null
    for (let i = 0; i < src.length; i++) {
      const c = src[i] ?? ""
      if (c === "\n") line++
      if (quote !== null) {
        text += c
        if (c === quote && src[i - 1] !== "\\") quote = null
        continue
      }
      if (text.trim() === "" && !/\s/.test(c)) start = line
      if (c === '"' || c === "'") {
        quote = c
        text += c
      } else if (c === "{") {
        open.push({ head: text.trim(), line: start, decls: [] })
        text = ""
      } else if (c === ";" || c === "}") {
        const top = open.at(-1)
        if (top !== undefined && text.trim() !== "") top.decls.push(...decl(text.trim(), line))
        text = ""
        if (c === "}") {
          const closed = open.pop()
          if (closed !== undefined && !closed.head.startsWith("@"))
            out.push({
              selectors: closed.head.split(",").map(selector),
              line: closed.line,
              at: open.map(o => o.head),
              decls: closed.decls,
            })
        }
      } else text += c
    }
    return out
  }

  const conditional = (r: Rule): boolean => r.at.some(a => /^@(media|supports|container)\b/.test(a))

  const SIDES = ["top", "right", "bottom", "left"]
  const sided = (p: string, parts: readonly string[] = SIDES) => parts.map(x => `${p}-${x}`)
  // The longhands each shorthand sets; a property sets itself.
  const LONGHANDS: Record<string, readonly string[]> = {
    inset: SIDES,
    margin: sided("margin"),
    padding: sided("padding"),
    border: [...sided("border"), "border-width", "border-style", "border-color", ...SIDES.flatMap(x => sided(`border-${x}`, ["width", "style", "color"])), ...["width", "style", "color"].map(x => `border-${x}`)],
    "border-width": sided("border").map(b => `${b}-width`),
    "border-style": sided("border").map(b => `${b}-style`),
    "border-color": sided("border").map(b => `${b}-color`),
    ...Object.fromEntries(SIDES.map(x => [`border-${x}`, sided(`border-${x}`, ["width", "style", "color"])])),
    "border-radius": ["top-left", "top-right", "bottom-right", "bottom-left"].map(c => `border-${c}-radius`),
    background: sided("background", ["color", "image", "position", "size", "repeat", "attachment", "origin", "clip"]),
    font: sided("font", ["family", "size", "style", "weight", "variant", "stretch"]).concat("line-height"),
    flex: sided("flex", ["grow", "shrink", "basis"]),
    "flex-flow": ["flex-direction", "flex-wrap"],
    gap: ["row-gap", "column-gap"],
    overflow: ["overflow-x", "overflow-y"],
    outline: sided("outline", ["width", "style", "color"]),
    grid: ["grid-template", "grid-template-rows", "grid-template-columns", "grid-template-areas", "grid-auto-rows", "grid-auto-columns", "grid-auto-flow"],
    "grid-template": ["grid-template-rows", "grid-template-columns", "grid-template-areas"],
    "grid-area": ["grid-row", "grid-column", "grid-row-start", "grid-row-end", "grid-column-start", "grid-column-end"],
    "grid-row": ["grid-row-start", "grid-row-end"],
    "grid-column": ["grid-column-start", "grid-column-end"],
    "list-style": sided("list-style", ["type", "position", "image"]),
    "text-decoration": sided("text-decoration", ["line", "style", "color", "thickness"]),
    transition: sided("transition", ["property", "duration", "timing-function", "delay"]),
    animation: sided("animation", ["name", "duration", "timing-function", "delay", "iteration-count", "direction", "fill-mode", "play-state"]),
    "place-items": ["align-items", "justify-items"],
    "place-content": ["align-content", "justify-content"],
    "place-self": ["align-self", "justify-self"],
  }
  const sets = (p: string): ReadonlySet<string> => {
    const out = new Set<string>([p])
    const walk = (q: string): void => (LONGHANDS[q] ?? []).forEach(l => (out.has(l) ? undefined : (out.add(l), walk(l))))
    walk(p)
    return out
  }
  // Two declarations collide when either sets what the other does.
  export const collide = (a: string, b: string): boolean => sets(a).has(b) || sets(b).has(a)
  // One spelling per selector: combinators spaced alike, runs of space as one.
  export const selector = (s: string): string => s.trim().replace(/\s*([>+~])\s*/g, " $1 ").replace(/\s+/g, " ")

  // Each conditional property a later rule outside any at-rule sets again, or sets with a shorthand, for the same selector.
  export const lost = (css: string): Lost[] => {
    const rs = rules(css)
    return rs.flatMap((r, i) =>
      !conditional(r)
        ? []
        : rs.slice(i + 1).filter(b => b.at.length === 0).flatMap(b =>
            r.selectors.filter(s => b.selectors.includes(s)).flatMap(selector =>
              r.decls.flatMap(d => b.decls.filter(e => collide(d.property, e.property)).map(e => ({ selector, property: d.property, base: e.property, wide: d.line, at: e.line }))))))
  }

  export const said = (l: Lost): string =>
    `${l.selector} { ${l.property} } is set at line ${l.wide} inside a conditional block and again at line ${l.at} by a later base rule's ${l.base}, which wins`
}

test("no conditional rule loses the cascade to a later base rule", () => {
  const found = sheet.lost(readFileSync("web/static/style.css", "utf8"))
  assert.deepEqual(found.map(sheet.said), [])
})

test("the reader names a conditional rule that a later base rule overrides", () => {
  const css = [
    "@media (min-width: 62rem) {",
    "  .a, .b { left: auto; top: 0 }",
    "}",
    "/* { not a block } */",
    ".b::before { content: \"}\"; }",
    ".b {",
    "  left: 1px;",
    "}",
    "@media (min-width: 62rem) { .b { top: 1px } }",
  ].join("\n")
  assert.deepEqual(sheet.lost(css), [{ selector: ".b", property: "left", base: "left", wide: 2, at: 7 }])
})

test("every conditional block counts: max-width, range syntax, @supports and @container", () => {
  for (const head of ["@media (max-width: 61.99rem)", "@media (width >= 62rem)", "@media (hover: hover)", "@supports (display: grid)", "@container (min-width: 20rem)"]) {
    const css = `${head} { .a { color: red } }\n.a { color: blue }`
    assert.deepEqual(sheet.lost(css).map(l => l.property), ["color"], head)
  }
})

test("a later shorthand overrides its longhands, and a later longhand part of a shorthand", () => {
  const pairs: [string, string][] = [["left", "inset"], ["padding-top", "padding"], ["margin-left", "margin"], ["border-left-color", "border"], ["border-top-left-radius", "border-radius"], ["background-color", "background"], ["row-gap", "gap"], ["flex-basis", "flex"], ["padding", "padding-bottom"]]
  for (const [wide, base] of pairs) {
    const css = `@media (min-width: 62rem) { .a { ${wide}: 0 } }\n.a { ${base}: 1px }`
    assert.deepEqual(sheet.lost(css).map(l => [l.property, l.base]), [[wide, base]], `${wide} then ${base}`)
  }
  assert.equal(sheet.collide("border", "border-radius"), false, "border does not set the radius")
  assert.equal(sheet.collide("padding", "margin"), false)
})

test("a selector is one selector however its combinators are spaced", () => {
  const css = "@media (min-width: 62rem) { .a>.b, .c  +  .d { left: 0 } }\n.a > .b { left: 1px }\n.c+.d { left: 2px }"
  assert.deepEqual(sheet.lost(css).map(l => l.selector), [".a > .b", ".c + .d"])
  assert.equal(sheet.selector(" .x   ~.y "), ".x ~ .y")
})

test("a conditional rule after its base rule is not reported", () => {
  const css = ".a { left: 0 }\n@media (min-width: 62rem) { .a { left: auto } }\n.c { inset: 0 }\n@media (max-width: 61.99rem) { .c { left: 1px } }"
  assert.deepEqual(sheet.lost(css), [])
})
