// What the view keeps of a place while away from it, read from the page when it is left and put back when it is
// read again: the window's scroll, and the keyed node that had the keyboard.

import type routelog from "../shell/routelog"

export namespace kept {
  export const read = (): routelog.Kept => ({
    scroll: Math.round(window.scrollY),
    focus: document.activeElement?.closest("[data-key]")?.getAttribute("data-key") ?? "",
  })

  const FOCUSABLE = "a[href], button, input, textarea, select, [tabindex]"
  const node = (key: string): HTMLElement | null =>
    [...document.querySelectorAll<HTMLElement>("[data-key]")].filter(n => n.getAttribute("data-key") === key).find(n => n.matches(FOCUSABLE)) ?? null

  // A page draws once its value lands, so the place is put back over the next frames, until the page is tall
  // enough and the keyed node is there, or a later place is drawn (`still` answers false).
  export const restore = (k: routelog.Kept, still: () => boolean, frames = 30): void => {
    const step = (left: number): void => {
      if (!still()) return
      const n = k.focus === "" ? null : node(k.focus)
      const tall = document.documentElement.scrollHeight - window.innerHeight >= k.scroll
      if ((tall && (k.focus === "" || n !== null)) || left === 0) {
        n?.focus({ preventScroll: true })
        window.scrollTo(0, k.scroll)
        return
      }
      window.requestAnimationFrame(() => step(left - 1))
    }
    step(frames)
  }
}
export default kept
