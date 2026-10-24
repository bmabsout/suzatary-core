// Who asked for a card, in the owner's words: the tag on its head and the word a settled row is filed under.

import type { Asker } from "../wire/generated/wire"

export namespace asker {
  export const fold = <R>(a: Asker, alg: { readonly secretary: () => R; readonly triage: () => R; readonly plugin: (name: string) => R; readonly setup: () => R; readonly box: () => R }): R => {
    switch (a.kind) {
      case "secretary":
        return alg.secretary()
      case "triage":
        return alg.triage()
      case "plugin":
        return alg.plugin(a.name)
      case "setup":
        return alg.setup()
      case "box":
        return alg.box()
    }
  }

  // The secretary's cards are the ordinary ones, so they carry no tag.
  export type Tag = { readonly text: string; readonly tip: string } | null

  export const tag = (a: Asker): Tag =>
    fold<Tag>(a, {
      secretary: () => null,
      triage: () => ({ text: "Triage", tip: "The triage agent asks for this from mail anyone can send. Nothing changes until you answer." }),
      plugin: name => ({ text: `${name} plugin`, tip: `The ${name} plugin asks for this. It only proposes; nothing happens until you answer.` }),
      setup: () => ({ text: "Setup", tip: "The app asks for this itself, not the secretary: something it needs is not set up yet." }),
      box: () => ({ text: "App", tip: "The app asks for this itself, not the secretary." }),
    })

  export const named = (a: Asker): string => fold(a, { secretary: () => "Secretary", triage: () => "Triage", plugin: name => `${name} plugin`, setup: () => "Setup", box: () => "App" })

  // Only a setup card folds into the Inbox's Setup row; the app's other cards stand among the work.
  export const isSetup = (a: Asker): boolean => fold(a, { secretary: () => false, triage: () => false, plugin: () => false, setup: () => true, box: () => false })
}
export default asker
