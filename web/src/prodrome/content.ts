// The lines' content on this device: each object's literal by its name, taken only when its bytes hash to that name,
// kept in the device's store and handed to the core's worker, which reads it; this thread keeps only the names.

import type { ContentPayload, SkeletonNode } from "../wire/generated/wire"
import idb from "../app/idb"
import lo from "../algebra/lo"
import { CONTENT } from "./objects"
import { sha256Hex } from "./verify"

export namespace content {
  // Who asks first: a conversation a page opens, or the whole store behind it.
  export type Lane = "First" | "Behind"

  export type Effects = {
    readonly ask: (hashes: readonly string[]) => Promise<ContentPayload>
    // The bytes a fetch will ask the box for, said once as it starts; then each batch's bytes as it comes or is lost.
    readonly asked: (bytes: number) => void
    readonly came: (bytes: number) => void
    readonly lost: (bytes: number) => void
    // How many answers may be out at once, asked as each goes out.
    readonly atOnce: () => number
    // Literals now held, handed to the reader once each; it settles once the reader has them.
    readonly held: (rows: readonly Row[]) => Promise<void>
  }

  // The box's bounds on one answer (`query::MOST_HASHES`, `routes::MOST_CONTENT_BYTES`).
  export const MOST_HASHES = 64
  export const MOST_BYTES = 1 << 20

  // Over HTTP/2 one connection carries every request. Over HTTP/1.1 a browser opens at most six to the box, a page holds
  // three open for streams (its chain's, the stores', a conversation's) and a pull piece may hold one more, so one answer
  // is out at a time and a connection stays free for every other request.
  export const atOnce = (protocol: string): number => (protocol === "h2" || protocol === "h3" ? 4 : 1)

  export type Row = { readonly hash: string; readonly literal: string }

  const memory = new Set<string>()
  let moves = 0

  export const has = (name: string): boolean => memory.has(name)
  // Counts every change of what is held, so a reading of it is named by it.
  export const version = (): number => moves

  const byName = (x: SkeletonNode, y: SkeletonNode): number => (x.hash < y.hash ? -1 : x.hash > y.hash ? 1 : 0)

  // The longest run from the front of `queue` within `MOST_HASHES` items and `MOST_BYTES` by `size`, one item alone
  // whatever its size. A fetch sorts its run by name, so one set has one URL.
  export const batch = <A>(queue: readonly A[], size: (a: A) => number): A[] => {
    const out: A[] = []
    let bytes = 0
    for (const n of queue) {
      if (out.length > 0 && (out.length === MOST_HASHES || bytes + size(n) > MOST_BYTES)) break
      out.push(n)
      bytes += size(n)
    }
    return out
  }

  // The whole store newest first: a line by when it was written, structure before every line.
  const newest = (x: SkeletonNode, y: SkeletonNode): number => {
    const at = (n: SkeletonNode): string => (n.header.node === "line" ? n.header.at : "￿")
    return at(x) < at(y) ? 1 : at(x) > at(y) ? -1 : 0
  }

  // An answer's literals, each checked against the name asked for in the same place; one that is not refuses the answer.
  export const admit = async (asked: readonly string[], answer: ContentPayload): Promise<lo.Settled<readonly Row[]>> => {
    if (answer.objects.length !== asked.length) return lo.error(`the box answered ${answer.objects.length} objects for ${asked.length} names`)
    const hashed = await Promise.all(answer.objects.map(o => sha256Hex(o.literal)))
    const wrong = asked.filter((name, i) => hashed[i] !== name || answer.objects[i]?.hash !== name)
    return wrong.length > 0 ? lo.error(`the box's bytes for ${wrong.join(", ")} are not what those names hash to`) : lo.val(answer.objects.map(o => ({ hash: o.hash, literal: o.literal })))
  }

  // Rows handed to the reader in runs of at most `MOST_BYTES`, so no one message holds the thread long.
  const take = async (fx: Effects, rows: readonly Row[]): Promise<void> => {
    const fresh = rows.filter(r => !memory.has(r.hash))
    for (let at = 0; at < fresh.length; ) {
      const run = batch(fresh.slice(at), r => r.literal.length)
      await fx.held(run)
      for (const r of run) memory.add(r.hash)
      at += run.length
    }
    if (fresh.length > 0) moves += 1
  }

  // What the device's store holds of `names`, taken in, `MOST_HASHES` names a read so no one task asks for thousands; a
  // store that does not open holds nothing.
  const load = async (fx: Effects, names: readonly string[]): Promise<void> => {
    const wanted = names.filter(n => !memory.has(n))
    if (!idb.available()) return
    for (let at = 0; at < wanted.length; at += MOST_HASHES) {
      const run = wanted.slice(at, at + MOST_HASHES)
      const found = await idb
        .within(CONTENT, "readonly", s => Promise.all(run.map(n => idb.done(s.get(n)).then((v: unknown) => (typeof v === "string" ? [{ hash: n, literal: v }] : [])))))
        .catch(() => [])
      await take(fx, found.flat())
    }
  }

