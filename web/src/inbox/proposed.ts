// A proposed object by what it does: a merge, a content write with its changes, or a lifecycle event with its note.

import type { ProposedChange, ProposedObject } from "../wire/generated/wire"

export namespace proposed {
  export type Act = ProposedObject["act"]
  // What every act on a todo shows of it (the wire's `ProposedTodo`, flattened into each arm).
  export type Todo = Pick<Extract<ProposedObject, { act: "added" }>, "todo" | "body" | "at" | "actor" | "pending_todo">

  export const todo = (o: ProposedObject): Todo | null => (o.act === "merge" ? null : o)

  export const changes = (o: ProposedObject): readonly ProposedChange[] =>
    o.act === "added" || o.act === "edited" || o.act === "repriced" ? o.changes : []

  export const note = (o: ProposedObject): string | null =>
    o.act === "merge" || o.act === "added" || o.act === "edited" || o.act === "repriced" ? null : o.note
}
export default proposed
