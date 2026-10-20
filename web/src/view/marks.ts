// The system's marks: the capsule rule, the diamond, and the callout the rule outlines.

import type { Child } from "./dom"
import { el, svg } from "./dom"

export namespace marks {
  const capsule = { fill: "none", "stroke-width": 4, "stroke-linecap": "round", "stroke-dasharray": "4 8" }

  export const rule = (): SVGSVGElement =>
    svg("svg", { class: "rule", "aria-hidden": "true" }, svg("line", { x1: 2, y1: 2, x2: "100%", y2: 2, ...capsule }))

  export const outline = (): SVGSVGElement =>
    svg("svg", { class: "outline", "aria-hidden": "true" }, svg("rect", { width: "100%", height: "100%", rx: 14, ...capsule }))

  // A page's head: its title and what stands under it, on a sheet, closed by the rule.
  export const head = (...kids: Child[]): HTMLElement => el("div", { class: "pagehead" }, ...kids, rule())

  export const diamond = (): HTMLElement => el("span", { class: "diamond", "aria-hidden": "true" })

  export const callout = (title: Child, ...body: Child[]): HTMLElement =>
    el("section", { class: "callout" }, outline(), el("h2", { class: "callouttitle" }, title), ...body)
}
export default marks
