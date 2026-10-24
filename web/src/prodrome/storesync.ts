// The stores' effectful shell: kept on the device, pulled from the box by heads, told of a move by each store's
// stream, and read by the core once per change of what is held, never once per question. A store's row holds its
// replica and the keys of their words; a key an erasure names is dropped once the blocks its words fed are forgotten.
// The inbox's replica holds its objects. The lines' holds their skeleton, and a reading fetches the content it needs
// by name (`content`), reading the lines only once every one it reads is held (`lines.need`).

import type { Chains, Conversations, Core, Erased, Proposals, Secretary, Window } from "./core"
import { CoreError, erasedOf, loadCore, objectsOf, secretaryOf } from "./core"
import heads from "./heads"
import excluded from "./excluded"
import readings from "./readings"
import locked from "./locked"
import take from "./take"
import lines from "./lines"
import content from "./content"
import progress from "./progress"
import type { Rows } from "./objects"
import { CHAIN, RENDERED_OWED, alterRow, dropRow, dropRows, heldRow, heldRows, holdRow, keepRow, replicaRow } from "./objects"
import idb from "../app/idb"
import rendered from "../app/rendered"
import { canonical, losses, pulled, replicaOf, wentBack } from "./replica"
import type { Held, Obj, Replica, Said } from "./replica"
import stores from "./stores"
import serial from "../algebra/serial"
import once from "../algebra/once"
import lo from "../algebra/lo"
import table from "../algebra/table"
import sources from "../algebra/source"
import stream from "../algebra/stream"
import time from "../app/time"
import d from "../wire/decoder"
import payload from "../wire/generated/payload"
import { storeContent, storeObjects, storeSkeleton } from "../wire/api"
import type { ConversationPayload, InboxProposalsPayload, MemoryPayload, SkeletonNode, StoredObject } from "../wire/generated/wire"
import pieces from "./pieces"
import http from "../wire/http"

export namespace storesync {
  // A replica is a down-set, so its heads name its objects.
  const setOf = <O extends Obj>(r: Replica<O>): string => `${r.generation}|${[...r.heads].sort().join(",")}`
  const NONE_DEAD: ReadonlySet<string> = new Set()

  // What this tab reads the lines' held content with, made once the core is loaded: one `Conversations` over the
  // largest down-set of the skeleton whose content is held, shared by the purge and the keys it names erased.
  type Reader = { readonly core: Core; readonly held: (r: Replica<SkeletonNode>) => Conversations; readonly dead: locked.Dead<SkeletonNode> }

  const readerOf = (core: Core): Reader => {
    let last: { readonly at: string; readonly lines: Conversations } | null = null
    const held = (r: Replica<SkeletonNode>): Conversations => {
      const at = `${setOf(r)}|${content.version()}`
      if (last?.at === at) return last.lines
      const made = new core.Conversations(objectsOf(lines.held(r, content.literal)))
      last?.lines.free()
      last = { at, lines: made }
      return made
    }
    const seen = table.of<ReadonlySet<string>>(table.lru(8))
    const dead: locked.Dead<SkeletonNode> = r => {
      const at = `${setOf(r)}|${content.version()}`
      const kept = table.hit(seen.get(at))
      if (kept !== undefined) return kept
      // A skeleton holding no erasure names no key, which is what most hold, so the core is asked only of one that does.
      const named = lines.erases(r) ? new Set(Object.keys(erasedOf(held(r), locked.NO_KEYS, []))) : NONE_DEAD
      seen.insert(at, named)
      return named
    }
    return { core, held, dead }
  }
  const reader = once.of(() => loadCore().then(readerOf)).get

  const source = sources.create<stores.T, stores.Event>(stores.NONE, stores.step)
  const bars = sources.create<progress.T, progress.Event>(progress.NONE, progress.step)

  // A store's row on the device sits beside the chains', under a key no chain can be named.
  const kept = (name: stores.Name): string => `store/${name}`

  // A row as this build keeps it, or, for the inbox alone, as a build before the keys kept it: a bare replica, which is
  // deleted under a reset's rule, since the box alone writes the store and a pull takes all of it again.
  type Shape<O extends Obj> = { readonly type: "Keyed"; readonly row: locked.Row<O> } | { readonly type: "Bare" }

  // One piece the box answered, as each store's pull answers it.
  type Answer<O extends Obj> = {
    readonly said: Said<O>
    readonly keys: locked.Keys
    readonly erased: readonly string[]
    readonly rest: number
    readonly size: number
    readonly weight: number
  }

