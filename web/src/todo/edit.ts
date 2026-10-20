// A todo's edit as a form, seeded from what the chain holds, read as an `UpdateTodo` naming only what moved:
// an absent field is kept, so a page that sent everything would overwrite what moved on the chain meanwhile.
// An edit that moves nothing is still sent, since the box's refusal is the one a reader should see.

import type { Authored, UpdateTodo } from "../wire/generated/wire"
import form from "../view/form"
import parsed from "../algebra/parsed"
import price from "./price"
import recur from "./recur"

export type Edit = {
  readonly body: form.Markup
  readonly detail: form.Markup
  readonly category: form.Choice
  readonly waiting: form.Line
  // A price needs a rationale to move, so its controls stay closed until asked for.
  readonly reprice: form.Toggle
  readonly price: form.Price
  readonly rationale: form.Line
  readonly note: form.Line
}

// The text each markup field opens with (`edit.opened`): what a diff is measured against, so opening and saving
// unchanged sends nothing.
export type Opening = { readonly body: string; readonly detail: string }

export const stored = (a: Authored): Opening => ({ body: a.body, detail: a.detail })

// A recurring todo opens with its cycle, read back off the term the box authored.
// The deployment's categories are the choices, with the todo's own kept where the vocabulary lost it.
export const formOf = (a: Authored, categories: readonly string[], o: Opening = stored(a)): Edit => ({
  body: form.markup({ cls: "draftfield draftbody", label: "the todo's action line, in Typst markup" }, 2, o.body),
  detail: form.markup({ cls: "draftfield", label: "detail, in Typst markup — empty clears it" }, 3, o.detail),
  category: form.choice(
    "Pick a category",
    (a.category === "" || categories.includes(a.category) ? categories : [...categories, a.category]).map(c => ({ value: c, label: c })),
    a.category,
  ),
  waiting: form.line({ cls: "draftfield", label: "what it is blocked on — empty clears it" }, a.waiting_on),
  reprice: form.toggle({ cls: "editrepricelabel", label: " change the priority" }),
  price: form.priced((a.spec === null ? null : recur.priceOf(a.spec)) ?? price.EMPTY),
  rationale: form.line({ cls: "draftfield", label: "why the new priority", hint: "why this number" }),
  note: form.line({ cls: "noteline", label: "Why, or what next", hint: "Why, or what next" }),
})

export const diffOf = (todo: string, a: Authored, f: Edit, o: Opening = stored(a)): parsed.T<UpdateTodo> => {
  const body = f.body.v.trim()
  const detail = f.detail.v.trim()
  const waiting = f.waiting.v.trim()
  if (body !== o.body && body === "") return parsed.refused("The action line cannot be empty.")
  const kept: UpdateTodo = {
    todo,
    ...(body === o.body.trim() ? {} : { body }),
    ...(detail === o.detail.trim() ? {} : { detail }),
    ...(f.category.v === a.category ? {} : { category: f.category.v }),
    ...(waiting === a.waiting_on ? {} : waiting === "" ? { clear_waiting_on: true } : { waiting_on: waiting }),
  }
  const rationale = f.rationale.v.trim()
  return !f.reprice.v ? parsed.ok(kept)
    : parsed.bind(price.of(f.price.v), priority =>
        rationale === "" ? parsed.refused("Say why the priority is changing.") : parsed.ok({ ...kept, priority, rationale }))
}
