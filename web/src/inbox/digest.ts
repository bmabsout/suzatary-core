// A diff in one line: its acts grouped in the order they first appear, a todo named when it is one
// and counted when several, and what the owner must see to decide said beside it, never only in the rows.

import type { ProposedObject } from "../wire/generated/wire"
import proposed from "./proposed"

export namespace digest {
  export type Group = { readonly act: proposed.Act; readonly todos: readonly string[]; readonly objects: number }
  export type Priced = { readonly todo: string; readonly percent: number }
  export type T = {
    readonly groups: readonly Group[]
    readonly urgent: readonly Priced[]
    readonly pending: readonly string[]
  }

  // SECRETARY.md's review holds a todo priced at or under this for the owner.
  export const HOLD = 45

  // The one phrasing of an act, for a row's sentence and a group's.
  export const phrase = (act: proposed.Act, what: string): string => {
    switch (act) {
      case "added": return `add ${what}`
      case "edited": return `reword ${what}`
      case "repriced": return `reprice ${what}`
      case "respecified": return `reprice ${what}`
      case "completed": return `complete ${what}`
      case "cancelled": return `cancel ${what}`
      case "reopened": return `reopen ${what}`
      case "tended": return `record a pass at ${what}`
      case "created": return `register ${what}`
    }
  }

  export const sentence = (o: ProposedObject): string => phrase(o.act, proposed.todo(o).todo)

  // The box prints a price `{:.0}%` first, then any recurrence; anything else has no number to hold on.
  export const percent = (printed: string): number | null => {
    const m = /^(\d+)%/.exec(printed)
    return m === null ? null : Number(m[1])
  }

  const unionBy = <A>(same: (x: A, y: A) => boolean) => (xs: readonly A[], ys: readonly A[]): A[] =>
    [...xs, ...ys.filter(y => !xs.some(x => same(x, y)))]
  const union = unionBy<string>((x, y) => x === y)
  const unionPriced = unionBy<Priced>((x, y) => x.todo === y.todo && x.percent === y.percent)

  export const empty: T = { groups: [], urgent: [], pending: [] }

  // A monoid: `empty` the identity, groups joined by act in first-seen order, every list a union, counts summed.
  export const combine = (a: T, b: T): T => ({
    groups: b.groups.reduce<Group[]>(
      (gs, g) => gs.some(h => h.act === g.act)
        ? gs.map(h => (h.act === g.act ? { act: h.act, todos: union(h.todos, g.todos), objects: h.objects + g.objects } : h))
        : [...gs, g],
      [...a.groups],
    ),
    urgent: unionPriced(a.urgent, b.urgent),
    pending: union(a.pending, b.pending),
  })

  export const one = (o: ProposedObject): T => {
    const t = proposed.todo(o)
    const price = proposed.changes(o).find(c => c.field === "priority")
    const p = price === undefined ? null : percent(price.after)
    return {
      groups: [{ act: o.act, todos: [t.todo], objects: 1 }],
      urgent: p !== null && p <= HOLD ? [{ todo: t.todo, percent: p }] : [],
      pending: t.pending_todo ? [t.todo] : [],
    }
  }

  export const of = (rows: readonly ProposedObject[]): T => rows.map(one).reduce(combine, empty)

  const counted = (k: number, noun: string): string => `${k} ${noun}${k === 1 ? "" : "s"}`

  const groupLine = (g: Group): string =>
    phrase(g.act, g.todos.length === 1 ? (g.todos[0] ?? "") : counted(g.todos.length, "todo"))

  export const line = (t: T): string => t.groups.map(groupLine).join(" · ")

  const price = (p: Priced): string => `${p.todo} at ${p.percent}%`

  export const urgentLine = ({ urgent }: T): string =>
    urgent.length === 0 ? "" : `At or under the ${HOLD}% the review holds: ${urgent.map(price).join(", ")}`

  export const pendingLine = ({ pending }: T): string =>
    pending.length === 0 ? "" : `Not on the chain yet, waiting on another card: ${pending.join(", ")}`
}
export default digest