  // What differs between the stores: what their replicas hold, how a piece is asked for, and what of it is erased.
  type Store<O extends Obj> = {
    readonly name: stores.Name
    readonly held: () => locked.Row<O>
    readonly hold: (row: locked.Row<O>) => void
    readonly none: locked.Row<O>
    readonly rows: Rows<locked.Row<O>>
    readonly shape: (raw: unknown) => lo.Settled<Shape<O>>
    readonly ask: (since: heads.Heads) => Promise<Answer<O>>
    readonly dead: (rd: Reader) => locked.Dead<O>
    // Every key `r`'s objects name as erased and every key in `told`, with the blocks `r.keys` opens of each.
    readonly erased: (rd: Reader, r: locked.Row<O>, told: readonly string[]) => Promise<Erased>
    // What follows a landing that moved the row.
    readonly landed: (row: locked.Row<O>) => Promise<void>
    // What runs before the row is first read.
    readonly restoring: () => Promise<void>
  }

  const keyedRows = <O extends Obj>(objects: d.Decoder<O>, none: locked.Row<O>, unsent: (r: Replica<O>) => readonly StoredObject[]): Rows<locked.Row<O>> => {
    const Kept = d.obj<{ replica: Held<O>; keys: locked.Keys }>({ replica: replicaRow(objects), keys: d.rec(d.str) })
    return {
      parse: raw => lo.map(Kept(raw), k => locked.of(replicaOf(canonical(k.replica)), k.keys)),
      print: r => ({ replica: r.replica.held, keys: r.keys }),
      generation: r => r.replica.generation,
      unsent: r => unsent(r.replica),
      join: locked.join,
      after: g => locked.of(replicaOf<O>({ ...none.replica.held, generation: g + 1 })),
      nothing: none,
    }
  }

  const keyed =
    <O extends Obj>(rows: Rows<locked.Row<O>>) =>
    (raw: unknown): lo.Settled<Shape<O>> =>
      lo.map(rows.parse(raw), (row): Shape<O> => ({ type: "Keyed", row }))

  const objectRows = keyedRows(payload.StoredObject, locked.NONE, losses)

  // A store pulled whole, its objects with their literals, which locks no words.
  type Whole = "inbox" | "topics" | "topic-aliases"
  const wholeStore = (name: Whole, hold: (row: locked.Row) => void): Store<StoredObject> => ({
    name,
    held: () => source.state()[name],
    hold,
    none: locked.NONE,
    rows: objectRows,
    shape: keyed(objectRows),
    ask: since => storeObjects(name, since).then(a => ({ said: a, keys: a.keys, erased: a.erased, rest: a.rest, size: a.size, weight: a.weight })),
    dead: () => () => NONE_DEAD,
    erased: () => Promise.resolve({}),
    landed: () => Promise.resolve(),
    restoring: () => Promise.resolve(),
  })

  const INBOX: Store<StoredObject> = {
    ...wholeStore("inbox", row => source.dispatch({ type: "Held", name: "inbox", row })),
    shape: raw => {
      const read = keyed(objectRows)(raw)
      return read.type === "Val" || CHAIN.parse(raw).type !== "Val" ? read : lo.val({ type: "Bare" })
    },
  }
  const TOPICS = wholeStore("topics", row => source.dispatch({ type: "Held", name: "topics", row }))
  const ALIASES = wholeStore("topic-aliases", row => source.dispatch({ type: "Held", name: "topic-aliases", row }))

  // The lines' content, fetched by name, each batch's bytes told to the bar.
  const fetching = content.fetcher({
    ask: hashes => storeContent("lines", hashes),
    asked: bytes => bars.dispatch({ type: "Asked", name: "lines", bytes }),
    came: bytes => bars.dispatch({ type: "Came", name: "lines", bytes }),
    lost: bytes => bars.dispatch({ type: "Lost", name: "lines", bytes }),
    atOnce: () => content.atOnce(http.protocol()),
  })

