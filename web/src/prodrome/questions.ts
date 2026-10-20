// The questions the tab asks of the core in its worker, and the one function that answers them. The worker holds each
// chain's objects, sent once and changed by the names added and dropped, and reads them into the core once per change:
// a question names a chain, an instant, a reading or terms, never objects. The same `serve` runs in the worker and,
// under test, in this thread, so an answer by message is the answer of the core read directly.

import type { FoldPayload, SpecNode } from "../wire/generated/wire"
import type { Acted, Chains, Core, CoreEnv, CoreFold, Linker, Named, Standing, Todos } from "./core"
import { appendTo, foldOf, holdIn, linked, linker, linkerOf, listOf, objectsOf, standingIn, within } from "./core"
import excluded from "./excluded"
import lo from "../algebra/lo"
import thirty from "../todo/thirty"

export namespace questions {
  // A chain as the tab holds it now, by what changed since the last holding: names the worker lacks with their bytes,
  // and names it holds that the tab no longer does.
  export type Holding = {
    readonly name: string
    readonly untrusted: readonly string[]
    readonly add: readonly Named[]
    readonly drop: readonly string[]
  }

  // What names a reading: the chain folded, the chains it is linked through as they were held (each chain and the
  // version of what it held), and the instant asked. Two chains read at one instant are two readings.
  export type Name = { readonly chain: string; readonly set: string; readonly at: string }
  export const same = (a: Name, b: Name): boolean => a.chain === b.chain && a.set === b.set && a.at === b.at

  // A reading of a chain at one instant, linked through every chain held.
  export type Read =
    | { readonly type: "Read"; readonly reading: Name; readonly at: string; readonly specs: CoreFold["specs"]; readonly functions: CoreFold["functions"] }
    | { readonly type: "Unheld" }

  // What prices a term: a reading's functions, or the box's rows, each todo's `Ref` its row's value at `at`.
  export type Basis = { readonly type: "Fold"; readonly reading: Name } | { readonly type: "Rows"; readonly at: string; readonly functions: Readonly<Record<string, SpecNode>> }

  // A term's price: its value (`null` for ∅), or why it does not link.
  export type Priced = { readonly type: "Value"; readonly v: number | null } | { readonly type: "Refused"; readonly why: string }

  // Each question, and what answers it.
  export type Table = {
    // The chains held, replacing the last holding; what each chain's reading leaves out.
    Hold: { q: { readonly chains: readonly Holding[] }; a: readonly (readonly [string, excluded.Left])[] }
    // The box's `GET …/fold` of `chain` at `at`, priced through every chain held.
    List: { q: { readonly chain: string; readonly at: string | null }; a: FoldPayload }
    // `event` sealed onto `chain` as held, and the fold after it.
    Act: { q: { readonly chain: string; readonly event: string; readonly at: string }; a: Acted }
    // The reading at `at`, kept while the set and the instant stand.
    Reading: { q: { readonly chain: string; readonly at: string }; a: Read }
    // Each term's price under `basis`, in the order asked.
    Price: { q: { readonly basis: Basis; readonly terms: readonly SpecNode[] }; a: readonly Priced[] }
    // A todo's thirty days at `ms`.
    Thirty: { q: { readonly chain: string; readonly id: string; readonly ms: number }; a: thirty.Thirty }
    // What `key` is to `actor` under `root` in `chain`.
    Standing: { q: { readonly chain: string; readonly root: string | null; readonly actor: string; readonly key: string }; a: Standing }
  }
  export type Kind = keyof Table
  export type Question = { [k in Kind]: { readonly type: k } & Table[k]["q"] }[Kind]
  // The question of one kind.
  export type Of<K extends Kind> = Extract<Question, { readonly type: K }>
  export type Answer<K extends Kind> = Table[K]["a"]

  // On the wire: each question numbered, each answer under its number, a failure in the core's words.
  export type Asked = { readonly id: number; readonly question: Question }
  export type Answered = { readonly id: number; readonly answer: lo.Settled<Answer<Kind>> }

  // How many readings are kept at once: the page's, and the one it replaced while a page still reads it.
  const KEPT = 4

  type Chain = { objects: Map<string, string>; untrusted: readonly string[]; version: number; todos: { readonly version: number; readonly t: Todos } | null }
  type Reading = { readonly answer: Read & { type: "Read" }; readonly set: Linker; readonly price: (t: SpecNode) => number | null; readonly prices: Map<string, Priced> }

  // The box's rows carry their values, so nothing is bound when one is linked.
  const UNBOUND: CoreEnv = { outcomes: {}, tended: {} }

  const priced = (price: (t: SpecNode) => number | null, t: SpecNode): Priced => {
    try {
      return { type: "Value", v: price(t) }
    } catch (e) {
      return { type: "Refused", why: lo.reason(e) }
    }
  }

