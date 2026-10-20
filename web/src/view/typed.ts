// What a person is typing belongs to the DOM until it is sent, not to the value a page draws: a body drawn again
// carries each draft into its counterpart, named by its rows and what the field is, with the keyboard and the
// selection where they were.

export namespace typed {
  type Field = HTMLInputElement | HTMLTextAreaElement

  // Only a draft is the DOM's: text a person is typing. A box, a choice or a pick is a page's value, drawn from it.
  const TEXTS: ReadonlySet<string> = new Set(["text", "search", "email", "url", "tel", "password", "number"])
  const FIELDS = "input, textarea"

  const isField = (n: Element): n is Field => n instanceof HTMLTextAreaElement || (n instanceof HTMLInputElement && TEXTS.has(n.type))

  // The rows a field stands in, by their keys: a field's identity is its row's, never its place among its siblings,
  // so a reading that adds or reorders rows carries each field into its own row.
  const rows = (f: Field, root: Element): string => {
    const keys: string[] = []
    for (let n = f.closest("[data-key]"); n !== null && root.contains(n); n = n.parentElement?.closest("[data-key]") ?? null) keys.push(n.getAttribute("data-key") ?? "")
    return keys.join("/")
  }

  // The same field across two drawings: its rows, then what it is; the n-th of those where several in one row share it.
  const named = (f: Field, root: Element): string => `${rows(f, root)} ${f.tagName} ${f.type} ${f.className} ${f.getAttribute("aria-label") ?? f.getAttribute("name") ?? ""}`

  const fields = (roots: readonly Node[]): Map<string, Field[]> => {
    const by = new Map<string, Field[]>()
    for (const r of roots) {
      if (!(r instanceof Element)) continue
      for (const f of [...(isField(r) ? [r] : []), ...r.querySelectorAll(FIELDS)].filter(isField)) by.set(named(f, r), [...(by.get(named(f, r)) ?? []), f])
    }
    return by
  }

  // Before `to` replaces `from`'s children: a draft typed into a field of `from` goes into its counterpart in `to`
  // when the page drew that counterpart as it drew the field (so the page did not move it), and the keyboard follows.
  // Nothing is dispatched: drawing runs no handler. Whether any draft was found.
  export const carry = (from: Element, to: readonly Node[]): boolean => {
    if (typeof HTMLElement === "undefined") return false
    const was = fields([from])
    const now = fields(to)
    const active = document.activeElement
    let any = false
    for (const [name, fs] of was) {
      fs.forEach((f, i) => {
        const typed = f.value !== f.defaultValue
        any ||= typed
        const g = now.get(name)?.[i]
        if (g === undefined || g === f) return
        if (typed && g.defaultValue === f.defaultValue && g.value === g.defaultValue) g.value = f.value
        if (f === active) focusLater(g, f)
      })
    }
    return any
  }

  // A body's carrier: nothing is looked for until a person types in it or holds the keyboard in it (a field focused and
  // not yet typed into keeps the keyboard), and once nothing typed is left to carry.
  export const keeper = (body: Element): ((to: readonly Node[]) => void) => {
    if (typeof HTMLElement === "undefined") return () => {}
    let touched = false
    body.addEventListener("input", () => void (touched = true))
    return to => {
      const focused = document.activeElement !== null && document.activeElement !== body && body.contains(document.activeElement)
      if (touched || focused) touched = carry(body, to)
    }
  }

  // The new field is in the page only once the frame places it, so the keyboard follows on the next task.
  const focusLater = (g: Field, f: Field): void => {
    const [start, end] = [selection(f, "start"), selection(f, "end")]
    queueMicrotask(() => {
      if (!g.isConnected) return
      g.focus({ preventScroll: true })
      if (start !== null && end !== null) select(g, start, end)
    })
  }

  // Some inputs (a number) have no selection, and asking throws.
  const selection = (f: Field, at: "start" | "end"): number | null => {
    try {
      return at === "start" ? f.selectionStart : f.selectionEnd
    } catch {
      return null
    }
  }

  const select = (g: Field, start: number, end: number): void => {
    try {
      g.setSelectionRange(start, end)
    } catch {
      return
    }
  }
}
export default typed
