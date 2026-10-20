// Which rows of a long table are in the document: those the reader can see and a margin either side, with the space of
// the rest held above and below them, so the scrollbar and every row's place are what the whole table's would be.

export namespace rows {
  // Rows [from, to) are drawn; `before` and `after` are the pixels of the rows left out above and below.
  export type Window = { readonly from: number; readonly to: number; readonly before: number; readonly after: number }

  // `count` rows of `px` each, seen through a box `view` pixels tall scrolled `top` down; `margin` rows either side.
  export const of = (count: number, px: number, top: number, view: number, margin: number): Window => {
    const first = Math.floor(Math.max(0, top) / px)
    const last = Math.ceil((Math.max(0, top) + Math.max(0, view)) / px)
    const from = Math.min(count, Math.max(0, first - margin))
    const to = Math.min(count, Math.max(from, last + margin))
    return { from, to, before: from * px, after: (count - to) * px }
  }

  export const same = (a: Window, b: Window): boolean => a.from === b.from && a.to === b.to && a.before === b.before && a.after === b.after

  // From the rows drawn (`kept`) to the rows wanted, both by their place in the table and in its order: the rows to take
  // out, and each new row with the kept row it goes before (`null`: at the end). A row in both is never taken out, so
  // the element the keyboard is on stays the same element.
  export type Moves<A> = { readonly gone: readonly number[]; readonly added: readonly { readonly row: A; readonly before: number | null }[] }
  export const moves = <A>(kept: readonly number[], wanted: readonly A[], key: (a: A) => number): Moves<A> => {
    const want = new Set(wanted.map(key))
    const had = new Set(kept)
    const stays = wanted.map(key).filter(i => had.has(i))
    let next = 0
    const added: { row: A; before: number | null }[] = []
    for (const a of wanted) {
      if (had.has(key(a))) next++
      else added.push({ row: a, before: stays[next] ?? null })
    }
    return { gone: kept.filter(i => !want.has(i)), added }
  }

  // How far down to scroll a box `view` pixels tall so row `i` stands in its middle.
  export const centred = (i: number, px: number, view: number): number => Math.max(0, i * px + px / 2 - view / 2)
}
export default rows
