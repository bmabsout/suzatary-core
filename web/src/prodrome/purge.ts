// The box's sweep in a tab: the keys a row owes are found, the blocks their words were rendered to forgotten while the
// keys still open them, and only then may the keys go. A forget that fails keeps them, to be asked again.

import lo from "../algebra/lo"
import type { Erased } from "./core"
import locked from "./locked"

export namespace purge {
  export type Effects = {
    // Every key `r`'s objects name as erased, with the answer keys of the blocks `keys` opens: `Conversations.erased`.
    readonly erased: (r: locked.Row) => Erased
    readonly forget: (blocks: readonly string[]) => Promise<lo.Settled<null>>
  }

  // The keys of `r` that may go now: none owed, or every owed one once its blocks are gone.
  export const forgotten = async (fx: Effects, dead: locked.Dead, r: locked.Row): Promise<lo.Settled<readonly string[]>> => {
    const owed = locked.owed(dead, r)
    if (owed.length === 0) return lo.val(owed)
    const fed = fx.erased(locked.of(r.replica, Object.fromEntries(owed.map(n => [n, r.keys[n]!]))))
    return lo.map(await fx.forget(owed.flatMap(n => fed[n] ?? [])), () => owed)
  }
}
export default purge
