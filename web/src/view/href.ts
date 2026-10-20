// Where an anchor typst emitted may lead, read as the browser reads it (`URL`, after it strips tabs, newlines and C0):
// the app's own route for `todo:`, an outbound URL of a scheme that only navigates, or nowhere.

export namespace href {
  export type Href =
    | { readonly type: "Todo"; readonly id: string }
    | { readonly type: "Outbound"; readonly href: string }
    | { readonly type: "Inert" }

  /** The schemes an outbound link may name: each navigates and none runs anything. */
  export const OUTBOUND: ReadonlySet<string> = new Set(["http:", "https:", "mailto:", "tel:"])

  const parsed = (raw: string, base: string): URL | null => {
    try {
      return new URL(raw, base)
    } catch {
      return null
    }
  }

  /** An outbound href is kept as written, since the browser reads it as `URL` just did; a bare `todo:` names nothing
   * and stays inert, as every scheme outside `OUTBOUND` does. */
  export const of = (raw: string, base: string): Href => {
    const u = parsed(raw, base)
    return u === null ? { type: "Inert" }
      : u.protocol === "todo:" ? (u.pathname === "" ? { type: "Inert" } : { type: "Todo", id: u.pathname })
      : OUTBOUND.has(u.protocol) ? { type: "Outbound", href: raw }
      : { type: "Inert" }
  }

  export const fold = <R>(h: Href, on: { Todo: (id: string) => R; Outbound: (href: string) => R; Inert: () => R }): R =>
    h.type === "Todo" ? on.Todo(h.id) : h.type === "Outbound" ? on.Outbound(h.href) : on.Inert()
}
export default href
