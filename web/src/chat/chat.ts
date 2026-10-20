// One conversation on screen: its source, its transcript in a scroll box, and its composer, wired once.

import type { Reference } from "./refs"
import type { Composer } from "./composer"
import { mountComposer } from "./composer"
import drafts from "./drafts"
import ui from "../app/ui"
import type { Loaded, Source } from "./source"
import { createSource, defaultDeps } from "./source"
import type { TranscriptHandle } from "./transcript"
import { mountTranscript } from "./transcript"

export namespace chat {
  export type Chat = {
    readonly source: Source
    readonly transcript: TranscriptHandle
    readonly composer: Composer
    readonly destroy: () => void
  }

  export type Options = {
    readonly id: string
    readonly about?: Reference
    readonly seeded: readonly Reference[]
    readonly focus: boolean
    readonly at?: number
    readonly scroll: Element
    // Where the transcript and the composer go, before the source starts; a page's frame places them itself.
    readonly place?: (transcript: HTMLElement, composer: HTMLElement) => void
    readonly loaded?: (state: Loaded) => void
    // The first window could not be read.
    readonly unread?: () => void
  }

  export const mount = (o: Options): Chat => {
    const source = createSource(o.id, o.about, defaultDeps, o.at)
    const transcript = mountTranscript(o.scroll, source, {
      quote: text => composer.quote(text),
      attachProposal: p => composer.attach({ kind: "proposal", conversation: o.id, index: p.index }),
      edit: question => composer.edit(question),
    })
    const composer = mountComposer({
      seeded: o.seeded,
      focus: o.focus,
      onSend: (text, references, files) => (transcript.asked(), source.say(text, references, files).then(transcript.sent)),
      draft: drafts.of(ui.tab, o.id),
      onStop: () => transcript.stop(),
      again: {
        offer: () => transcript.offer(),
        edit: (text, references, files) => source.edit(text, references, files),
      },
    })
    const unheard = source.subscribe((state, event) => {
      if (event.kind === "window" && state.kind === "loaded") o.loaded?.(state)
      if (state.kind === "failed") o.unread?.()
    })
    o.place?.(transcript.content, composer.root)
    source.start()
    return {
      source,
      transcript,
      composer,
      destroy: () => {
        unheard()
        transcript.destroy()
        composer.destroy()
        source.destroy()
      },
    }
  }
}
export default chat