  // The answerer: each chain's objects, the core's set of them, and the readings kept, all its own.
  export const serve = (core: Core): ((q: Question) => Answer<Kind>) => {
    const chains = new Map<string, Chain>()
    const set: Chains = new core.Chains()
    const inSet = new Map<string, { readonly version: number; readonly left: excluded.Left }>()
    const readings: Reading[] = []
    let versions = 0

    const named = (name: string): Chain => {
      const c = chains.get(name)
      if (c === undefined) throw new Error(`this tab holds nothing of ${name}`)
      return c
    }
    const objectsIn = (c: Chain): Named[] => [...c.objects].map(([hash, literal]) => ({ hash, literal }))
    // A chain read into the core once per change of what it holds.
    const todosOf = (c: Chain): Todos => {
      if (c.todos?.version === c.version) return c.todos.t
      c.todos?.t.free()
      const t = new core.Todos(objectsOf(objectsIn(c)))
      c.todos = { version: c.version, t }
      return t
    }
    const forget = (c: Chain): void => {
      c.todos?.t.free()
      c.todos = null
    }
    // The set holds each chain as it is now; what names it is each chain's name and version.
    const setNow = (): string => {
      for (const [name, c] of chains) {
        if (inSet.get(name)?.version === c.version) continue
        inSet.set(name, { version: c.version, left: holdIn(set, name, objectsIn(c), c.untrusted) })
      }
      for (const name of [...inSet.keys()]) {
        if (chains.has(name)) continue
        set.drop(name)
        inSet.delete(name)
      }
      return [...chains].map(([name, c]) => `${name}:${c.version}`).sort().join(";")
    }
    // The readings kept, newest last; a reading is found by its name, never by a print of it.
    const kept = (name: Name): Reading | undefined => readings.find(r => same(r.answer.reading, name))
    const keep = (r: Reading): void => {
      readings.push(r)
      for (const old of readings.splice(0, Math.max(0, readings.length - KEPT))) old.set.free()
    }
    const sorted = (xs: readonly string[]): string => JSON.stringify([...xs].sort())

    const hold = (holding: readonly Holding[]): Answer<"Hold"> => {
      for (const h of holding) {
        const c = chains.get(h.name) ?? { objects: new Map<string, string>(), untrusted: [], version: 0, todos: null }
        chains.set(h.name, c)
        for (const hash of h.drop) c.objects.delete(hash)
        for (const o of h.add) c.objects.set(o.hash, o.literal)
        if (h.add.length > 0 || h.drop.length > 0 || sorted(h.untrusted) !== sorted(c.untrusted) || c.version === 0) c.version = versions += 1
        c.untrusted = h.untrusted
      }
      for (const [name, c] of [...chains]) {
        if (holding.some(h => h.name === name)) continue
        forget(c)
        chains.delete(name)
      }
      setNow()
      return [...inSet].map(([name, i]): readonly [string, excluded.Left] => [name, i.left])
    }

    const act = (chain: string, event: string, at: string): Answer<"Act"> => {
      setNow()
      const c = named(chain)
      const t = todosOf(c)
      try {
        return { appended: appendTo(t, event), answer: listOf(t, at, c.untrusted, chain, set) }
      } finally {
        // What it holds is what the tab holds, so the act is read in again once the tab keeps it and holds it.
        forget(c)
      }
    }

    const reading = (chain: string, at: string): Answer<"Reading"> => {
      const name: Name = { chain, set: setNow(), at }
      const known = kept(name)
      if (known !== undefined) return known.answer
      const c = chains.get(chain)
      if (c === undefined) return { type: "Unheld" }
      const f = foldOf(todosOf(c), at, c.untrusted)
      const linkSet = linkerOf(set, f.functions, f.at)
      const read: Reading = {
        answer: { type: "Read", reading: name, at: f.at, specs: f.specs, functions: f.functions },
        set: linkSet,
        price: linked(core, linker(t => linkSet.link(t)), f.env, f.at),
        prices: new Map(),
      }
      keep(read)
      return read.answer
    }

    const price = (basis: Basis, terms: readonly SpecNode[]): Answer<"Price"> => {
      if (basis.type === "Rows") {
        const p = linked(core, linker(within(core, basis.functions)), UNBOUND, basis.at)
        return terms.map(t => priced(p, t))
      }
      const r = kept(basis.reading)
      if (r === undefined) throw new Error("that reading is no longer kept; the page reads again")
      return terms.map(t => {
        const key = JSON.stringify(t)
        const known = r.prices.get(key)
        if (known !== undefined) return known
        const p = priced(r.price, t)
        r.prices.set(key, p)
        return p
      })
    }

    return q => {
      switch (q.type) {
        case "Hold":
          return hold(q.chains)
        case "List": {
          setNow()
          const c = named(q.chain)
          return listOf(todosOf(c), q.at, c.untrusted, q.chain, set)
        }
        case "Act":
          return act(q.chain, q.event, q.at)
        case "Reading":
          return reading(q.chain, q.at)
        case "Price":
          return price(q.basis, q.terms)
        case "Thirty": {
          setNow()
          const c = chains.get(q.chain)
          return c === undefined ? thirty.unheld : thirty.sample(core, set, todosOf(c), c.untrusted, q.id, q.ms)
        }
        case "Standing":
          return standingIn(todosOf(named(q.chain)), q.root, q.actor, q.key)
      }
    }
  }

  // A port the answers go out on and the questions come in on: a worker's own scope, or one end of a channel.
  export type Port<In, Out> = {
    readonly postMessage: (m: Out) => void
    readonly addEventListener: (type: "message", f: (e: { readonly data: In }) => void) => void
  }

  // Each question answered in the order it came, once the core is ready; a core that did not load fails every one.
  export const listen = (port: Port<Asked, Answered>, ready: Promise<(q: Question) => Answer<Kind>>): void =>
    port.addEventListener("message", e => {
      const { id, question } = e.data
      void ready.then(
        answer => {
          const settled = ((): lo.Settled<Answer<Kind>> => {
            try {
              return lo.val(answer(question))
            } catch (x) {
              return lo.error(lo.reason(x))
            }
          })()
          port.postMessage({ id, answer: settled })
        },
        (x: unknown) => port.postMessage({ id, answer: lo.error(lo.reason(x)) }),
      )
    })
}
export default questions
