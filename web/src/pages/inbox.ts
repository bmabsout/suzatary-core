// The Inbox: what waits on the owner, ascending in urgency, and what was settled today, collapsed.
// Each row names what it is about: a proposal mounts the chat's own card, whoever asked for it,
// and a row this app cannot resolve is plain text with the problem and no control.
// Prose is typst's (its source was escaped before the model's words were stored); the rest is text.

import dressed from "../chat/dressed"
import storesync from "../prodrome/storesync"
import intentsync from "../prodrome/intentsync"
import type { ConversationPayload, FailedChain, MemoryPayload } from "../wire/generated/wire"
import excluded from "../prodrome/excluded"
import readings from "../prodrome/readings"
import app from "../app/app"
import lo from "../algebra/lo"
import empty from "../view/empty"
import { DEFAULT_CHAIN, PROJECT_CHAIN } from "../app/chain"
import type { Child } from "../view/dom"
import { el } from "../view/dom"
import placeholder from "../view/placeholder"
import disclosure from "../view/disclosure"
import frame from "../view/frame"
import asked from "../app/asked"
import reading from "../algebra/reading"
import { iso } from "../view/format"
import type { Row } from "../inbox/inbox"
import { rowOf, splitSetup, waitingSentence } from "../inbox/inbox"
import waiting from "../inbox/waiting"
import asker from "../inbox/asker"
import onScreen from "../inbox/onscreen"
import { proposalCard } from "../inbox/proposal"
import { print } from "../chat/refs"
import { richOrText } from "../rich"
import time from "../app/time"
import { boxFolds, pullServed, watchOthers } from "../prodrome/sync"
import inboxState from "./inbox.state"
import address from "../shell/address"
import { hrefOf } from "../shell/route"

export namespace inboxPage {
  type T = inboxState.T
  type Fx = { readonly dispatch: (e: inboxState.Event) => void; readonly again: () => void }

  // Named, not discovered: `/api/me` lands after the first paint, and this page's job is to answer quickly.
  const CHAINS: readonly string[] = [DEFAULT_CHAIN, PROJECT_CHAIN]

  // What the box answered: each chain's fold (the decisions held for the owner) and the memory's words. The page draws what the tab holds with the last of these at once and asks again in the background, so no
  // fetch stands between the owner and the page. A source that refused is skipped and named.
  export type Answers = {
    readonly folds: readonly (waiting.ChainFold & { readonly failed: readonly FailedChain[] })[]
    readonly memory: MemoryPayload | null
    readonly problems: readonly string[]
  }
  const NONE: Answers = { folds: [], memory: null, problems: [] }

  const said = (what: string, l: lo.T<unknown>): string[] => lo.fold(l, { loading: () => [], error: e => [`${what}: ${e}`], val: () => [] })

  // An answer is data: one equal to the answer standing moves nothing, so nothing is gathered
  // again for it. Each chain's fold is the one kept while the box's heads stand (`boxFolds`, the open chain's heard on
  // its stream), so a visit with nothing moved fetches no fold; the box moving asks again, and so does a pull bringing
  // another chain something.
  export const answers: reading.Reading<Answers> = reading.againOn(asked.question(
    () =>
      Promise.all([
        dressed.asked(),
        Promise.all(CHAINS.map(chain => lo.of(boxFolds.of(chain)).then(l => lo.map(l, p => ({ chain, entries: p.entries, failed: p.failed }))))),
      ]).then(([memory, folds]) => {
        const held = folds.flatMap(f => lo.fold(f, { loading: () => [], error: () => [], val: v => [v] }))
        return { folds: held, memory, problems: folds.flatMap((f, i) => said(`The ${CHAINS[i] ?? ""} chain did not answer`, f)) }
      }),
  ), watchOthers)

  // What waits, read from the tab and the box's last answer, and whether that answer is in yet, with each source that
  // failed by name; every chain a reading was priced without is told to the one line.
  const gather = async (): Promise<{ all: waiting.Waiting[]; unplaced: waiting.Unplaced[]; problems: string[]; box: inboxState.Box; memory: MemoryPayload | null }> => {
    const answered = answers.now()
    const known = lo.withDefault(answered, NONE)
    const [inbox, listed] = await Promise.all([lo.of(dressed.inbox(time.moment(), known.memory)), lo.of(storesync.list())])
    const read = lo.withDefault(inbox, null)
    readings.told("inbox", excluded.left(excluded.NONE, [...(read?.payload.failed ?? []), ...known.folds.flatMap(f => f.failed)]))
    return {
      all: waiting.all(read?.payload.proposals ?? [], known.folds, read?.held ?? null),
      unplaced: waiting.unplacedOf(lo.withDefault(listed, null)?.conversations ?? []),
      problems: [...said("The Inbox did not read", inbox), ...said("The conversations did not read", listed), ...known.problems],
      box: answered.type === "Val" ? "Answered" : "Asking",
      memory: known.memory,
    }
  }

  // Every conversation the open rows name, so each is fetched once however many rows share it.
  const referenced = (rows: readonly waiting.Waiting[]): Set<string> => {
    const conversations = new Set<string>()
    for (const w of rows) {
      const at = waiting.where(w)
      if (at.type === "Proposal") conversations.add(at.conversation)
    }
    return conversations
  }

