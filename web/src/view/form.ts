// A form: its fields, each a kind of control holding what the owner typed, a pure step, and one view that draws it in place.

import type { Child } from "./dom"
import { el } from "./dom"
import act from "./act"
import lo from "../algebra/lo"
import { pct } from "./format"
import pie from "./pie"
import { toast } from "./toast"
import source from "../algebra/source"
import parsed from "../algebra/parsed"
import price from "../todo/price"
import time from "../app/time"
import app from "../app/app"
import editor from "../typst/editor"
import ui from "../app/ui"
import data from "../algebra/data"

export namespace form {
  export type Look = { readonly cls: string; readonly label: string; readonly hint?: string }
  export type Option = { readonly value: string; readonly label: string }

  export type Line = { readonly type: "Line"; readonly look: Look; readonly v: string }
  export type Lines = { readonly type: "Lines"; readonly look: Look; readonly rows: number; readonly v: string }
  // Typst source, edited with its highlighting, completion and a live preview.
  export type Markup = { readonly type: "Markup"; readonly look: Look; readonly rows: number; readonly v: string }
  export type Secret = { readonly type: "Secret"; readonly look: Look; readonly v: string }
  export type Day = { readonly type: "Day"; readonly look: Look; readonly min: string; readonly v: string }
  // `prompt` is what the chips say in place while none is chosen.
  export type Choice = { readonly type: "Choice"; readonly prompt: string; readonly options: readonly Option[]; readonly v: string }
  export type Toggle = { readonly type: "Toggle"; readonly look: Look; readonly v: boolean }
  export type Price = { readonly type: "Price"; readonly v: price.Form }
  export type Field = Line | Lines | Markup | Secret | Day | Choice | Toggle | Price

  export type Fields<F> = { readonly [K in keyof F]: Field }
  export type Said = { readonly type: "Quiet" } | { readonly type: "Refused"; readonly reason: string; readonly about: parsed.About }
  export type T<F extends Fields<F>> = { readonly fields: F; readonly said: Said }

  export type Event<F extends Fields<F>> =
    | { readonly [K in keyof F]: { readonly type: "Typed"; readonly name: K; readonly v: F[K]["v"] } }[keyof F]
    | { readonly type: "Refused"; readonly reason: string; readonly about: parsed.About }
    | { readonly type: "Sent" }
    | { readonly type: "Reset"; readonly to: T<F> }

  export type Values<F extends Fields<F>> = { readonly [K in keyof F]?: F[K]["v"] }

  export const quiet: Said = { type: "Quiet" }
  export const of = <F extends Fields<F>>(fields: F): T<F> => ({ fields, said: quiet })

  export const line = (look: Look, v = ""): Line => ({ type: "Line", look, v })
  export const lines = (look: Look, rows: number, v = ""): Lines => ({ type: "Lines", look, rows, v })
  export const markup = (look: Look, rows: number, v = ""): Markup => ({ type: "Markup", look, rows, v })
  export const secret = (look: Look): Secret => ({ type: "Secret", look, v: "" })
  export const day = (look: Look, min: string, v = ""): Day => ({ type: "Day", look, min, v })
  export const choice = (prompt: string, options: readonly Option[], v = ""): Choice => ({ type: "Choice", prompt, options, v })
  export const toggle = (look: Look, v = false): Toggle => ({ type: "Toggle", look, v })
  export const priced = (v: price.Form): Price => ({ type: "Price", v })

  const put = <F extends Fields<F>, K extends keyof F>(fs: F, k: K, f: F[K]): F => ({ ...fs, [k]: f })

  // What is typed, field by field, a secret's never: a secret leaves in its request and is kept nowhere else.
  export const values = <F extends Fields<F>>(fs: F): Values<F> => {
    let acc: Values<F> = {}
    for (const k in fs) if (fs[k].type !== "Secret") acc = { ...acc, [k]: fs[k].v }
    return acc
  }

  // What a form keeps of itself while it is drawn again: what is typed, a secret's never, and what was said of it.
  export type Kept<F extends Fields<F>> = { readonly values: Values<F>; readonly said: Said }
  export const kept = <F extends Fields<F>>(s: T<F>): Kept<F> => ({ values: values(s.fields), said: s.said })
  const blankKept = { values: {}, said: quiet }
  export const restore = <F extends Fields<F>>(blank: F, k: Kept<F>): T<F> => ({ fields: fill(blank, k.values), said: k.said })

