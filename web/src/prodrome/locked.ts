// A store's row: the replica of its objects and the keys of their words, joined as a pair of unions, the free
// semilattice on both, within one generation and one epoch of the box's word, over objects or a skeleton's nodes. A key its objects name as erased opens nothing (the core reads that slot as erased whatever key
// is given) and is OWED: held only until the blocks its words were rendered to are forgotten, since nothing else could
// find them, and then dropped.

import type { Obj, Replica } from "./replica"
import type { StoredObject } from "../wire/generated/wire"
import { empty, join, reset, said } from "./replica"

// The replicas' own, under names the row's join and reset do not shadow.
const joined = join
const emptied = reset

export namespace locked {
  // A key's bytes in hex, by its name: the name is the bytes' SHA-256, so one name holds one key.
  export type Keys = Readonly<Record<string, string>>
  export type Row<O extends Obj = StoredObject> = { readonly replica: Replica<O>; readonly keys: Keys }
  // The keys a replica's objects name as erased: monotone in the objects.
  export type Dead<O extends Obj = StoredObject> = (r: Replica<O>) => ReadonlySet<string>

  export const NO_KEYS: Keys = {}
  export const none = <O extends Obj>(): Row<O> => ({ replica: empty<O>(), keys: NO_KEYS })
  export const NONE: Row = none()

  export const of = <O extends Obj>(replica: Replica<O>, keys: Keys = NO_KEYS): Row<O> => ({ replica, keys })

  export const names = (k: Keys): string[] => Object.keys(k).sort()

  export const sameKeys = (a: Keys, b: Keys): boolean => {
    const [x, y] = [names(a), names(b)]
    return x.length === y.length && x.every((n, i) => n === y[i] && a[n] === b[n])
  }

  // The union, a name spelled twice keeping its least spelling, so either order holds the same keys.
  export const union = (a: Keys, b: Keys): Keys =>
    Object.fromEntries(
      [...new Set([...names(a), ...names(b)])].sort().map(n => {
        const [x, y] = [a[n], b[n]]
        return [n, x === undefined ? y! : y === undefined || x <= y ? x : y]
      }),
    )

  // The epoch of the box's word a row holds, 0 before the box has spoken: every epoch it answers with is 1 or more.
  const epochOf = <O extends Obj>(r: Row<O>): number => said(r.replica.box, 0, w => w.epoch)

  // A semilattice with `NONE` its unit, ordered by generation and then by the epoch of the box's word: the later row is
  // kept whole, so the join that brings in a later epoch's word is the one that drops every object and key of the
  // epochs before, and no reader holds the word beside them. A store is a copy of the box's alone, so nothing of this
  // device's own goes with them. Within one generation and epoch, both sides united. Nothing new is `a` itself.
  export const join = <O extends Obj>(a: Row<O>, b: Row<O>): Row<O> => {
    if (a.replica.generation === b.replica.generation && epochOf(a) !== epochOf(b)) return epochOf(a) > epochOf(b) ? a : b
    const replica = joined(a.replica, b.replica)
    const keys = a.replica.generation === b.replica.generation ? union(a.keys, b.keys) : replica === a.replica ? a.keys : b.keys
    return replica === a.replica && sameKeys(keys, a.keys) ? a : replica === b.replica && sameKeys(keys, b.keys) ? b : of(replica, keys)
  }

  // The keys the row holds that its objects name as erased.
  export const owed = <O extends Obj>(dead: Dead<O>, r: Row<O>): string[] => {
    const gone = dead(r.replica)
    return gone.size === 0 ? [] : names(r.keys).filter(n => gone.has(n))
  }

  // The row less `gone`: what a purge leaves once their blocks are forgotten, the one step down.
  export const without = <O extends Obj>(r: Row<O>, gone: readonly string[]): Row<O> => {
    const drop = new Set(gone)
    return names(r.keys).some(n => drop.has(n)) ? of(r.replica, Object.fromEntries(names(r.keys).filter(n => !drop.has(n)).map(n => [n, r.keys[n]!]))) : r
  }

  // The one way down: an empty row of the generation after, holding no key.
  export const reset = <O extends Obj>(r: Row<O>): Row<O> => of(emptied(r.replica))
}
export default locked
