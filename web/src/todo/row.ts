// A node of the list as one line, a group as a sheet: what the Todos page draws, and what its keys mean.

import { actButton } from "./acts"
import type { Acted } from "./acts"
import type { Entry } from "../wire/generated/wire"
import { describeConflicts } from "./conflicts"
import { el } from "../view/dom"
import { chip, percent } from "../view/format"
import pie from "../view/pie"
import grouping from "./grouping"
import { chevronIcon } from "../view/icons"
import { richOrTextInline } from "../rich"
import tree from "../algebra/tree"
import type { Unsent } from "../prodrome/replica"
import { applicable, headline, isOpen as stillOpen, preview, signature } from "./todos"

export namespace row {
  type Node = grouping.Node
  type Worth = grouping.Worth
  const { place, worth } = grouping

  export type Folds = Readonly<Record<string, boolean>>

  // A fold nobody has touched is open exactly when its value is not Fine.
  export const isOpen = (folds: Folds, key: string, w: Worth): boolean =>
    folds[key] ?? grouping.bands.fold(worth.state(w), { Problem: true, Watch: true, Fine: false, Unpriced: false })

  const todos = (t: tree.Tree<Node>): string[] => [...new Set(tree.nodes(t).flatMap(n => place.todo(n.place) ?? []))]

  export const count = (t: tree.Tree<Node>): number => todos(t).length

  // The most urgent todo below, the one a folded card keeps in sight.
  export const peek = (t: tree.Tree<Node>): Node | null =>
    tree
      .nodes(t)
      .filter(n => place.todo(n.place) !== null)
      .reduce<Node | null>((a, n) => (a === null || worth.urgency(n.worth) < worth.urgency(a.worth) ? n : a), null)

  // How many todos are in Problem, each once however many places it sits.
  export const attention = (t: tree.Tree<Node>): number =>
    new Set(tree.nodes(t).flatMap(n => (worth.state(n.worth).type === "Problem" ? (place.todo(n.place) ?? []) : []))).size

  export const label = (p: grouping.Place): string =>
    p.type === "All"
      ? "Everything"
      : grouping.segment.fold(p.at, { Category: n => (n === "" ? "uncategorised" : n), Band: b => b.type, Todo: t => t })

  export type Card = {
    key: string
    label: string
    node: Node
    open: boolean
    count: number
    peek: Node | null
    lines: tree.Line<Node>[]
  }

  // The root's groups, or the root itself when nothing groups it.
  export const cards = (root: tree.Tree<Node>, by: grouping.Grouping, folds: Folds): Card[] => {
    const card = (t: tree.Tree<Node>): Card => {
      const key = place.key(t.v.place)
      const open = isOpen(folds, key, t.v.worth)
      return {
        key,
        label: label(t.v.place),
        node: t.v,
        open,
        count: count(t),
        peek: peek(t),
        lines: open ? tree.visible(n => place.key(n.place), n => isOpen(folds, place.key(n.place), n.worth), t, key) : [],
      }
    }
    return by === "flat" ? (root.kids.length === 0 ? [] : [card(root)]) : root.kids.map(card)
  }

  // Where the cursor can stand: each card's head, then its visible lines.
  export type Stop = { type: "Card"; key: string; card: Card } | { type: "Line"; key: string; line: tree.Line<Node> }

  export const stops = (cs: readonly Card[]): Stop[] =>
    cs.flatMap((c): Stop[] => [{ type: "Card", key: c.key, card: c }, ...c.lines.map((l): Stop => ({ type: "Line", key: l.key, line: l }))])

  // What a key does where the cursor stands.
  export type Move =
    | { type: "Fold"; key: string; open: boolean }
    | { type: "Open"; todo: string }
    | { type: "Close" }
    | { type: "Go"; todo: string }
    | { type: "Cursor"; key: string }
    | { type: "Stay" }

  const foldKey = (l: tree.Line<Node>): string => place.key(l.v.place)
  const todoOf = (l: tree.Line<Node>): string => place.todo(l.v.place) ?? ""