  const linesNone = locked.none<SkeletonNode>()
  // A skeleton holds no write of this device's: the box alone writes the lines.
  const linesRows = keyedRows(payload.SkeletonNode, linesNone, () => [])
  const LINES: Store<SkeletonNode> = {
    name: "lines",
    held: () => source.state().lines,
    hold: row => source.dispatch({ type: "Held", name: "lines", row }),
    none: linesNone,
    rows: linesRows,
    shape: keyed(linesRows),
    ask: since =>
      storeSkeleton(since).then(a => ({ said: { ...a, objects: a.nodes, untrusted: [], root: null }, keys: a.keys, erased: a.erased, rest: a.rest, size: a.size, weight: a.weight })),
    dead: rd => rd.dead,
    // The blocks a key's words fed are found by opening its lines, so their content is fetched first; a fetch that
    // fails rejects, and the keys stay owed.
    erased: async (rd, r, told) => {
      await fetching("First", lines.naming(r.replica, locked.names(r.keys)))
      return erasedOf(rd.held(r.replica), r.keys, told)
    },
    landed: async row => {
      if (row.replica.objects.length > 0) await retire()
      const gone = await content.keepOnly(new Set(row.replica.objects.map(n => n.hash)))
      if (gone.type === "Error") told("lines", gone.e)
    },
    restoring: () => cleared(),
  }

  // A row that does not read is said, with its own reset, never passed over.
  const told = (name: stores.Name, unread: string | null): void =>
    readings.told(`row/${kept(name)}`, excluded.left(excluded.NONE, [], unread === null ? [] : [excluded.unread(kept(name), unread)]))

  // A purge that could not forget is said, and asked again at the next sync; the keys it owes stay until it can.
  const owing = (name: stores.Name, failed: string | null): void =>
    readings.told(
      `purge/${kept(name)}`,
      excluded.left(excluded.NONE, [], failed === null ? [] : [{ chain: kept(name), why: `the blocks some erased words were rendered to are still on this device (${failed}); it tries again at the next sync`, cure: "None" }]),
    )

  // The plaintext conversations a build before the lines kept: deleted once the lines' row holds what replaces them,
  // under a reset's rule, the blocks they rendered cleared first, since nothing names which of those words went since.
  const PLAIN = "store/conversations"
  const retired = once.of(async (): Promise<void> => {
    const was = await heldRow(CHAIN, PLAIN)
    if (was.type === "Absent") return
    const cleared = await rendered.store.clear()
    if (cleared.type === "Error") throw new Error(cleared.e)
    const gone = await dropRow(CHAIN, PLAIN, new Set())
    if (gone.type === "Error") throw new Error(gone.e)
  })

  // A retirement that failed is said, and tried again at the next pull of the lines.
  const retire = (): Promise<void> =>
    retired.get().then(
      () => readings.told(`row/${PLAIN}`, excluded.NOTHING),
      (e: unknown) =>
        readings.told(
          `row/${PLAIN}`,
          excluded.left(excluded.NONE, [], [{ chain: PLAIN, why: `the conversations a build before the lines kept are still on this device (${lo.reason(e)}); it tries again at the next sync`, cure: "None" }]),
        ),
    )

  // The blocks a build before version 5 rendered, cleared once after the upgrade that dropped the lines they were
  // rendered from, since no purge can find them by their words any more; a failure is said and tried again.
  const owedClear = once.of(async (): Promise<void> => {
    const owed = await heldRow({ parse: raw => (raw === true ? lo.val(true) : lo.error("not the upgrade's mark")) }, RENDERED_OWED)
    if (owed.type === "Absent") return
    const done = await rendered.store.clear()
    if (done.type === "Error") throw new Error(done.e)
    const gone = await dropRow(CHAIN, RENDERED_OWED, new Set())
    if (gone.type === "Error") throw new Error(gone.e)
  })
  const cleared = (): Promise<void> =>
    owedClear.get().then(
      () => readings.told(`row/${RENDERED_OWED}`, excluded.NOTHING),
      (e: unknown) =>
        readings.told(
          `row/${RENDERED_OWED}`,
          excluded.left(excluded.NONE, [], [{ chain: RENDERED_OWED, why: `the blocks a build before this one rendered are still on this device (${lo.reason(e)}); it tries again at the next load`, cure: "None" }]),
        ),
    )

  // A pull that did not end whole is asked again after a wait that doubles from two seconds to a minute, and whatever
  // a stream says meanwhile asks sooner; a whole pull starts the waits over.
  const AGAIN_MS = { first: 2_000, most: 60_000 }

