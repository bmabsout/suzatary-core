// The browser's key-value storage: every access may throw, so a read is then absent and a write is lost, never the page.

export namespace kv {
  export type Store = {
    readonly getItem: (key: string) => string | null
    readonly setItem: (key: string, value: string) => void
    readonly removeItem: (key: string) => void
  }
  export type Kv = {
    readonly get: (key: string) => string | null
    readonly set: (key: string, value: string) => void
    readonly drop: (key: string) => void
  }

  const quiet = <A>(f: () => A, otherwise: A): A => {
    try {
      return f()
    } catch {
      return otherwise
    }
  }

  export const over = (store: () => Store): Kv => ({
    get: key => quiet(() => store().getItem(key), null),
    set: (key, value) => quiet(() => store().setItem(key, value), undefined),
    drop: key => quiet(() => store().removeItem(key), undefined),
  })

  // What comes back is input from outside the program: every caller parses it into a value of its own type.
  export const local: Kv = over(() => window.localStorage)
}
export default kv
