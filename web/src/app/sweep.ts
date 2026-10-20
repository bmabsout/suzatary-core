// The storage keys the ui registers replaced, swept once at start: an unsent draft moves into its register, and every
// old key goes. Nothing reads them after.

import registers from "./registers"
import type ui from "./ui"

export namespace sweep {
  export type Old = { readonly keys: () => readonly string[]; readonly get: (k: string) => string | null; readonly drop: (k: string) => void }

  const DRAFT = "suzatary.draft."
  const GONE = ["suzatary.theme", "suzatary.chain", "suzatary.secretarypanel.open", "suzatary.todos."]

  export const old = (k: string): boolean => k.startsWith(DRAFT) || GONE.some(g => (g.endsWith(".") ? k.startsWith(g) : k === g))

  export const run = (store: Old, u: ui.Ui): void => {
    for (const k of store.keys().filter(old)) {
      const text = k.startsWith(DRAFT) ? (store.get(k) ?? "") : ""
      const draft = u.cell(registers.draft, k.slice(DRAFT.length))
      if (text.trim() !== "" && draft.read().type === "Unwritten") draft.set(text)
      store.drop(k)
    }
  }

  export const local: Old = {
    keys: () => {
      try {
        return Array.from({ length: window.localStorage.length }, (_, i) => window.localStorage.key(i) ?? "")
      } catch {
        return []
      }
    },
    get: k => {
      try {
        return window.localStorage.getItem(k)
      } catch {
        return null
      }
    },
    drop: k => {
      try {
        window.localStorage.removeItem(k)
      } catch {
        // A storage that will not drop a key keeps it; nothing reads it.
      }
    },
  }
}
export default sweep