  // How a landing settled: the store landed, or a later epoch replaced the landing before it did (`startOver`).
  type Settled = "Landed" | "Replaced"
  type Landing = { readonly once: Promise<Settled>; readonly done: (how: Settled) => void }
  const awaited = (): Landing => {
    let done = (_how: Settled): void => {}
    const once = new Promise<Settled>(resolve => (done = resolve))
    return { once, done }
  }

  // One store's pull, its pieces kept beside its row, and its landing.
  type Sync = {
    readonly restored: once.T<void>
    readonly pull: () => Promise<boolean>
    readonly holding: () => Replica<Obj>
    readonly landed: () => boolean
    readonly whole: () => Promise<void>
    readonly reset: (said: (why: string) => void) => Promise<void>
  }

  const syncOf = <O extends Obj>(st: Store<O>, replaced: () => void): Sync => {
    const name = st.name
    const taking = (rd: Reader, told: readonly string[]): take.Effects<O> => ({
      held: st.held,
      dispatch: e => (e.type === "Held" ? st.hold(e.row) : source.dispatch({ type: "Forgot", name, keys: e.keys })),
      purge: { erased: r => st.erased(rd, r, told), forget: rendered.store.forget },
      dead: told.length === 0 ? st.dead(rd) : r => new Set([...st.dead(rd)(r), ...told]),
      alter: gone => (idb.available() ? alterRow(st.rows, kept(name), r => locked.without(r, gone)) : Promise.resolve(lo.val(null))),
      owing: failed => owing(name, failed),
    })

    // The pieces of the pull that have not landed: held beside the row and asked from, never read by a page.
    let pending = st.none
    // What the tab holds of the store, landed and not: what a pull asks from, and what a stream's heads are compared with.
    const holding = (): Replica<O> => locked.join(st.held(), pending).replica
    // The two parts of it, each asked on its own whether a later epoch overtook it.
    const parts = (): readonly Replica<O>[] => [st.held().replica, pending.replica]

    // Each piece kept in a row of its own, named by how many objects were held when it was asked for, so keeping one
    // writes that piece alone and a run of them reads back in the order they came.
    const piecesOf = `${kept(name)}/piece/`
    const pieceAt = (at: number): string => `${piecesOf}${String(at).padStart(12, "0")}`

    // One read of the row per page.
    const restoredOne = async (rd: Reader): Promise<void> => {
      await st.restoring()
      const read = await heldRow({ parse: st.shape }, kept(name))
      if (read.type === "Held" && read.v.type === "Bare") {
        const gone = await dropRow(CHAIN, kept(name), new Set())
        return told(name, gone.type === "Error" ? gone.e : null)
      }
      told(name, read.type === "Unreadable" ? read.why : null)
      if (read.type === "Held" && read.v.type === "Keyed") await take.into(taking(rd, []), read.v.row)
      // The pieces a pull kept and did not land, another tab's included: held beside the row, never read by a page.
      const left = await heldRows(st.rows, piecesOf)
      pending = left.rows.reduce(locked.join, pending)
      if (left.unread.length > 0) told(name, left.unread.join("; "))
    }
    const restored = once.of(() => reader().then(restoredOne))

    type Piece = { readonly asked: Replica<O>; readonly answer: Answer<O>; readonly rest: number }

    // One piece kept: the erasures it names forgotten from what pages read, then the piece held beside it, less any key
    // those name, and written on the device by itself. It holds only what was asked of a replica, so another tab's reset
    // kept meanwhile, a later generation, keeps it out when it lands, and the next piece is asked of the reset.
    const keep = async (rd: Reader, { asked, answer }: Piece): Promise<void> => {
      readings.told(`epoch/${kept(name)}`, excluded.behind(kept(name), wentBack(asked, answer.said)))
      bars.dispatch({ type: "Piece", name, size: answer.size, weight: answer.weight })
      // Only while more pieces are to come: the last lands with the erasure's lines, which then read as erased.
      if (answer.erased.length > 0 && answer.rest > 0) await take.into(taking(rd, answer.erased), st.none)
      const piece = locked.without(locked.of(pulled(replicaOf<O>({ ...st.none.replica.held, generation: asked.generation }), answer.said, time.now()), answer.keys), answer.erased)
      pending = locked.without(locked.join(pending, piece), answer.erased)
      if (idb.available() && answer.said.objects.length > 0) {
        const written = await holdRow(st.rows, pieceAt(asked.objects.length), { type: "Join", replica: piece })
        if (written.type === "Error") told(name, written.e)
      }
    }

    let landing = awaited()

    // What the pieces brought, landed in one step, so a page reading the store reads it once, whole; then the row written
    // once with what another tab kept joined in, and the pieces' own rows deleted.
    const land = async (rd: Reader, last: Piece): Promise<void> => {
      const was = st.held()
      const brought = pending
      await take.into(taking(rd, last.answer.erased), brought)
      pending = st.none
      const now = st.held()
      // Without IndexedDB what is held is the only copy, as the chains' is.
      if (now !== was && idb.available()) {
        const { row, unread } = await keepRow(st.rows, kept(name), { type: "Join", replica: now })
        if (row.type === "Val") await take.into(taking(rd, last.answer.erased), row.v)
        told(name, unread)
      }
      if (brought !== st.none && idb.available()) {
        const gone = await dropRows(piecesOf)
        if (gone.type === "Error") told(name, gone.e)
      }
      landing.done("Landed")
      await st.landed(st.held())
    }

    // One wait at a time: a later failure moves it, never adds another.
    let wait = AGAIN_MS.first
    let timer: ReturnType<typeof setTimeout> | undefined
    const retry = (): void => {
      clearTimeout(timer)
      timer = setTimeout(heard, wait)
      wait = Math.min(wait * 2, AGAIN_MS.most)
    }
    const settled = (): void => {
      clearTimeout(timer)
      wait = AGAIN_MS.first
    }

    // A piece asked of what is held, landed and not.
    const askOf = (): Promise<Piece> => {
      const asked = holding()
      return st.ask(stores.asked(asked)).then((answer): Piece => ({ asked, answer, rest: answer.rest }))
    }

    // What the box's later epoch overtook dropped, on the device first, so no copy from before it, another tab's included,
    // reaches it: the row reset to an empty one of the generation after, keys and all, the kept pieces deleted, and the
    // pull's progress with them, so it starts again from nothing.
    const startOver = async (rd: Reader): Promise<void> => {
      const row = idb.available()
        ? await holdRow(st.rows, kept(name), { type: "Reset", shown: new Set(), floor: 0, over: "Unread" })
        : lo.val(locked.reset(st.held()))
      if (row.type === "Error") throw new Error(row.e)
      if (idb.available()) {
        const gone = await dropRows(piecesOf)
        if (gone.type === "Error") throw new Error(gone.e)
      }
      pending = st.none
      // The store is unlanded again, so a reading waits for its next landing, never reads the emptied row. The landing
      // a reading already waits on is settled as it is replaced, so that reading asks again and waits on this one.
      const was = landing
      landing = awaited()
      was.done("Replaced")
      replaced()
      settled()
      await take.into(taking(rd, []), row.v)
    }

    // By pieces, each asked from what is held, landed and not, and kept before the next. A pull that did not end whole
    // answers false, keeps what came, and is asked again for the rest. A piece of a later epoch than all that is held
    // starts the pull over, and the store is asked for again in pieces.
    const pullNow = (): Promise<boolean> =>
      restored
        .get()
        .then(reader)
        .then(rd =>
          pieces.pull({
            ask: async () => {
              const piece = await askOf()
              if (!stores.overtaken(parts(), piece.answer.said)) return piece
              await startOver(rd)
              return askOf()
            },
            keep: piece => keep(rd, piece),
            land: last => land(rd, last),
            held: () => [st.held(), pending],
          }),
        )
        .catch((e: unknown): pieces.Ended => ({ type: "Cut", taken: 0, e }))
        .then(ended => {
          bars.dispatch({ type: "Pulled", name })
          if (ended.type === "Whole") {
            settled()
            return true
          }
          retry()
          return false
        })

    const landed = (): boolean => st.held().replica.box.type === "Heard"
    // A replaced landing is waited on again through the one that replaced it; a landed one ends the wait, even empty.
    const whole = (): Promise<void> => (landed() ? Promise.resolve() : landing.once.then(how => (how === "Landed" ? undefined : whole())))

    // A row that does not read, written over by a reset and taken again from the box; a reset that fails is said.
    const reset = async (said: (why: string) => void): Promise<void> => {
      const done = await holdRow(st.rows, kept(name), { type: "Reset", shown: new Set(), floor: 0, over: "Unread" })
      if (done.type === "Error") said(`the reset was not kept: ${done.e}`)
      const gone = await dropRows(piecesOf)
      if (gone.type === "Error") said(`the pieces of a pull were not deleted: ${gone.e}`)
      pending = st.none
      restored.forget()
    }

    // One pull at a time, so a pull asked for during another is a pull that starts after it.
    return { restored, pull: serial.of(pullNow), holding, landed, whole, reset }
  }