  // The drafts of one kind of form, one register a row in this tab: a body drawn again mounts the form on its draft,
  // so a draw is a function of what is typed and said and never reads it out of the DOM.
  export const drafts = <F extends Fields<F>>(name: string): ui.Kind<Kept<F>> => ui.kind<Kept<F>>({ name: `draft.${name}`, tier: "Tab", initial: blankKept })

  // One row's draft of a kind of form.
  export type Draft<F extends Fields<F>> = { readonly kind: ui.Kind<Kept<F>>; readonly key: string }
  export const draft = <F extends Fields<F>>(kind: ui.Kind<Kept<F>>, key: string): Draft<F> => ({ kind, key })

  // The forms of each row still holding their draft: whether each is on the page, and how it lets go. A row with none
  // holding has no entry.
  type Holder = { readonly shown: () => boolean; readonly release: () => void }
  const mounts = new Map<string, readonly Holder[]>()

  export const fill = <F extends Fields<F>>(fs: F, vs: Values<F>): F => {
    let acc = fs
    for (const k in vs) {
      const v = vs[k]
      if (v !== undefined) acc = put(acc, k, { ...acc[k], v })
    }
    return acc
  }

  // A secret leaves in the request and nowhere else: sending clears it from the value, and so from the field.
  const unsecret = <F extends Fields<F>>(fs: F): F => {
    let acc = fs
    for (const k in fs) if (fs[k].type === "Secret" && fs[k].v !== "") acc = put(acc, k, { ...fs[k], v: "" })
    return acc
  }

  export const step = <F extends Fields<F>>(s: T<F>, e: Event<F>): T<F> =>
    e.type === "Typed" ? (Object.is(s.fields[e.name].v, e.v) ? s : { ...s, fields: put(s.fields, e.name, { ...s.fields[e.name], v: e.v }) })
    : e.type === "Refused" ? (s.said.type === "Refused" && s.said.reason === e.reason && data.same(s.said.about, e.about) ? s : { ...s, said: { type: "Refused", reason: e.reason, about: e.about } })
    : e.type === "Sent" ? { fields: unsecret(s.fields), said: quiet }
    : e.to

  // How a form reads its fields and where it sends them; `say` is where a refusal is said aloud, the toast by default.
  export type Spec<F extends Fields<F>, B> = {
    readonly read: (fields: F) => parsed.T<B>
    readonly send: (body: B, dispatch: (e: Event<F>) => void) => Promise<unknown>
    readonly refusal?: string
    readonly say?: (reason: string) => void
    // Whether what is sent leaves the fields as it is sent, so what is typed while the box answers is the next entry.
    readonly clears?: boolean
    // Where what is typed is kept while the form is drawn again: the form mounts on it, and every step writes it.
    readonly draft?: Draft<F>
  }

  export type Form<F extends Fields<F>> = source.Source<T<F>, Event<F>>

  // What a refusal leaves of a form: what was sent handed back to fields still `cleared` as the send left them, else what
  // was typed since, the reason said beside what it is about.
  const refused = <F extends Fields<F>>(now: T<F>, sent: F, cleared: F | null, reason: string, about: parsed.About): T<F> =>
    step(cleared !== null && data.same(values(now.fields), values(cleared)) ? { ...now, fields: unsecret(sent) } : now, { type: "Refused", reason, about })

  // Where a form's value is while it is drawn again: the form on the page, or the draft the one shown follows.
  export type Home<F extends Fields<F>> = { readonly get: () => T<F>; readonly set: (v: T<F>) => void }

  // A clearing form gives up what it sends as it sends it, so the box's answer, whenever it lands, takes nothing typed
  // since. A refusal, the box's or the form's own, is kept beside the button until the next submit and said once.
  export const submit = <F extends Fields<F>, B>(home: Home<F>, spec: Spec<F, B>, blank: F): Promise<act.Outcome> => {
    const sent = home.get().fields
    const cleared = spec.clears === true ? blank : null
    const refuse = (reason: string, about: parsed.About): act.Outcome =>
      (home.set(refused(home.get(), sent, cleared, reason, about)), (spec.say ?? toast)(reason), act.AGAIN)
    const read = spec.read(sent)
    if (read.type === "Refused") return Promise.resolve(refuse(read.reason, read.about))
    home.set(cleared === null ? step(home.get(), { type: "Sent" }) : of(cleared))
    return spec.send(read.v, e => home.set(step(home.get(), e))).then(() => act.AGAIN, (e: unknown) => refuse(lo.reason(e), parsed.whole))
  }

