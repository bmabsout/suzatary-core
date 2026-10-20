// A new todo in the add sheet's form: its fields, and what they read to, a `NewTodo` or the sentence to show beside the
// field it is about. The price is typed by the owner; nothing here invents a number.

import type { NewTodo } from "../wire/generated/wire"
import form from "../view/form"
import parsed from "../algebra/parsed"
import price from "./price"

export type Draft = {
  readonly body: form.Markup
  readonly category: form.Choice
  readonly price: form.Price
  readonly rationale: form.Line
}

// The deployment's categories are the choices, in its order, with `chosen` lit.
export const emptyDraft = (categories: readonly string[], chosen = ""): Draft => ({
  body: form.markup({ cls: "draftfield draftbody", label: "the todo's action line, in Typst markup", hint: "What needs doing?" }, 2),
  category: form.choice("Pick a category", categories.map(c => ({ value: c, label: c })), chosen),
  price: form.priced(price.EMPTY),
  rationale: form.line({ cls: "draftfield", label: "why this priority", hint: "why this number (optional)" }),
})

// The category a new todo starts in: the one the list is narrowed to, else the last one a todo was added in, else none,
// and never one the deployment no longer has.
export const preselect = (categories: readonly string[], narrowed: string, last: string): string =>
  [narrowed, last].find(c => c !== "" && categories.includes(c)) ?? ""

export const draft = (f: Draft): parsed.T<NewTodo> =>
  f.body.v.trim() === "" ? parsed.refused("Say what the todo is.", parsed.field("body"))
  : f.category.v === "" ? parsed.refused("Pick a category.", parsed.field("category"))
  : parsed.map(parsed.of("price", price.of(f.price.v)), priority => ({ body: f.body.v.trim(), category: f.category.v, priority, rationale: f.rationale.v.trim() }))
