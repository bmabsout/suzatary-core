// The stores' effectful shell: kept on the device, pulled from the box by heads, told of a move by each store's
// stream, and read by the core once per change of what is held, never once per question. A store's row holds its
// objects and the keys of their words; a key an erasure names is dropped once the blocks its words fed are forgotten.

import type { Chains, Conversations, Core, Proposals, Secretary } from "./core"
import { erasedOf, loadCore, objectsOf, secretaryOf } from "./core"
import heads from "./heads"
import excluded from "./excluded"
import readings from "./readings"
import locked from "./locked"
import take from "./take"
import type { Rows } from "./objects"
import { CHAIN, Stored, alterRow, dropRow, dropRows, heldRow, heldRows, holdRow, keepRow } from "./objects"
import idb from "../app/idb"
import rendered from "../app/rendered"
import { NOTHING, canonical, pulled, replicaOf, wentBack } from "./replica"
import type { Replica } from "./replica"
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
import { storeObjects } from "../wire/api"
import type { InboxProposalsPayload, MemoryPayload, StorePayload } from "../wire/generated/wire"
import pieces from "./pieces"

export namespace storesync {
  // What this tab reads the lines with, made once the core is loaded: one `Conversations` over the set of objects last
  // read, shared by the purge and every reading of that set, and what each set names as erased.
  type Reader = { readonly core: Core; readonly lines: (r: Replica) => Conversations; readonly dead: { readonly [k in stores.Name]: locked.Dead } }