  // A control is built once; `paint` brings it to a field's value, `pull` reads what the DOM holds now.
  type Control<X extends Field> = { readonly node: HTMLElement; paint(x: X): void; pull(): X["v"] }
  type Set = (v: Field["v"]) => void

  const text = (node: HTMLInputElement | HTMLTextAreaElement, set: Set, event = "input"): Control<Line | Lines | Secret | Day> => {
    node.addEventListener(event, () => set(node.value))
    return { node, paint: x => void (node.value === x.v ? undefined : (node.value = x.v)), pull: () => node.value }
  }

  const press = (b: HTMLElement, on: boolean): void => {
    b.classList.toggle("on", on)
    b.setAttribute("aria-pressed", on ? "true" : "false")
  }

  // Tapping the lit chip clears it, and while none is lit the row says what it asks for, before anything is sent.
  const chips = (x: Choice, set: Set, now: () => string): Control<Choice> => {
    const bs = x.options.map(o => ({ o, b: el("button", { type: "button", class: "chipbtn", onclick: () => set(now() === o.value ? "" : o.value) }, o.label) }))
    const prompt = el("p", { class: "chipprompt" }, x.prompt)
    const node = el("div", { class: "chipfield" }, prompt, el("div", { class: "chiprow", role: "group", "aria-label": x.prompt }, bs.map(c => c.b)))
    const paint = (y: Choice): void => {
      bs.forEach(c => press(c.b, c.o.value === y.v))
      prompt.hidden = y.v !== ""
      node.classList.toggle("unchosen", y.v === "")
    }
    return { node, paint, pull: now }
  }

  const check = (x: Toggle, set: Set): Control<Toggle> => {
    const box = el("input", { type: "checkbox" })
    box.addEventListener("change", () => set(box.checked))
    return { node: el("label", { class: x.look.cls }, box, x.look.label), paint: y => void (box.checked = y.v), pull: () => box.checked }
  }

  const range = (label: string): HTMLInputElement =>
    el("input", { type: "range", class: "draftrange", min: 0, max: 100, step: 1, "aria-label": label })
  const days = (label: string, max: number): HTMLInputElement =>
    el("input", { type: "number", class: "draftfield draftdays", min: 1, max, step: 1, "aria-label": label })
  const row = (...kids: Child[]): HTMLElement => el("div", { class: "draftrow" }, kids)
  const meta = (s: string): HTMLElement => el("span", { class: "meta" }, s)
  const ratio = (r: HTMLInputElement): number => price.wholePercent(Number(r.value) / 100)
  const whole = (n: HTMLInputElement, otherwise: number): number => {
    const d = Number.parseInt(n.value, 10)
    return Number.isNaN(d) ? otherwise : d
  }

