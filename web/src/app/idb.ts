// The device's IndexedDB, one door: a database per use, opened for one transaction over its store and closed after
// it, and a kept row read as absent, held, a row that does not read, or a store that does not open, never one taken
// for another.

import lo from "../algebra/lo"

export namespace idb {
  export type Db = {
    readonly name: string
    readonly version: number
    readonly store: string
    // Run in the version change's own transaction, from the version the device held (0 for none).
    readonly upgrade: (db: IDBDatabase, tx: IDBTransaction, from: number) => void
  }

  export type Row<A> =
    | { readonly type: "Absent" }
    | { readonly type: "Held"; readonly v: A }
    | { readonly type: "Unreadable"; readonly why: string }
    | { readonly type: "Unavailable"; readonly why: string }

  export const available = (): boolean => typeof indexedDB !== "undefined"

  export const done = <T>(r: IDBRequest<T>): Promise<T> =>
    new Promise((ok, no) => {
      r.onsuccess = () => ok(r.result)
      r.onerror = () => no(new Error(String(r.error)))
    })

  const open = (d: Db): Promise<IDBDatabase> => {
    const r = indexedDB.open(d.name, d.version)
    // A version change always runs in its own transaction.
    r.onupgradeneeded = e => d.upgrade(r.result, r.transaction!, e.oldVersion)
    return done(r)
  }

  // `work` runs in one transaction over an opened database's store, answers once that transaction has committed, and
  // the database is closed after.
  const over = async <T>(db: IDBDatabase, d: Db, mode: IDBTransactionMode, work: (s: IDBObjectStore) => Promise<T>): Promise<T> => {
    try {
      const tx = db.transaction(d.store, mode)
      const committed = new Promise<void>((ok, no) => {
        tx.oncomplete = () => ok()
        tx.onerror = tx.onabort = () => no(new Error(String(tx.error)))
      })
      const [answer] = await Promise.all([work(tx.objectStore(d.store)), committed])
      return answer
    } finally {
      db.close()
    }
  }

  export const within = async <T>(d: Db, mode: IDBTransactionMode, work: (s: IDBObjectStore) => Promise<T>): Promise<T> => {
    if (!available()) throw new Error("this browser keeps nothing (no IndexedDB)")
    return over(await open(d), d, mode, work)
  }

  const held = <A>(l: lo.Settled<A>): Row<A> => (l.type === "Val" ? { type: "Held", v: l.v } : { type: "Unreadable", why: l.e })

  // Never throws: a row that cannot be read says why, and nothing is ever written over it here. Only a database that
  // does not open is `Unavailable`; a read or a transaction that fails is the row's.
  export const read = <A>(d: Db, key: string, parse: (u: unknown) => lo.Settled<A>): Promise<Row<A>> =>
    !available()
      ? Promise.resolve({ type: "Absent" })
      : open(d).then(
          db =>
            over(db, d, "readonly", (s): Promise<unknown> => done(s.get(key))).then(
              (u): Row<A> => (u === undefined ? { type: "Absent" } : held(parse(u))),
              (e: unknown): Row<A> => ({ type: "Unreadable", why: lo.reason(e) }),
            ),
          (e: unknown): Row<A> => ({ type: "Unavailable", why: lo.reason(e) }),
        )
}
export default idb