  // A store that lands empty moves no row, so the landing itself is what is waited on; a later epoch forgets the cells
  // that waited on the landing it replaced, so the store is waited for again until it lands again.
  const syncs: { readonly [N in stores.Name]: Sync } = {
    inbox: syncOf(INBOX, () => {
      ready.forget()
      inboxReady.forget()
    }),
    lines: syncOf(LINES, () => ready.forget()),
    topics: syncOf(TOPICS, () => ready.forget()),
    "topic-aliases": syncOf(ALIASES, () => ready.forget()),
  }

  const restore = (): Promise<void> => Promise.all(stores.NAMES.map(name => syncs[name].restored.get())).then(() => undefined)

  export const pull = (name: stores.Name): Promise<boolean> => syncs[name].pull()

  // Which stores a page has asked for so far. A stream's move or reconnect pulls those and nothing else, so before a page
  // asks it pulls nothing: the boot asks for none, and a visit that never reads a conversation never pulls the lines.
  type Wanted = "None" | "Inbox" | "All"
  let wanted: Wanted = "None"
  const heard = (): void => {
    switch (wanted) {
      case "None":
        return
      case "Inbox":
        return void inboxOnly()
      case "All":
        return void all()
    }
  }

  // The lines, then the inbox: the box files a proposal before the line that names it, so an inbox pulled after the
  // lines holds every proposal they name. The topic stores name rounds, never a proposal, so they are pulled beside them.
  const all = (): Promise<boolean> => {
    wanted = "All"
    const topicStores = Promise.all([pull("topics"), pull("topic-aliases")])
    return pull("lines").then(lines => pull("inbox").then(inbox => topicStores.then(([t, a]) => lines && inbox && t && a)))
  }