  // A replica is a down-set, so its heads name its objects.
  const setOf = (r: Replica): string => `${r.generation}|${[...r.heads].sort().join(",")}`
  // A set holding no erasure line names no key, which is what most pulls hold, so the core is asked only of one that does.
  const ERASURE = /event=Erased\(/
  const NONE_DEAD: ReadonlySet<string> = new Set()

  const readerOf = (core: Core): Reader => {
    let last: { readonly at: string; readonly lines: Conversations } | null = null
    const lines = (r: Replica): Conversations => {
      const at = setOf(r)
      if (last?.at === at) return last.lines
      const made = new core.Conversations(objectsOf(r.objects))
      last?.lines.free()
      last = { at, lines: made }
      return made
    }
    const seen = table.of<ReadonlySet<string>>(table.lru(8))
    const dead: locked.Dead = r => {
      const at = setOf(r)
      const kept = table.hit(seen.get(at))
      if (kept !== undefined) return kept
      const named = r.objects.some(o => ERASURE.test(o.literal)) ? new Set(Object.keys(erasedOf(lines(r), locked.NO_KEYS, []))) : NONE_DEAD
      seen.insert(at, named)
      return named
    }
    return { core, lines, dead: { inbox: () => NONE_DEAD, lines: dead, tags: () => NONE_DEAD, "tag-aliases": () => NONE_DEAD } }
  }
  const reader = once.of(() => loadCore().then(readerOf)).get

  const source = sources.create<stores.T, stores.Event>(stores.NONE, stores.step)

  // A store's row on the device sits beside the chains', under a key no chain can be named.
  const kept = (name: stores.Name): string => `store/${name}`

  const Kept = d.obj<{ replica: Replica["held"]; keys: locked.Keys }>({ replica: Stored, keys: d.rec(d.str) })
  const rows: Rows<locked.Row> = {
    parse: raw => lo.map(Kept(raw), k => locked.of(replicaOf(canonical(k.replica)), k.keys)),
    print: r => ({ replica: r.replica.held, keys: r.keys }),
    replica: r => r.replica,
    join: locked.join,
    after: g => locked.of(CHAIN.after(g)),
    nothing: locked.NONE,
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

  // `told` is what the last piece of a pull said an erasure anywhere in the store names: owed at once, beside what the
  // objects held name, so a key held from before an erasure opens nothing from the first piece on.
  const taking = (rd: Reader, name: stores.Name, told: readonly string[]): take.Effects => ({
    held: () => source.state()[name],
    dispatch: e => source.dispatch(e.type === "Held" ? { type: "Held", name, row: e.row } : { type: "Forgot", name, keys: e.keys }),
    purge: { erased: r => erasedOf(rd.lines(r.replica), r.keys, told), forget: rendered.store.forget },
    dead: told.length === 0 ? rd.dead[name] : r => new Set([...rd.dead[name](r), ...told]),
    alter: gone => (idb.available() ? alterRow(rows, kept(name), r => locked.without(r, gone)) : Promise.resolve(lo.val(null))),
    owing: failed => owing(name, failed),
  })

  // A row as this build keeps it, or, for the inbox alone, as a build before the keys kept it: a bare replica, which is
  // deleted under a reset's rule, since the box alone writes the store and a pull takes all of it again.
  type Shape = { readonly type: "Keyed"; readonly row: locked.Row } | { readonly type: "Bare" }
  const keyed = (raw: unknown): lo.Settled<Shape> => lo.map(rows.parse(raw), (row): Shape => ({ type: "Keyed", row }))
  const shapes: { readonly [k in stores.Name]: (raw: unknown) => lo.Settled<Shape> } = {
    inbox: raw => {
      const read = keyed(raw)
      return read.type === "Val" || CHAIN.parse(raw).type !== "Val" ? read : lo.val({ type: "Bare" })
    },
    lines: keyed,
    tags: keyed,
    "tag-aliases": keyed,
  }

  // One read of each row per page.
  const restoredOne = async (rd: Reader, name: stores.Name): Promise<void> => {
    const read = await heldRow({ parse: shapes[name] }, kept(name))
    if (read.type === "Held" && read.v.type === "Bare") {
      const gone = await dropRow(CHAIN, kept(name), new Set())
      return told(name, gone.type === "Error" ? gone.e : null)
    }
    told(name, read.type === "Unreadable" ? read.why : null)
    if (read.type === "Held" && read.v.type === "Keyed") await take.into(taking(rd, name, []), read.v.row)
    // The pieces a pull kept and did not land, another tab's included: held beside the row, never read by a page.
    const left = await heldRows(rows, piecesOf(name))
    pending[name] = left.rows.reduce(locked.join, pending[name])
    if (left.unread.length > 0) told(name, left.unread.join("; "))
  }

  // Each row read once, when its store is first asked for: a page that reads only the inbox never reads the lines' row.
  const restored: { readonly [N in stores.Name]: once.T<void> } = {
    inbox: once.of(() => reader().then(rd => restoredOne(rd, "inbox"))),
    lines: once.of(() => reader().then(rd => restoredOne(rd, "lines"))),
    tags: once.of(() => reader().then(rd => restoredOne(rd, "tags"))),
    "tag-aliases": once.of(() => reader().then(rd => restoredOne(rd, "tag-aliases"))),
  }
  const restore = (): Promise<void> => Promise.all(stores.NAMES.map(name => restored[name].get())).then(() => undefined)

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

  // A piece the box answered, and the replica it was asked of.
  type Piece = { readonly asked: Replica; readonly answer: StorePayload; readonly rest: number }

  // The pieces of each store's pull that have not landed: held beside its row and asked from, never read by a page.
  const pending: { [N in stores.Name]: locked.Row } = { ...stores.NONE }

  // What the tab holds of a store, landed and not: what a pull asks from, and what a stream's heads are compared with.
  const holding = (name: stores.Name): Replica => locked.join(source.state()[name], pending[name]).replica
  // The two parts of it, each asked on its own whether a later epoch overtook it.
  const parts = (name: stores.Name): readonly Replica[] => [source.state()[name].replica, pending[name].replica]

  // Each piece kept in a row of its own, named by how many objects were held when it was asked for, so keeping one
  // writes that piece alone and a run of them reads back in the order they came.
  const piecesOf = (name: stores.Name): string => `${kept(name)}/piece/`
  const pieceAt = (name: stores.Name, at: number): string => `${piecesOf(name)}${String(at).padStart(12, "0")}`

  // One piece kept: the erasures it names forgotten from what pages read, then the piece held beside it, less any key
  // those name, and written on the device by itself. It holds only what was asked of a replica, so another tab's reset
  // kept meanwhile, a later generation, keeps it out when it lands, and the next piece is asked of the reset.
  const keep = async (rd: Reader, name: stores.Name, { asked, answer }: Piece): Promise<void> => {
    readings.told(`epoch/${kept(name)}`, excluded.behind(kept(name), wentBack(asked, answer)))
    // Only while more pieces are to come: the last lands with the erasure's lines, which then read as erased.
    if (answer.erased.length > 0 && answer.rest > 0) await take.into(taking(rd, name, answer.erased), locked.NONE)
    const piece = locked.without(locked.of(pulled(replicaOf({ ...NOTHING.held, generation: asked.generation }), answer, time.now()), answer.keys), answer.erased)
    pending[name] = locked.without(locked.join(pending[name], piece), answer.erased)
    if (idb.available() && answer.objects.length > 0) {
      const written = await holdRow(rows, pieceAt(name, asked.objects.length), { type: "Join", replica: piece })
      if (written.type === "Error") told(name, written.e)
    }
  }

  // What the pieces brought, landed in one step, so a page reading the store reads it once, whole; then the row written
  // once with what another tab kept joined in, and the pieces' own rows deleted.
  const land = async (rd: Reader, name: stores.Name, last: Piece): Promise<void> => {
    const was = source.state()[name]
    const landing = pending[name]
    await take.into(taking(rd, name, last.answer.erased), landing)
    pending[name] = locked.NONE
    const now = source.state()[name]
    // Without IndexedDB what is held is the only copy, as the chains' is.
    if (now !== was && idb.available()) {
      const { row, unread } = await keepRow(rows, kept(name), { type: "Join", replica: now })
      if (row.type === "Val") await take.into(taking(rd, name, last.answer.erased), row.v)
      told(name, unread)
    }
    if (landing !== locked.NONE && idb.available()) {
      const gone = await dropRows(piecesOf(name))
      if (gone.type === "Error") told(name, gone.e)
    }
    landings[name].done("Landed")
    if (name === "lines" && source.state().lines.replica.objects.length > 0) await retire()
  }

  // A pull that did not end whole is asked again after a wait that doubles from two seconds to a minute, and whatever
  // a stream says meanwhile asks sooner; a whole pull starts the waits over.
  // One wait per store at a time: a later failure moves it, never adds another.
  const AGAIN_MS = { first: 2_000, most: 60_000 }
  const again: { [N in stores.Name]: { wait: number; timer: ReturnType<typeof setTimeout> | undefined } } = {
    inbox: { wait: AGAIN_MS.first, timer: undefined },
    lines: { wait: AGAIN_MS.first, timer: undefined },
    tags: { wait: AGAIN_MS.first, timer: undefined },
    "tag-aliases": { wait: AGAIN_MS.first, timer: undefined },
  }
  const retry = (name: stores.Name): void => {
    const a = again[name]
    clearTimeout(a.timer)
    a.timer = setTimeout(heard, a.wait)
    a.wait = Math.min(a.wait * 2, AGAIN_MS.most)
  }
  const settled = (name: stores.Name): void => {
    const a = again[name]
    clearTimeout(a.timer)
    a.wait = AGAIN_MS.first
  }

  // A piece asked of what is held, landed and not.
  const askOf = (name: stores.Name): Promise<Piece> => {
    const asked = holding(name)
    return storeObjects(name, stores.asked(asked)).then((answer): Piece => ({ asked, answer, rest: answer.rest }))
  }

  // What the box's later epoch overtook dropped, on the device first, so no copy from before it, another tab's included,
  // reaches it: the row reset to an empty one of the generation after, keys and all, the kept pieces deleted, and the
  // pull's progress with them, so it starts again from nothing.
  const startOver = async (rd: Reader, name: stores.Name): Promise<void> => {
    const row = idb.available()
      ? await holdRow(rows, kept(name), { type: "Reset", shown: new Set(), floor: 0, over: "Unread" })
      : lo.val(locked.reset(source.state()[name]))
    if (row.type === "Error") throw new Error(row.e)
    if (idb.available()) {
      const gone = await dropRows(piecesOf(name))
      if (gone.type === "Error") throw new Error(gone.e)
    }
    pending[name] = locked.NONE
    // The store is unlanded again, so a reading waits for its next landing, never reads the emptied row. The landing
    // a reading already waits on is settled as it is replaced, so that reading asks again and waits on this one.
    const was = landings[name]
    landings[name] = awaited()
    was.done("Replaced")
    ready.forget()
    if (name === "inbox") inboxReady.forget()
    settled(name)
    await take.into(taking(rd, name, []), row.v)
  }

  // By pieces, each asked from what is held, landed and not, and kept before the next. A pull that did not end whole
  // answers false, keeps what came, and is asked again for the rest. A piece of a later epoch than all that is held
  // starts the pull over, and the store is asked for again in pieces.
  const pullNow = (name: stores.Name): Promise<boolean> =>
    restored[name]
      .get()
      .then(reader)
      .then(rd =>
        pieces.pull({
          ask: async () => {
            const piece = await askOf(name)
            if (!stores.overtaken(parts(name), piece.answer)) return piece
            await startOver(rd, name)
            return askOf(name)
          },
          keep: piece => keep(rd, name, piece),
          land: last => land(rd, name, last),
          held: () => [source.state()[name], pending[name]],
        }),
      )
      .catch((e: unknown): pieces.Ended => ({ type: "Cut", taken: 0, e }))
      .then(ended => {
        if (ended.type === "Whole") {
          settled(name)
          return true
        }
        retry(name)
        return false
      })

  // One pull per store at a time, so a pull asked for during another is a pull that starts after it.
  const pulls: { readonly [N in stores.Name]: () => Promise<boolean> } = {
    inbox: serial.of(() => pullNow("inbox")),
    lines: serial.of(() => pullNow("lines")),
    tags: serial.of(() => pullNow("tags")),
    "tag-aliases": serial.of(() => pullNow("tag-aliases")),
  }
  export const pull = (name: stores.Name): Promise<boolean> => pulls[name]()

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
  // lines holds every proposal they name. The tag stores name rounds, never a proposal, so they are pulled beside them.
  const all = (): Promise<boolean> => {
    wanted = "All"
    const tagged = Promise.all([pull("tags"), pull("tag-aliases")])
    return pull("lines").then(lines => pull("inbox").then(inbox => tagged.then(([t, a]) => lines && inbox && t && a)))
  }

  // The inbox alone, for a reading of the proposals; a move of the stores pulls the rest too once a page asked for all.
  const inboxOnly = (): Promise<boolean> => {
    if (wanted === "None") wanted = "Inbox"
    return pull("inbox")
  }

  export const sync = (): Promise<void> => all().then(() => undefined)

  // A sync that must have reached the box, or says it did not.
  export const asked = (): Promise<void> => all().then(ok => (ok ? undefined : Promise.reject(new Error("the box did not answer"))))

  // Once `name` has landed whole, from this device's rows or a pull the box said was all of it: a store that lands
  // empty moves no row, so the landing itself is what is waited on.
  // How a landing settled: the store landed, or a later epoch replaced the landing before it did (`startOver`).
  type Settled = "Landed" | "Replaced"
  type Landing = { readonly once: Promise<Settled>; readonly done: (how: Settled) => void }
  const awaited = (): Landing => {
    let done = (_how: Settled): void => {}
    const once = new Promise<Settled>(resolve => (done = resolve))
    return { once, done }
  }
  // Armed again when a later epoch drops the store (`startOver`).
  const landings: { [N in stores.Name]: Landing } = { inbox: awaited(), lines: awaited(), tags: awaited(), "tag-aliases": awaited() }
  const landed = (name: stores.Name): boolean => source.state()[name].replica.box.type === "Heard"
  // A replaced landing is waited on again through the one that replaced it; a landed one ends the wait, even empty.
  const whole = (name: stores.Name): Promise<void> =>
    landed(name) ? Promise.resolve() : landings[name].once.then(how => (how === "Landed" ? undefined : whole(name)))

  // Local-first, store by store: a store this device has landed answers at once and is pulled behind; one it never
  // landed is waited for until it is whole, so a page never reads part of it and never reads nothing for empty. A
  // later epoch forgets the cell (`startOver`), so the store is waited for again until it lands again.
  const readyOf = (names: readonly stores.Name[], pulling: () => Promise<boolean>): once.T<void> =>
    once.of(() =>
      Promise.all(names.map(name => restored[name].get())).then(() => {
        void pulling()
        return Promise.all(names.filter(name => !landed(name)).map(whole)).then(() => undefined)
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

  let read: { readonly t: stores.T; readonly s: Secretary } | null = null

  // The question runs while the reading is current: a later change frees it, so nothing may keep `s`.
  export const reading = async <A>(ask: (s: Secretary) => A): Promise<A> => {
    await ready.get()
    const rd = await reader()
    const t = source.state()
    if (read === null || read.t !== t) {
      read?.s.free()
      read = { t, s: secretaryOf(rd.core, t.inbox.replica.objects, rd.lines(t.lines.replica), t.lines.keys) }
      readings.told("stores", excluded.left(read.s.excluded))
    }
    return ask(read.s)
  }

  // The store a reading's row names, or none for a chain's.
  export const named = (key: string): stores.Name | null => stores.NAMES.find(name => kept(name) === key) ?? null

  // A store's row that does not read, written over by a reset and taken again from the box; a reset that fails is said.
  export const reset = async (name: stores.Name, said: (why: string) => void): Promise<void> => {
    const done = await holdRow(rows, kept(name), { type: "Reset", shown: new Set(), floor: 0, over: "Unread" })
    if (done.type === "Error") said(`the reset was not kept: ${done.e}`)
    const pieces = await dropRows(piecesOf(name))
    if (pieces.type === "Error") said(`the pieces of a pull were not deleted: ${pieces.e}`)
    pending[name] = locked.NONE
    restored[name].forget()
    await restore()
    await sync()
  }

  // Wakes on every change of what is held; returns its own removal. A pull changes it once, when it lands.
  export const watch = (listener: () => void): (() => void) => source.subscribe(() => listener())

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
        if (stores.behind(heads.of(f.v.heads), holding(f.v.store))) heard()
      },
    })
}
export default storesync