  // l / Enter: unfold, then open in place (a pane on a wide screen), then go to the todo.
  export const onOpen = (s: Stop, folds: Folds, opened: string, wide: boolean): Move =>
    s.type === "Card"
      ? s.card.open
        ? { type: "Stay" }
        : { type: "Fold", key: s.key, open: true }
      : s.line.sub.kids.length > 0 && !isOpen(folds, foldKey(s.line), s.line.v.worth)
        ? { type: "Fold", key: foldKey(s.line), open: true }
        : wide || opened === todoOf(s.line)
          ? { type: "Go", todo: todoOf(s.line) }
          : { type: "Open", todo: todoOf(s.line) }

  // h: close what is open in place, then fold, then climb to the parent.
  export const onClose = (s: Stop, folds: Folds, opened: string): Move =>
    s.type === "Card"
      ? s.card.open
        ? { type: "Fold", key: s.key, open: false }
        : { type: "Stay" }
      : opened === todoOf(s.line)
        ? { type: "Close" }
        : s.line.sub.kids.length > 0 && isOpen(folds, foldKey(s.line), s.line.v.worth)
          ? { type: "Fold", key: foldKey(s.line), open: false }
          : { type: "Cursor", key: s.line.parent }

  // --- the views ---------------------------------------------------------------

  // A node's fulfillment: its pie, with its number on hover and in the pie's label; "unpriced" and no pie for ∅;
  // "not yet priced" for a ∅ that waits on a chain this device does not hold; or a blank of the pie's size saying
  // why its term does not link.
  export const markOf = (n: Node): Element =>
    worth.fold<Element>(n.worth, {
      Value: v => el("span", { class: "mark", title: percent(v) }, pie.fulfilled(v)),
      Absent: () => el("span", { class: "mark" }, chip("unpriced")),
      NotYet: chain => el("span", { class: "mark", title: `it refers into ${chain}, which this device does not hold yet` }, chip("not yet priced")),
      Unlinked: why =>
        el("span", { class: "mark unlinked", role: "img", title: why, "aria-label": "no value", style: `width:${pie.SIZE}px;height:${pie.SIZE}px` }),
    })

  export type Fx = {
    readonly fold: (key: string, open: boolean) => void
    readonly tap: (todo: string, key: string) => void
    readonly under: (todo: string) => void
    readonly acted: Acted
    readonly ask: (todo: string) => string
    readonly href: (todo: string) => string
  }

  // An unconfirmed line is its words, its markup taken out and nothing rendered; the chip says why.
  const unconfirmed = (): HTMLElement => {
    const c = chip("unconfirmed", "prov")
    c.title = "Not confirmed yet. The list shows its words; open it to read what was written and confirm it."
    return c
  }

  const title = (e: Entry | undefined, todo: string): Element =>
    e === undefined
      ? el("span", { class: "rowtitle" }, todo)
      : richOrTextInline(e.authored?.rich.body ?? null, headline(e), "rowtitle")

  export const headSignature = (c: Card): string => JSON.stringify([c.label, c.node.worth, c.count, c.open])

  // Chevron, label, the composed value's pie, the count: one 44px button over the group's rule.
  export const head = (c: Card, fx: Fx): HTMLElement =>
    el(
      "button",
      {
        type: "button",
        class: "grouphead",
        "data-key": c.key,
        "aria-expanded": c.open ? "true" : "false",
        onclick: () => fx.fold(c.key, !c.open),
      },
      chevronIcon(),
      el("span", { class: "grouplabel" }, c.label),
      markOf(c.node),
      el("span", { class: "groupcount" }, `${c.count} open`),
    )

  // A folded card's one line under its rule: the most urgent todo's pie and title, so nothing hides.
  export const peekSignature = (p: Node, e: Entry | undefined): string =>
    JSON.stringify([place.key(p.place), p.worth, e === undefined ? null : signature(e)])

  export const peekLine = (p: Node, e: Entry | undefined): HTMLElement => {
    const id = place.todo(p.place) ?? ""
    return el("div", { class: "peek", "data-todo": id }, markOf(p), title(e, id))
  }

  // Where a line's last act stands: taken by the box, or one this tab holds that the box has not.
  export type Act = "Taken" | Unsent
  // What a line draws besides its node: whether its subtodos show, and where its last act stands.
  export type Shown = { open: boolean; act: Act }