  // The inbox alone, for a reading of the proposals; a move of the stores pulls the rest too once a page asked for all.
  const inboxOnly = (): Promise<boolean> => {
    if (wanted === "None") wanted = "Inbox"
    return pull("inbox")
  }

  export const sync = (): Promise<void> => all().then(() => undefined)

  // A sync that must have reached the box, or says it did not.
  export const asked = (): Promise<void> => all().then(ok => (ok ? undefined : Promise.reject(new Error("the box did not answer"))))

  // Local-first, store by store: a store this device has landed answers at once and is pulled behind; one it never
  // landed is waited for until it is whole, so a page never reads part of it and never reads nothing for empty.
  const readyOf = (names: readonly stores.Name[], pulling: () => Promise<boolean>): once.T<void> =>
    once.of(() =>
      Promise.all(names.map(name => syncs[name].restored.get())).then(() => {
        void pulling()
        return Promise.all(names.filter(name => !syncs[name].landed()).map(name => syncs[name].whole())).then(() => undefined)
      }),
    )
  const ready = readyOf(stores.NAMES, all)
  const inboxReady = readyOf(["inbox"], inboxOnly)

  let proposalsRead: { readonly inbox: Replica; readonly p: Proposals } | null = null

  // Every proposal at `at`, priced through `chains`, read from the inbox alone: what a page that reads no conversation,
  // the "Needs you" popup, asks, so it pulls the inbox and never the lines.
  export const proposals = async (at: string, memory: MemoryPayload | null, chains: Chains): Promise<InboxProposalsPayload> => {
    await inboxReady.get()
    const rd = await reader()
    const inbox = source.state().inbox.replica
    if (proposalsRead?.inbox !== inbox) {
      proposalsRead?.p.free()
      proposalsRead = { inbox, p: new rd.core.Proposals(objectsOf(inbox.objects)) }
    }
    const read = d.json(payload.InboxProposalsPayload)(proposalsRead.p.inbox(at, chains, memory === null ? undefined : JSON.stringify(memory)))
    if (read.type === "Error") throw new Error(`the inbox's proposals: unexpected shape (${read.e})`)
    return read.v
  }

  // Each scope's reading, kept while the stores it read stand: a later change frees it.
  const reads = new Map<string, { readonly t: stores.T; readonly s: Secretary }>()
  const scopeKey = (scope: lines.Scope): string => (scope.type === "Whole" ? "" : scope.id)

