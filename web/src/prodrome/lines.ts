// What a reading of the lines needs of the device: the nodes of the skeleton it reads, and their content. A reading is
// whole or it is not made: it reads every node it needs with its literal, or it names the nodes still to fetch, never
// a fold over part of what it needs. Pure: the skeleton is a replica of nodes, the content a lookup by name.

import type { Replica } from "./replica"
import type { SkeletonNode, StoredObject } from "../wire/generated/wire"

export namespace lines {
  // The whole store, for the list and a search; or one conversation, which reads its own lines and what they rest on.
  export type Scope = { readonly type: "Whole" } | { readonly type: "Conversation"; readonly id: string }

  // Every object the reading needs, parents first, each with its literal; or the nodes whose content is not held.
  export type Need = { readonly type: "Whole"; readonly objects: readonly StoredObject[] } | { readonly type: "Fetching"; readonly nodes: readonly SkeletonNode[] }

  export type Literal = (name: string) => string | undefined

  export const WHOLE: Scope = { type: "Whole" }

  // The nodes `wanted` names and everything they rest on, parents first, by one pass back over the skeleton's order.
  const closure = (nodes: readonly SkeletonNode[], wanted: (n: SkeletonNode) => boolean): SkeletonNode[] => {
    const under = new Set<string>()
    for (const n of [...nodes].reverse()) {
      if (!under.has(n.hash) && !wanted(n)) continue
      under.add(n.hash)
      for (const p of n.parents) under.add(p)
    }
    return nodes.filter(n => under.has(n.hash))
  }

  // A conversation's lines are a register of their own: no line of one rests on a line of another, and an erasure
  // erases only its own conversation's words, so its closure reads as the whole store reads it.
  export const nodesOf = (skeleton: Replica<SkeletonNode>, scope: Scope): readonly SkeletonNode[] =>
    scope.type === "Whole" ? skeleton.objects : closure(skeleton.objects, n => n.header.node === "line" && n.header.conversation === scope.id)

  // Whether the skeleton holds any line of `id`, read off the headers alone.
  export const holds = (skeleton: Replica<SkeletonNode>, id: string): boolean =>
    skeleton.objects.some(n => n.header.node === "line" && n.header.conversation === id)

  export const need = (skeleton: Replica<SkeletonNode>, literal: Literal, scope: Scope): Need => {
    const objects: StoredObject[] = []
    const lacking: SkeletonNode[] = []
    for (const n of nodesOf(skeleton, scope)) {
      const l = literal(n.hash)
      if (l === undefined) lacking.push(n)
      else objects.push({ hash: n.hash, literal: l })
    }
    return lacking.length > 0 ? { type: "Fetching", nodes: lacking } : { type: "Whole", objects }
  }

  // The largest down-set of the skeleton whose content is held: what a purge reads, since a word was only ever rendered
  // from a conversation read whole.
  export const held = (skeleton: Replica<SkeletonNode>, literal: Literal): StoredObject[] => {
    const read = new Set<string>()
    const out: StoredObject[] = []
    for (const n of skeleton.objects) {
      const l = literal(n.hash)
      if (l === undefined || !n.parents.every(p => read.has(p))) continue
      read.add(n.hash)
      out.push({ hash: n.hash, literal: l })
    }
    return out
  }

  // The nodes of every conversation a line naming one of `keys` is in, and what they rest on.
  export const naming = (skeleton: Replica<SkeletonNode>, keys: readonly string[]): readonly SkeletonNode[] => {
    const named = new Set(keys)
    const ids = new Set(skeleton.objects.flatMap(n => (n.header.node === "line" && n.header.keys.some(k => named.has(k)) ? [n.header.conversation] : [])))
    return ids.size === 0 ? [] : closure(skeleton.objects, n => n.header.node === "line" && ids.has(n.header.conversation))
  }

  // Whether any line the skeleton holds is an erasure.
  export const erases = (skeleton: Replica<SkeletonNode>): boolean => skeleton.objects.some(n => n.header.node === "line" && n.header.kind === "Erased")
}
export default lines
