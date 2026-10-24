// The watches on mail the owner expects, each with Cancel; the secretary sets them, the sync rings them.

import type { MailWatch } from "../wire/generated/wire"
import { cancelWatch, mailWatches } from "../wire/api"
import app from "../app/app"
import asked from "../app/asked"
import act from "../view/act"
import { el } from "../view/dom"
import { whenOf } from "../view/format"

export namespace watches {
  export const listed = asked.question(() => mailWatches().then(p => p.watches))

  const what = (w: MailWatch): string => (w.on === "sender" ? `mail from “${w.text}”` : `a subject with “${w.text}”`)

  const cancel = (w: MailWatch, again: () => void): act.T => ({
    label: "Cancel",
    cls: "act act-cancel",
    aria: `Stop watching for ${what(w)}`,
    run: async () => {
      await cancelWatch(w.id)
      again()
      return act.SPENT
    },
  })

  const row = (w: MailWatch, again: () => void): HTMLElement =>
    el(
      "li",
      { class: "watchrow", "data-watch": w.id },
      el("span", { class: "watchwhat" }, `Watching for ${what(w)}, until ${whenOf(w.until, app.now().zone)}`),
      ...act.offered<Node[]>(act.gate(), () => [act.view(cancel(w, again))], []),
    )

  export const view = (ws: readonly MailWatch[], again: () => void): HTMLElement | null =>
    ws.length === 0 ? null : el("ul", { class: "watchlist", "aria-label": "Watches on expected mail" }, ws.map(w => row(w, again)))
}
export default watches