  // Each scope's lines, kept while the lines' replica stands, however the other stores move: a `Conversations` reads
  // its lines once (every line verified and folded, seconds on a phone for the whole store) and every reading of it
  // after that reuses what it read.
  const linesOf = new Map<string, { readonly at: string; readonly lines: Conversations }>()
  const conversationsOf = (core: Core, key: string, at: string, objects: readonly StoredObject[]): Conversations => {
    const kept = linesOf.get(key)
    if (kept?.at === at) return kept.lines
    for (const [k, l] of linesOf) {
      if (l.at === at) continue
      l.lines.free()
      linesOf.delete(k)
    }
    const made = new core.Conversations(objectsOf(objects))
    linesOf.set(key, { at, lines: made })
    return made
  }

  // The reading of `scope` once every line it reads is held, fetching what is not, a conversation's before the whole
  // store's; the question runs while the reading is current, so nothing may keep `s`.
  const over = async <A>(scope: lines.Scope, ask: (s: Secretary) => A): Promise<A> => {
    await ready.get()
    const rd = await reader()
    for (;;) {
      const t = source.state()
      const need = lines.need(t.lines.replica, content.literal, scope)
      if (need.type === "Fetching") {
        await fetching(scope.type === "Whole" ? "Behind" : "First", need.nodes)
        continue
      }
      const key = scopeKey(scope)
      const was = reads.get(key)
      if (was?.t === t) return ask(was.s)
      for (const [k, r] of reads) {
        if (k !== key && r.t === t) continue
        r.s.free()
        reads.delete(k)
      }
      const read = conversationsOf(rd.core, key, setOf(t.lines.replica), need.objects)
      const s = secretaryOf(rd.core, t.inbox.replica.objects, read, t.lines.keys)
      reads.set(key, { t, s })
      if (scope.type === "Whole") readings.told("stores", excluded.left(s.excluded))
      return ask(s)
    }
  }

  // The whole of both stores: the list and a search read every conversation.
  export const reading = <A>(ask: (s: Secretary) => A): Promise<A> => over(lines.WHOLE, ask)

  // One conversation over its own lines alone, which reads as the whole store reads it; absent where the store holds no
  // line of it, read off the skeleton without fetching anything.
  export type One = { readonly type: "Absent" } | { readonly type: "Held"; readonly read: (w: Window, memory: MemoryPayload | null) => ConversationPayload }

  export const conversation = async <A>(id: string, ask: (one: One) => A): Promise<A> => {
    await ready.get()
    return lines.holds(source.state().lines.replica, id)
      ? over({ type: "Conversation", id }, s => ask({ type: "Held", read: (w, memory) => s.conversation(id, w, memory) }))
      : ask({ type: "Absent" })
  }

  // A conversation read where the store holds none of it, in the words the core refuses one with.
  export const absent = (id: string): never => {
    throw new CoreError(`no conversation named ${id}`)
  }

  // The store a reading's row names, or none for a chain's.
  export const named = (key: string): stores.Name | null => stores.NAMES.find(name => kept(name) === key) ?? null

  // A store's row that does not read, written over by a reset and taken again from the box; a reset that fails is said.
  export const reset = async (name: stores.Name, said: (why: string) => void): Promise<void> => {
    await syncs[name].reset(said)
    await restore()
    await sync()
  }

  // Wakes on every change of what is held; returns its own removal. A pull changes it once, when it lands.
  export const watch = (listener: () => void): (() => void) => source.subscribe(() => listener())

  // Wakes on every move of a store's sync, with how far each has come.
  export const watchProgress = (listener: (p: progress.T) => void): (() => void) => bars.subscribe(() => listener(bars.state()))

  const frame = d.json(payload.StoreFrame)

  // One stream for every store, opened once by the shell, so a store the box adds costs no connection; once a page has
  // read the stores, a reconnect pulls, and so does a frame naming an object the tab does not hold of its store, landed
  // or not, so a pull that was cut is taken up again whatever the tab last heard of the box's heads. `moved` hears each
  // store's heads from the box, the first frame of each included, whether or not a page has read them.
  export const listen = (moved: (name: stores.Name) => void): (() => void) =>
    stream.open(stream.of("/api/stores/events"), {
      opened: () => heard(),
      frame: data => {
        const f = frame(data)
        if (f.type !== "Val") return
        moved(f.v.store)
        if (stores.behind(heads.of(f.v.heads), syncs[f.v.store].holding())) heard()
      },
    })
}
export default storesync
