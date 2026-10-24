// The releases the project chain reads: how far each is, and what became of each of its items.

import type { Fulfillment, ReleaseItem, ReleaseRow, ReleasesPayload } from "../wire/generated/wire"
import type { Child } from "../view/dom"
import { el } from "../view/dom"
import { chip, worded } from "../view/format"
import pie from "../view/pie"

export namespace releases {
  // An item as the page draws it: what became of it, and an open item a revision dropped apart from what is owed.
  export type Mark =
    | { readonly type: "Owed" }
    | { readonly type: "Done" }
    | { readonly type: "Cancelled" }
    | { readonly type: "Dropped" }

  const owed: Mark = { type: "Owed" }
  const done: Mark = { type: "Done" }
  const cancelled: Mark = { type: "Cancelled" }
  const dropped: Mark = { type: "Dropped" }

  // An item completed once is done, whatever its state reads today: a reopened item stays done.
  export const mark = (i: ReleaseItem): Mark =>
    i.lot === "completed"
      ? done
      : i.lot === "cancelled"
        ? cancelled
        : i.named === "dropped"
          ? dropped
          : owed

  // A dropped item says so whatever its mark: done and moved out is still moved out.
  export const movedOut = (i: ReleaseItem): boolean => i.named === "dropped"

  export const fold = <R>(m: Mark, k: { readonly Owed: R; readonly Done: R; readonly Cancelled: R; readonly Dropped: R }): R =>
    k[m.type]

  // Items by mark: a monoid under addition, `none` its identity.
  export type Tally = { readonly owed: number; readonly done: number; readonly cancelled: number; readonly dropped: number }

  export const none: Tally = { owed: 0, done: 0, cancelled: 0, dropped: 0 }
  export const combine = (a: Tally, b: Tally): Tally => ({
    owed: a.owed + b.owed,
    done: a.done + b.done,
    cancelled: a.cancelled + b.cancelled,
    dropped: a.dropped + b.dropped,
  })
  export const one = (m: Mark): Tally =>
    fold(m, {
      Owed: { ...none, owed: 1 },
      Done: { ...none, done: 1 },
      Cancelled: { ...none, cancelled: 1 },
      Dropped: { ...none, dropped: 1 },
    })
  export const tally = (r: ReleaseRow): Tally => r.items.map(i => one(mark(i))).reduce(combine, none)

  // What is owed is what is done or still open in the release now; a cancelled or dropped item leaves the count.
  export const sentence = (t: Tally): string => {
    const asides = [
      t.cancelled === 0 ? "" : `${t.cancelled} cancelled`,
      t.dropped === 0 ? "" : `${t.dropped} moved out`,
    ].filter(s => s !== "")
    return t.owed + t.done === 0
      ? "No items yet."
      : `${t.done} of ${t.owed + t.done} done${asides.length === 0 ? "" : `, ${asides.join(", ")}`}.`
  }

  export const standing = (r: ReleaseRow): string =>
    r.lot === "completed"
      ? "Shipped."
      : r.lot === "cancelled"
        ? "Cancelled."
        : r.place === "next"
          ? "In progress."
          : r.place === "later"
            ? "Later."
            : "Passed over."

  // What is still owed first, then what is done, then what is moot: stable within each.
  export const ordered = (r: ReleaseRow): readonly ReleaseItem[] => {
    const rank = (i: ReleaseItem): number => fold(mark(i), { Owed: 0, Done: 1, Cancelled: 2, Dropped: 3 })
    return [...r.items].sort((a, b) => rank(a) - rank(b))
  }

  const pieOr = (v: Fulfillment | null): Child => (typeof v === "number" ? pie.fulfilled(v) : v === null ? null : chip(worded(v)))

  const item = (i: ReleaseItem): HTMLElement =>
    el(
      "li",
      { class: `releaseitem releaseitem-${mark(i).type.toLowerCase()}`, "data-todo": i.todo },
      pieOr(i.held?.row.value ?? null),
      el("span", { class: "releaseitemtitle" }, i.held?.title ?? i.todo),
      movedOut(i) ? chip("moved out") : null,
    )

  export const release = (r: ReleaseRow): HTMLElement =>
    el(
      "section",
      { class: r.place === "next" ? "release release-next" : "release", "data-release": r.todo },
      el("h3", { class: "releasetitle" }, pieOr(r.held?.row.value ?? null), el("span", null, r.held?.title ?? r.todo)),
      el("p", { class: "settingsmeta" }, `${standing(r)} ${sentence(tally(r))}`),
      r.items.length === 0 ? null : el("ul", { class: "releaseitems" }, ordered(r).map(item)),
    )

  // What the box answered: the tree, or why it could not read it, which is shown and never hidden.
  export type Read = { readonly type: "Read"; readonly p: ReleasesPayload } | { readonly type: "Failed"; readonly why: string }

  export const read = (p: ReleasesPayload): Read => ({ type: "Read", p })
  export const failed = (e: unknown): Read => ({ type: "Failed", why: e instanceof Error ? e.message : String(e) })

  export const view = (r: Read): Child =>
    r.type === "Failed"
      ? el("p", { class: "settingshealth settingshealth-failing" }, `The releases could not be read: ${r.why}`)
      : r.p.releases.length === 0
        ? null
        : el(
            "div",
            { class: "releases" },
            el("p", { class: "settingsmeta" }, `The project chain reads ${r.p.reads}, the last completed release.`),
            r.p.releases.map(release),
          )
}
export default releases
