// Runs asked one after another and running at once, their answers kept in the order asked: one landing after a
// later-asked one has is dropped, and what it holds let go, so a reader never steps back to an older answer.

import lo from "./lo"

export namespace latest {
  export type T<A> = { readonly ask: (run: () => Promise<A>) => Promise<void>; readonly close: () => void }

  // After `close` nothing lands, and every answer still on its way is let go.
  export const of = <A>(land: (l: lo.Settled<A>) => void, release: (a: A) => void = () => {}): T<A> => {
    let asked = 0
    let landed = 0
    let closed = false
    return {
      ask: run => {
        const n = ++asked
        return lo.of(new Promise<A>(ok => ok(run()))).then(l => {
          if (closed || n < landed) return void lo.map(l, release)
          landed = n
          land(l)
        })
      },
      close: () => void (closed = true),
    }
  }
}
export default latest