  const fetched = <A>(keys: Set<string>, ask: (k: string) => Promise<A>): Promise<Map<string, A>> =>
    Promise.all([...keys].map(k => ask(k).then((a): [string, A] => [k, a], () => null)))
      .then(xs => new Map(xs.filter(x => x !== null)))

  // The one resolution of open rows to cards, which Settings reads through `waits` too; `memory` gives a remember
  // card its words.
  const resolveRows = async (open: readonly waiting.Waiting[], memory: MemoryPayload | null): Promise<Row[]> => {
    const conversations = await fetched<ConversationPayload>(referenced(open), id => dressed.cards(id, memory))
    return open.map(w => rowOf(w, conversations))
  }

  // A card that might perform something else is worse than none.
  const unresolvedCard = (row: Row & { kind: "unresolved" }): HTMLElement =>
    el("section", { class: "batch decision-unresolved", "data-decision": waiting.key(row.w) },
      waiting.fold(
        row.w,
        p => [el("p", { class: "proposalbody" }, p.headline), el("p", { class: "proposalwhy" }, p.why)],
        (_, entry) => {
          const a = entry.authored
          return [
            a === null ? el("p", { class: "proposalbody" }, entry.todo) : richOrText(a.rich.body, a.body, { class: "proposalbody" }),
            a === null || a.rationale.length === 0 ? null : richOrText(a.rich.rationale, a.rationale.join(" "), { class: "proposalwhy" }),
          ]
        },
      ),
      el("p", { class: "problem" }, row.problem))

  const proposal = (row: Row & { kind: "proposal" }, v: T): HTMLElement =>
    proposalCard(row.conversation.id, row.proposal, {
      // A setup card sits in the Setup row, which already says where it is from and who asked.
      ...(asker.isSetup(row.proposal.card.by) ? { askerSaidBy: "row" as const } : { context: row.conversation.title }),
      // This page has no compose box, so Reply opens the conversation with the proposal attached.
      reply: p => {
        address.go({
          type: "conversation",
          id: row.conversation.id,
          refs: [print({ kind: "proposal", conversation: row.conversation.id, index: p.index })],
        })
      },
      intents: v.intents,
      titles: v.titles,
    })

  const settledSection = (settled: readonly waiting.Settled[], open: boolean, fx: Fx): HTMLElement | null => {
    if (settled.length === 0) return null
    const details = el("details", { class: "settledtoday", open },
      el("summary", null, `Settled today: ${settled.length} decision${settled.length === 1 ? "" : "s"}`),
      settled.map(({ w, how, at }) =>
        el("div", { class: `settledbatch settled-${how}`, "data-settled": waiting.key(w) },
          el("p", { class: "settledhead" },
            el("span", { class: "settledwhat" }, how),
            el("span", { class: "settledchain" }, waiting.source(w)),
            el("span", { class: "settledwhen" }, iso(at))),
          waiting.fold(
            w,
            p => el("p", { class: "settledwhy" }, p.headline),
            (_, entry) =>
              entry.authored === null
                ? el("p", { class: "settledwhy" }, entry.todo)
                : richOrText(entry.authored.rich.body, entry.authored.body, { class: "settledwhy" }),
          ))))
    details.addEventListener("toggle", () => fx.dispatch({ type: "SettledOpen", open: details.open }))
    return details
  }

  const unreadableSection = (ws: readonly waiting.Waiting[]): HTMLElement | null =>
    ws.length === 0 ? null
    : el("div", { class: "unreadable" },
        ws.map(w =>
          el("div", { class: "problem", "data-unreadable": waiting.key(w) },
            el("p", null, `Cannot be read, so nothing acts on it: ${waiting.unreadable(w) ?? ""}.`),
            el("p", { class: "settledwhy" }, waiting.asked(w)))))

  // A proposal line with no card says why, and links to the conversation that holds it.
  const unplacedSection = (lines: readonly waiting.Unplaced[]): HTMLElement | null =>
    lines.length === 0 ? null
    : el("div", { class: "unplaced" },
        lines.map(u => {
          const line = print({ kind: "proposal", conversation: u.conversation, index: u.index })
          return el("div", { class: "problem", "data-unplaced": line },
            el("p", null, `No card for ${line}: ${waiting.unheld(u.why)}.`),
            el("a", { href: hrefOf({ type: "conversation", id: u.conversation, refs: [line] }) }, "Open its conversation"))
        }))

  const card = (row: Row, v: T, fx: Fx): HTMLElement => {
    const node = row.kind === "proposal" ? proposal(row, v) : unresolvedCard(row)
    const key = waiting.key(row.w)
    node.dataset["decision"] = key
    // Its place is the bottom of the list because its price is unknown here, which the owner should not read as low stakes.
    const unheld = waiting.notYetPriced(row.w)
    if (unheld !== null)
      node.prepend(el("p", { class: "legend notyetpriced" }, `Not yet priced: this device holds nothing of ${unheld} yet, so it is listed last.`))
    // The row a push or a popup opened is marked, and scrolled to once.
    if (v.opening?.key === key) {
      node.classList.add("opened")
      if (v.opening.stage !== "Scrolled") requestAnimationFrame(() => (node.scrollIntoView({ block: "center" }), fx.dispatch({ type: "Scrolled" })))
    }
    return node
  }

