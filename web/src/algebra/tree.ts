// A rose tree: a functor with its fold, and the lines a reader sees of it.

export namespace tree {
  export type Tree<A> = { v: A; kids: Tree<A>[] }

  export const of = <A>(v: A, kids: Tree<A>[]): Tree<A> => ({ v, kids })

  export const map = <A, B>(f: (a: A) => B, t: Tree<A>): Tree<B> => ({ v: f(t.v), kids: t.kids.map(k => map(f, k)) })

  export const cata = <A, R>(alg: (v: A, rs: R[]) => R, t: Tree<A>): R => alg(t.v, t.kids.map(k => cata(alg, k)))

  // The cata that keeps what it folded at every node.
  export const scan = <A, R>(alg: (v: A, rs: R[]) => R, t: Tree<A>): Tree<[A, R]> =>
    cata((v: A, ks: Tree<[A, R]>[]) => of<[A, R]>([v, alg(v, ks.map(k => k.v[1]))], ks), t)

  export const nodes = <A>(t: Tree<A>): A[] => cata((v: A, rs: A[][]) => [v, ...rs.flat()], t)

  export const size = <A>(t: Tree<A>): number => cata((_: A, rs: number[]) => rs.reduce((a, b) => a + b, 1), t)

  // Children sorted by `by`, stably, at every level.
  export const sort = <A>(by: (a: A, b: A) => number, t: Tree<A>): Tree<A> =>
    cata((v: A, ks: Tree<A>[]) => of(v, [...ks].sort((a, b) => by(a.v, b.v))), t)

  // A node as a reader sees it: how deep, keyed by its path from `root`, and the subtree it heads.
  export type Line<A> = { v: A; depth: number; key: string; parent: string; sub: Tree<A> }

  // Preorder, below the root, of every node whose ancestors are all open.
  export const visible = <A>(key: (a: A) => string, open: (a: A) => boolean, t: Tree<A>, root = ""): Line<A>[] => {
    const go = (ks: Tree<A>[], depth: number, parent: string): Line<A>[] =>
      ks.flatMap(k => {
        const at = `${parent}/${key(k.v)}`
        const line = { v: k.v, depth, key: at, parent, sub: k }
        return k.kids.length > 0 && open(k.v) ? [line, ...go(k.kids, depth + 1, at)] : [line]
      })
    return go(t.kids, 0, root)
  }
}
export default tree
