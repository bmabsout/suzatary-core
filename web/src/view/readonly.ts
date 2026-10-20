// The line a tab that may not write draws where a box to write in would be; a status, so a screen reader says it.

import type app from "../app/app"
import { el } from "./dom"
import { hrefOf } from "../shell/route"

export namespace readonly {
  export const line = (r: app.Reach): HTMLElement | null =>
    r.type !== "Reads"
      ? null
      : el("p", { class: "readonly", role: "status" }, r.said, ...(r.settings ? [" ", el("a", { href: hrefOf({ type: "settings" }) }, "Open Settings")] : []))
}
export default readonly
