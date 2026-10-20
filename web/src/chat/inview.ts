/** The reference in view, which the Ask button and the secretary panel attach to: derived from the screen alone, never typed. */

import type { Reference } from "./refs";
import type { Screen } from "../shell/layout";

export type ViewReference =
  | { readonly type: "none" }
  | { readonly type: "one"; readonly reference: Reference };

/** `chain` is `App.chain`: a todo on `Screen` carries no chain of its own. */
export function referenceInView(screen: Screen, chain: string): ViewReference {
  switch (screen.type) {
    case "todos":
      return screen.selected === ""
        ? { type: "none" }
        : { type: "one", reference: { kind: "todo", chain, id: screen.selected } };
    case "todo":
      return { type: "one", reference: { kind: "todo", chain, id: screen.id } };
    case "message":
      return { type: "one", reference: { kind: "mail", hash: screen.id } };
    case "thread":
      return { type: "one", reference: { kind: "thread", id: screen.id } };
    case "briefing":
      return { type: "one", reference: { kind: "briefing", date: screen.date } };
    // Every other page is about the whole conversation.
    default:
      return { type: "none" };
  }
}
