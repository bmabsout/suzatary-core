// A week of the owner's calendars from a day, drawn by Typst on the box; every title is a stranger's words.

import type { CalendarPayload } from "../wire/generated/wire"
import { calendar } from "../wire/api"
import type { Child } from "../view/dom"
import { el } from "../view/dom"
import frame from "../view/frame"
import placeholder from "../view/placeholder"
import asked from "../app/asked"
import { richOrText } from "../rich"
import { hrefOf } from "../shell/route"
import app from "../app/app"
import time from "../app/time"

export namespace calendarPage {
  // `n` days on from an owner's day; a day is a UTC date, so no change of offset moves it.
  export const shift = (day: string, n: number): string => time.moment((time.ms(day) ?? NaN) + n * 86_400_000).slice(0, 10)

  export const start = (date: string, today: string): string => (date === "" ? today : date)

  const steps = (from: string): HTMLElement =>
    el(
      "p",
      { class: "calendarsteps" },
      el("a", { href: hrefOf({ type: "calendar", date: shift(from, -7) }) }, "Previous week"),
      " · ",
      el("a", { href: hrefOf({ type: "calendar", date: "" }) }, "Today"),
      " · ",
      el("a", { href: hrefOf({ type: "calendar", date: shift(from, 7) }) }, "Next week"),
    )

  export const view = (p: CalendarPayload): Child => [steps(p.from), richOrText(p.rich, p.text, { class: "calendarweek" })]

  const weeks = asked.family((from: string) => from, calendar, (a, b) => a.from === b.from && a.days === b.days && a.text === b.text && a.rich === b.rich)

  export const page = (date: string, today: string): frame.Page<CalendarPayload> => ({
    className: "calendarpage",
    title: "Calendar",
    legend: p => `${p.days} days from ${p.from}: the calendars at their last sync.`,
    reads: () => weeks(start(date, today)),
    shape: placeholder.ROWS,
    view,
  })

  export const mount = (root: HTMLElement, date: string) => frame.mount(root, page(date, time.date(app.now().zone)))
}
export default calendarPage