  // How it is priced, then only the rows that kind has.
  const pricing = (set: Set, now: () => price.Form): Control<Price> => {
    const kinds = price.KINDS.map(k => ({ k, b: el("button", { type: "button", class: "segment", onclick: () => set(price.withKind(now(), k.value)) }, k.label) }))
    const flat = range("fulfillment, as a percentage — low means urgent")
    const flatPct = el("span", { class: "draftpct" })
    // A date in the past prices like a deadline already missed, which nobody means to type.
    const date = el("input", { type: "date", class: "draftfield", min: time.date(app.now().zone), "aria-label": "deadline date" })
    const lead = days("lead-up, in days", 365)
    const span = days("how many days the cycle runs", 3650)
    const from = range("fulfillment just after a pass")
    const fromPct = el("span", { class: "draftpct" })
    const to = range("fulfillment the cycle settles at")
    const toPct = el("span", { class: "draftpct" })
    const leadOf = (p: price.Form): number => (p.type === "Deadline" ? p.leadUpDays : price.DEFAULT_LEAD_UP_DAYS)
    const cycle = (c: Partial<price.Cycle>): void => set(price.withCycle(now(), { ...price.cycleOf(now()), ...c }))
    flat.addEventListener("input", () => set(price.flat(ratio(flat))))
    date.addEventListener("input", () => set(price.deadline(date.value, leadOf(now()))))
    lead.addEventListener("input", () => set(price.deadline(now().type === "Deadline" ? date.value : "", whole(lead, price.DEFAULT_LEAD_UP_DAYS))))
    span.addEventListener("input", () => void (Number.isNaN(Number.parseInt(span.value, 10)) ? undefined : cycle({ days: whole(span, 0) })))
    from.addEventListener("input", () => cycle({ from: ratio(from) }))
    to.addEventListener("input", () => cycle({ to: ratio(to) }))
    const flatRow = row(flat, flatPct, meta("low means urgent"))
    const deadlineRow = row(date, lead, meta("days of lead-up"))
    const cycleRows = el("div", { class: "cyclefields" }, row(span, meta("days")), row(from, fromPct, meta("just after a pass")), row(to, toPct, meta("where it settles")))
    const same = (n: HTMLInputElement, v: string): void => void (n.value === v ? undefined : (n.value = v))
    const paint = (x: Price): void => {
      const p = x.v
      kinds.forEach(c => press(c.b, c.k.value === p.type))
      // A hidden row shows its kind's default, so what is drawn is a function of the value alone.
      const f = p.type === "Flat" ? p.flat : price.DEFAULT_FLAT
      const d = p.type === "Deadline" ? p : { deadline: "", leadUpDays: price.DEFAULT_LEAD_UP_DAYS }
      same(flat, String(Math.round(f * 100)))
      flatPct.replaceChildren(pie.fulfilled(f), pct(f))
      same(date, d.deadline)
      same(lead, String(d.leadUpDays))
      const c = price.cycleOf(p)
      same(span, String(c.days))
      same(from, String(Math.round(c.from * 100)))
      fromPct.replaceChildren(pie.fulfilled(c.from), pct(c.from))
      same(to, String(Math.round(c.to * 100)))
      toPct.replaceChildren(pie.fulfilled(c.to), pct(c.to))
      flatRow.hidden = p.type !== "Flat"
      deadlineRow.hidden = p.type !== "Deadline"
      cycleRows.hidden = p.type !== "Recur" && p.type !== "Periodic"
    }
    const kindRow = el("div", { class: "segmented", role: "group", "aria-label": "how this todo is priced" }, kinds.map(c => c.b))
    return { node: el("div", { class: "pricefields" }, kindRow, flatRow, deadlineRow, cycleRows), paint, pull: now }
  }

  const control = (x: Field, set: Set, now: () => Field): Control<Field> => {
    const attrs = (l: Look) => ({ class: l.cls, placeholder: l.hint ?? null, "aria-label": l.label })
    const v = <A>(pick: (f: Field) => A | undefined, otherwise: A) => (): A => pick(now()) ?? otherwise
    switch (x.type) {
      case "Line": return text(el("input", { type: "text", ...attrs(x.look) }), set)
      case "Lines": return text(el("textarea", { rows: x.rows, ...attrs(x.look) }), set)
      case "Markup": {
        const c = editor.control(x.look, x.rows, set)
        return { node: c.node, paint: y => c.paint(y.type === "Markup" ? y.v : ""), pull: c.pull }
      }
      case "Secret": return text(el("input", { type: "password", autocomplete: "off", ...attrs(x.look) }), set)
      case "Day": return text(el("input", { type: "date", min: x.min, ...attrs(x.look) }), set)
      case "Choice": return chips(x, set, v(f => (f.type === "Choice" ? f.v : undefined), ""))
      case "Toggle": return check(x, set)
      case "Price": return pricing(set, v(f => (f.type === "Price" ? f.v : undefined), price.EMPTY))
    }
  }

  // One form on screen. Every control is built once and painted from each value, so a half-typed field keeps its cursor;
  // the caller lays out its fields, the refusal lines and the button. `saidBy` is the line under one field, which says
  // the refusals about it; `said` says the rest.
  export type Mounted<F extends Fields<F>> = {
    readonly form: Form<F>
    readonly at: (name: keyof F) => HTMLElement
    readonly said: HTMLElement
    readonly saidBy: (name: keyof F) => HTMLElement
    readonly button: (label: Child, cls: string) => HTMLElement
    readonly when: (toggle: keyof F, cls: string, ...kids: Child[]) => HTMLElement
  }

