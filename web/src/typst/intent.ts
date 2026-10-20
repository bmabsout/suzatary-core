// What a control in a compiled view may ask: a closed vocabulary, parsed at the one listener; anything else is refused.

import type { Reference } from "../chat/refs"
import { parse, print } from "../chat/refs"

type Todo = Extract<Reference, { readonly kind: "todo" }>

const todoOf = (text: string): Todo | null => {
  const r = parse(text)
  return r !== null && r.kind === "todo" ? r : null
}

const printTodo = (t: Todo): string => print(t)

export namespace intent {
  export type Intent =
    | { readonly type: "Open"; readonly todo: Todo }
    | { readonly type: "Complete"; readonly todo: Todo }

  // Keyed by every member, so a member the sum gains and this table lacks does not compile.
  const VERBS: { readonly [K in Intent["type"]]: string } = { Open: "open", Complete: "complete" }
  const TYPES: Readonly<Record<string, Intent["type"]>> = { open: "Open", complete: "Complete" }

  export const fold = <R>(i: Intent, on: { readonly [K in Intent["type"]]: (todo: Todo) => R }): R => on[i.type](i.todo)

  // The kit's `intent(verb, ref)`: a verb, one space, a todo's reference.
  export const print = (i: Intent): string => `${VERBS[i.type]} ${printTodo(i.todo)}`

  export const parse = (text: string): Intent | null => {
    const cut = text.indexOf(" ")
    const verb = text.slice(0, cut)
    const todo = cut === -1 ? null : todoOf(text.slice(cut + 1))
    const type = Object.hasOwn(TYPES, verb) ? TYPES[verb] : undefined
    return todo === null || type === undefined ? null : { type, todo }
  }

  // The one listener a compiled view gets: a click on an element carrying `data-intent` is parsed, never run.
  export const listen = (root: HTMLElement, on: { readonly heard: (i: Intent) => void; readonly refused: (text: string) => void }): (() => void) => {
    const click = (e: MouseEvent): void => {
      const at = e.target instanceof Element ? e.target.closest("[data-intent]") : null
      if (at === null || !root.contains(at)) return
      const text = at.getAttribute("data-intent") ?? ""
      const i = parse(text)
      if (i === null) on.refused(text)
      else on.heard(i)
    }
    root.addEventListener("click", click)
    return () => root.removeEventListener("click", click)
  }
}
export default intent