  const keep = async (fx: Effects, rows: readonly Row[]): Promise<void> => {
    if (idb.available()) await idb.within(CONTENT, "readwrite", s => Promise.all(rows.map(r => idb.done(s.put(r.literal, r.hash)))))
    await take(fx, rows)
  }

  // One name asked for: queued in a lane, or in an answer that is out; settled once that answer comes or is lost.
  type Wait = { readonly node: SkeletonNode; readonly done: Promise<void>; readonly come: () => void; readonly fail: (e: unknown) => void }

  const wait = (node: SkeletonNode): Wait => {
    let come = (): void => {}
    let fail = (_: unknown): void => {}
    const done = new Promise<void>((ok, no) => ((come = ok), (fail = no)))
    // Its failure is each fetch's that waits on it; one already failed by another name is not a second failure.
    done.catch(() => {})
    return { node, done, come, fail }
  }

  // The content of the nodes it is given held, what the device keeps read first and the rest asked of the box, at most
  // `fx.atOnce` answers out. A conversation's names go before the whole store's, taken out of its queue if waiting there.
  // Rejects when an answer its names were in did not come or was not kept.
  export const fetcher = (fx: Effects): ((lane: Lane, nodes: readonly SkeletonNode[]) => Promise<void>) => {
    const waits = new Map<string, Wait>()
    const queues: { [L in Lane]: SkeletonNode[] } = { First: [], Behind: [] }
    let out = 0

    const send = async (sent: readonly SkeletonNode[]): Promise<void> => {
      const names = sent.map(n => n.hash)
      const bytes = sent.reduce((k, n) => k + n.size, 0)
      try {
        const rows = await fx.ask(names).then(a => admit(names, a))
        if (rows.type === "Error") throw new Error(rows.e)
        await keep(fx, rows.v)
        fx.came(bytes)
        for (const n of names) waits.get(n)?.come()
      } catch (e: unknown) {
        fx.lost(bytes)
        for (const n of names) waits.get(n)?.fail(e)
      } finally {
        for (const n of names) waits.delete(n)
        out -= 1
        pump()
      }
    }

    const pump = (): void => {
      while (out < fx.atOnce() && queues.First.length + queues.Behind.length > 0) {
        const lane: Lane = queues.First.length > 0 ? "First" : "Behind"
        const sent = batch(queues[lane], n => n.size).sort(byName)
        const going = new Set(sent.map(n => n.hash))
        queues[lane] = queues[lane].filter(n => !going.has(n.hash))
        out += 1
        void send(sent)
      }
    }

    return async (lane, nodes) => {
      await load(fx, nodes.map(n => n.hash))
      const lacking = nodes.filter(n => !memory.has(n.hash))
      const fresh = lacking.filter(n => !waits.has(n.hash))
      for (const n of fresh) waits.set(n.hash, wait(n))
      if (fresh.length > 0) fx.asked(fresh.reduce((k, n) => k + n.size, 0))
      const named = new Set(lacking.map(n => n.hash))
      const moved = lane === "First" ? queues.Behind.filter(n => named.has(n.hash)) : []
      if (moved.length > 0) queues.Behind = queues.Behind.filter(n => !named.has(n.hash))
      queues[lane] = [...queues[lane], ...moved, ...fresh]
      if (lane === "Behind") queues.Behind.sort(newest)
      pump()
      await Promise.all(lacking.flatMap(n => waits.get(n.hash)?.done ?? []))
    }
  }

  // Every row no name in `named` holds dropped, on the device, here and by the reader (`letGo`): content of an object
  // the skeleton no longer names is read by nothing.
  export const keepOnly = async (named: ReadonlySet<string>, letGo: (names: readonly string[]) => Promise<void>): Promise<lo.Settled<null>> => {
    const gone = [...memory].filter(n => !named.has(n))
    for (const n of gone) memory.delete(n)
    if (gone.length > 0) {
      moves += 1
      await letGo(gone)
    }
    if (!idb.available()) return lo.val(null)
    return idb
      .within(CONTENT, "readwrite", async s => {
        const keys = await idb.done(s.getAllKeys())
        await Promise.all(keys.flatMap(k => (typeof k === "string" && !named.has(k) ? [idb.done(s.delete(k))] : [])))
        return lo.val(null)
      })
      .catch((e: unknown) => lo.error(lo.reason(e)))
  }
}
export default content
