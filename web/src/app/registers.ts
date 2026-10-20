// Every register of the ui tier, and where its writes go, decided here once. `Tab`: this tab's memory, gone with it.
// `Device`: this browser, joined across its tabs. `Owner`: that, and it follows its writer to the owner's other
// devices, its words the owner's. The route is the one register kept elsewhere, as App's log (`shell/routelog.ts`),
// with each place's scroll and focus in its entry: a place is this tab's, never another device's.

import ui from "./ui"
import d from "../wire/decoder"
import type { Theme } from "./theme"
import type grouping from "../todo/grouping"
import type row from "../todo/row"
import type cardui from "../inbox/cardui"
import type anchor from "../chat/anchor"
import type aside from "../chat/aside"

export namespace registers {
  // A fold whose value is not a flag is dropped, never the whole set.
  const flags: d.Decoder<row.Folds> = d.map(d.rec(d.unknown), o =>
    Object.fromEntries(Object.entries(o).filter((kv): kv is [string, boolean] => typeof kv[1] === "boolean")),
  )

  // How the reader likes this screen to look: a phone in the dark and a desk in daylight differ, so per device.
  export const theme = ui.kind<Theme>({ name: "theme", tier: "Device", initial: "auto", value: d.oneOf({ auto: null, light: null, dark: null }) })
  // Which chain this browser reads when nothing names one; "" for the deployment's first.
  export const chain = ui.kind<string>({ name: "chain", tier: "Device", initial: "", value: d.str })
  // What a press left the secretary beside each wide page (`chat/aside.ts`): this tab's, by page, and never the device's.
  export const panel = ui.kind<aside.Presses>({ name: "panel", tier: "Tab", initial: {} })

  // A list's filter (its print), grouping and folded groups, per device: a phone's list is read differently.
  export const filter = ui.kind<string>({ name: "todos.filter", tier: "Device", initial: "", value: d.str })
  export const by = ui.kind<grouping.Grouping>({ name: "todos.by", tier: "Device", initial: "category", value: d.oneOf({ category: null, urgency: null, flat: null }) })
  export const folds = ui.kind<row.Folds>({
    name: "todos.folds",
    tier: "Device",
    initial: {},
    value: flags,
  })
  // The todo a narrow list has open in place, by chain: this tab's, so back to the list finds it open.
  export const opened = ui.kind<string>({ name: "todos.opened", tier: "Tab", initial: "" })
  // The category the last todo was added in, so the next starts there: per device, like the list's own choices.
  export const category = ui.kind<string>({ name: "todos.category", tier: "Device", initial: "", value: d.str })
  // Whether a list's add sheet is up, by chain: this tab's, drawn from here and never read back from the page.
  export const sheet = ui.kind<boolean>({ name: "todos.sheet", tier: "Tab", initial: false })

  // What is typed and not yet sent to the secretary, by conversation: the owner's words, so they follow the owner.
  export const draft = ui.kind<string>({ name: "draft", tier: "Owner", initial: "", value: d.str })

  // A card's step, the words typed there, and whether its details show: this tab's, by the card.
  export const card = ui.kind<cardui.T>({ name: "card", tier: "Tab", initial: { step: { type: "Idle" }, text: "", details: "Hidden" } })
  // Whether a todo page's provenance and history are opened, by todo and part: this tab's, kept across the page's redraws.
  export const told = ui.kind<boolean>({ name: "todo.told", tier: "Tab", initial: false })
  // Whether the shell's line of what this device left out is opened to its names: this tab's, kept as readings move it.
  export const leftout = ui.kind<boolean>({ name: "leftout", tier: "Tab", initial: false })
  // Where a transcript's box is held and how far it was scrolled, by where it is drawn and its conversation: this
  // tab's, so a conversation drawn again (back to it, the panel beside another page) opens where it was left.
  export const hold = ui.kind<{ readonly at: anchor.T; readonly top: number }>({ name: "transcript.hold", tier: "Tab", initial: { at: { type: "Newest" }, top: 0 } })

  // The build this device last announced, so a page started on a new one says so once, across its tabs.
  export const announced = ui.kind<string>({ name: "announced", tier: "Device", initial: "", value: d.str })
}
export default registers