  // What a line whose last act the box has not taken says: what happened, as this tab reads it, and what to do.
  export const said = (a: Unsent, e: Entry | undefined): string =>
    a === "Refused" ? "refused: open it"
    : a === "AwaitingTrust" ? "waits for this device to be trusted"
    : e?.state === "completed" ? "done · sending" : e?.state === "cancelled" ? "cancelled · sending" : "sending"

  export const lineSignature = (l: tree.Line<Node>, e: Entry | undefined, s: Shown): string =>
    JSON.stringify([e === undefined ? null : signature(e), l.key, l.depth, l.v.worth, count(l.sub), s])

  export const line = (l: tree.Line<Node>, e: Entry | undefined, s: Shown, fx: Fx): HTMLElement => {
    const id = place.todo(l.v.place) ?? ""
    const below = count(l.sub) - 1
    const fold = place.key(l.v.place)
    return el(
      "div",
      {
        class: ["row", below > 0 ? "parent" : "", s.act === "Taken" ? "" : `pending act-${s.act}`].filter(c => c !== "").join(" "),
        style: `--depth:${l.depth}`,
        "data-todo": id,
        "data-key": l.key,
        "data-worth": worth.toNumber(l.v.worth),
      },
      l.sub.kids.length > 0
        ? el(
            "button",
            {
              type: "button",
              class: "rowfold",
              "aria-expanded": s.open ? "true" : "false",
              "aria-label": `${s.open ? "fold" : "unfold"} ${id}`,
              onclick: () => fx.fold(fold, !s.open),
            },
            chevronIcon(),
          )
        : el("span", { class: "rowgap" }),
      el(
        "button",
        { type: "button", class: "rowmain", "data-key": l.key, "aria-expanded": "false", onclick: () => fx.tap(id, l.key) },
        markOf(l.v),
        title(e, id),
        e?.unconfirmed === true ? unconfirmed() : null,
        e !== undefined && describeConflicts(e).length > 0 ? chip("conflict", "prov") : null,
        s.act === "Taken" ? null : chip(said(s.act, e), "queuedmark"),
        below > 0 ? el("span", { class: "rowcount", title: `${below} below` }, String(below)) : null,
      ),
    )
  }

  // Which line is open in place and which fills the pane: marks on a kept node, so a tap never rebuilds what it tapped.
  export type Marks = { opened: boolean; selected: boolean }

  export const mark = (n: HTMLElement, m: Marks): void => {
    n.classList.toggle("inplace", m.opened)
    n.classList.toggle("selected", m.selected)
    if (m.selected) n.setAttribute("aria-current", "true")
    else n.removeAttribute("aria-current")
    n.querySelector(".rowmain")?.setAttribute("aria-expanded", m.opened ? "true" : "false")
  }

  export const panelSignature = (e: Entry, depth: number, mayAct: boolean): string => JSON.stringify([signature(e), depth, mayAct])

  // A line opened in place: what it waits on, then Open, Done, Ask and Add a subtodo; its title is the line above it.
  export const panel = (e: Entry, depth: number, mayAct: boolean, fx: Fx): HTMLElement => {
    const id = e.todo
    const waiting = e.authored?.waiting_on ?? ""
    return el(
      "div",
      { class: "rowopen", style: `--depth:${depth}`, "data-todo": id },
      waiting === ""
        ? null
        : el(
            "p",
            { class: "waiting" },
            el("span", { class: "waitlabel" }, "waiting on "),
            richOrTextInline(e.authored?.rich.waiting_on ?? null, preview(waiting), "waitingtext"),
          ),
      el(
        "div",
        { class: "rowacts" },
        el("a", { class: "capsule primary", href: fx.href(id) }, "Open"),
        mayAct ? applicable(e).filter(n => n === "done" || n === "tend" || n === "reopen").map(n => actButton(n, id, fx.acted, "capsule")) : null,
        el("a", { class: "capsule", href: fx.ask(id) }, "Ask"),
        mayAct && stillOpen(e) ? el("button", { type: "button", class: "capsule addsub", onclick: () => fx.under(id) }, "Add a subtodo") : null,
      ),
    )
  }
}
export default row