  const EMPTY =
    "Nothing is waiting on you. Changes the triage agent and the secretary propose appear here."

  // One sentence, not two: an empty queue's note is the answer, so the legend says nothing unless a source failed.
  // A line with no card waits on him as a card does, so it is counted here and the empty note never stands above it.
  const owed = (v: T): number => v.rows.length + v.unplaced.length

  // What is shown is counted as it is shown: the work, then the setup steps folded under it.
  export const legend = (v: T): string => {
    if (v.problem !== null) return "Something did not answer, so this may not be everything."
    const steps = splitSetup(v.rows).setup.length
    return waitingSentence(owed(v) - steps, steps)
  }

  // Nothing to show yet while the box is still asked for its folds: the frame's placeholder stays meanwhile.
  export const asking = (v: T): boolean => v.problem === null && v.box === "Asking" && owed(v) === 0

  // Emptiness is said only when every source answered: a failed or unanswered one never reads as nothing waiting.
  const nothing = (v: T): Child => (v.problem !== null || v.box === "Asking" ? null : empty.note(EMPTY))

  // The setup cards, folded into one row after the work and opened in place; Settings is where setup lives.
  const setupRow = (rows: readonly Row[], v: T, fx: Fx): HTMLElement | null =>
    rows.length === 0 ? null
    : disclosure.details(
        { get: v.expanded.has(inboxState.SETUP), set: open => fx.dispatch({ type: "Expanded", key: inboxState.SETUP, open }) },
        { class: "setupgroup", "data-setup": String(rows.length) },
        el("span", null, el("span", { class: "setuplabel" }, "Setup"), ` · ${rows.length} thing${rows.length === 1 ? "" : "s"} to set up`),
        rows.map(r => card(r, v, fx)),
        el("a", { class: "setupsettings", href: hrefOf({ type: "settings" }) }, "Open Settings"))

  export const view = (v: T, fx: Fx): Child => {
    const { work, setup } = splitSetup(v.rows)
    return [
      v.problem === null ? null : el("p", { class: "problem" }, v.problem),
      // Whether the box has answered for its folds yet: what a reader waiting on them waits for.
      el("div", { class: "proposals", "data-box": v.box },
        owed(v) === 0 ? nothing(v) : [work.map(r => card(r, v, fx)), setupRow(setup, v, fx)],
        unplacedSection(v.unplaced),
        unreadableSection(v.unreadable),
        settledSection(v.settled, v.settledOpen, fx)),
    ]
  }

  // What waits, read from the replicas and the box's last answer: read again when the replicas move, when a pull
  // brings another chain something that reprices them, and when the box answers again.
  type Read = Pick<T, "rows" | "unplaced" | "settled" | "unreadable" | "problem" | "box">
  export const waits: reading.Reading<Read> = reading.againOn(
    reading.againOn(
      reading.againOn(
        asked.question(async (): Promise<Read> => {
          const { all, unplaced, problems, box, memory } = await gather()
          const rows = await resolveRows(waiting.open(all), memory)
          return {
            rows,
            unplaced,
            settled: waiting.settledOn(all, time.date(app.now().zone), app.now().zone),
            unreadable: all.filter(w => waiting.unreadable(w) !== null),
            problem: problems.length === 0 ? null : problems.join(" "),
            box,
          }
        }),
        storesync.watch,
      ),
      watchOthers,
    ),
    answers.hear,
  )

  export const page = (open: string | undefined): frame.Page<T, inboxState.Event> => ({
    className: "inboxpage",
    title: "Inbox",
    masthead: "destination",
    legend,
    shape: placeholder.CARDS,
    waits: asking,
    reads: () =>
      reading.map(reading.both(waits, onScreen.titles), ([r, t]): T => inboxState.opened({
        ...r,
        titles: t,
        expanded: new Set(),
        settledOpen: false,
        opening: open === undefined ? null : { key: open, stage: "Pending" },
        me: app.actor(app.now()),
        writes: app.writes(app.now()),
        intents: intentsync.held(),
      })),
    step: inboxState.step,
    keeps: inboxState.keeps,
    listen: dispatch => {
      const unhear = app.hear(
        a => ({ me: app.actor(a), writes: app.writes(a) }),
        w => dispatch({ type: "Writes", ...w }),
        (w, was) => w.me === was.me && w.writes === was.writes,
      )
      const unask = intentsync.watch(() => dispatch({ type: "Asked", intents: intentsync.held() }))
      void pullServed()
      return () => (unhear(), unask())
    },
    // A settle moved the box: what the tab holds is read at once, and again with the box's new answer.
    view: (v, dispatch, again) => view(v, { dispatch, again: () => (again(), answers.again()) }),
  })

  export const mount = (root: HTMLElement, open?: string) => frame.mount(root, page(open))
}
export default inboxPage