  export const mount = <F extends Fields<F>, B>(blank: T<F>, spec: Spec<F, B>): Mounted<F> => {
    const draft = spec.draft === undefined ? undefined : ui.cell(spec.draft.kind, spec.draft.key)
    const initial = draft === undefined ? blank : restore(blank.fields, draft.get())
    const s = source.create<T<F>, Event<F>>(initial, step)
    const paints: ((v: T<F>, was: T<F> | null) => void)[] = []
    const pulls: (() => void)[] = []
    const built = new Map<keyof F, HTMLElement>()
    for (const name in initial.fields) {
      // A control is built for this field's kind, so what it sets is this field's value.
      const typed = (v: Field["v"]): void => s.dispatch({ type: "Typed", name, v } as Event<F>)
      const c = control(initial.fields[name], typed, () => s.state().fields[name])
      // Only a field whose value moved is painted, so a field the DOM changed alone keeps what it holds until it is pulled.
      paints.push((v, was) => (was !== null && was.fields[name] === v.fields[name] ? undefined : c.paint(v.fields[name])))
      // What autofill or a script set without an input event is still what the owner sees, so it is what is sent.
      pulls.push(() => typed(c.pull()))
      built.set(name, c.node)
    }
    // Each line says the refusal whose subject it is: a field with a line of its own, or else the form.
    const byField = new Map<string, HTMLElement>()
    const refusalLine = (): HTMLElement => el("p", { class: `formrefusal ${spec.refusal ?? ""}`.trim(), role: "alert", hidden: true })
    const said = refusalLine()
    const subject = (v: T<F>): HTMLElement | null =>
      v.said.type === "Quiet" ? null : v.said.about.type === "Field" ? (byField.get(v.said.about.name) ?? said) : said
    const sayAll = (v: T<F>): void => {
      const at = subject(v)
      for (const l of [said, ...byField.values()]) {
        l.hidden = l !== at
        l.textContent = l === at && v.said.type === "Refused" ? v.said.reason : ""
      }
    }
    paints.push(sayAll)
    let drawn: T<F> | null = null
    const paint = (v: T<F>): void => (paints.forEach(p => p(v, drawn)), void (drawn = v))
    paint(initial)
    s.subscribe(paint)
    // Only a form on the page writes its draft or follows it: what is typed is typed into one, so a form a later draw
    // replaced, or one drawn and not yet placed, never writes its own idea of the fields back.
    const shown = (): boolean => [...built.values()].some(n => n.isConnected)
    if (draft !== undefined && spec.draft !== undefined) {
      const row = `${spec.draft.kind.name}\u0000${spec.draft.key}`
      const holder: Holder = {
        shown,
        release: () => {
          heard?.()
          heard = null
          const rest = (mounts.get(row) ?? []).filter(h => h !== holder)
          if (rest.length === 0) mounts.delete(row)
          else mounts.set(row, rest)
        },
      }
      // A form drawn again while another sent shares its draft: what the box says of the send reaches this one. A change
      // heard off the page lets go, whether the form was shown and left or never placed.
      const follow = (): void => {
        if (!shown()) return holder.release()
        if (!data.same(draft.get(), kept(s.state()))) s.dispatch({ type: "Reset", to: restore(blank.fields, draft.get()) })
      }
      let heard: (() => void) | null = null
      const hold = (): void => {
        if (heard !== null) return
        heard = draft.hear(follow)
        mounts.set(row, [...(mounts.get(row) ?? []), holder])
      }
      // What a form on the page holds is its draft, so a form put back on the page after it let go holds it again.
      s.subscribe(v => {
        if (!shown()) return
        hold()
        draft.set(kept(v))
      })
      // A row mounted again lets every older form of it off the page go, so a visit away and back leaves none behind.
      for (const h of mounts.get(row) ?? []) if (!h.shown()) h.release()
      hold()
    }
    // A form on the page holds its value; one a later draw replaced hands the box's answer to the draft, which the form
    // shown follows.
    const home: Home<F> = {
      get: () => (draft === undefined || shown() ? s.state() : restore(blank.fields, draft.get())),
      set: v => (draft === undefined || shown() ? s.dispatch({ type: "Reset", to: v }) : draft.set(kept(v))),
    }
    const run = (): Promise<act.Outcome> => (pulls.forEach(p => p()), submit(home, spec, blank.fields))
    return {
      form: s,
      at: name => built.get(name) ?? el("span"),
      said,
      saidBy: name => {
        const l = byField.get(String(name)) ?? refusalLine()
        byField.set(String(name), l)
        sayAll(s.state())
        return l
      },
      button: (label, cls) => act.view({ label, cls, run }),
      when: (name, cls, ...kids) => {
        const group = el("div", { class: cls }, kids)
        const show = (v: T<F>): void => void (group.hidden = v.fields[name]?.v !== true)
        paints.push(show)
        show(s.state())
        return group
      },
    }
  }
}
export default form
