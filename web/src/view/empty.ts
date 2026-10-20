// What a list says when it holds nothing, in its own words: an empty chain and an empty filter are different news.

import { el } from "./dom"

export namespace empty {
  export const note = (said: string): HTMLElement => el("p", { class: "empty" }, said)
}
export default empty
