/** A photo or PDF on a person's turn, drawn the same in the composer and the transcript. */

import type { AttachmentRow } from "../wire/generated/wire"
import { attachmentUrl } from "../wire/api"
import { el } from "../view/dom"
import { docIcon, lockIcon } from "../view/icons"

export namespace attachment {
  // A private file is in the owner's Drive, so the box has nothing to show but its name.
  export const fold = <R>(
    f: AttachmentRow,
    onPhoto: (url: string) => R,
    onPdf: (name: string) => R,
    onPrivate: (name: string, path: string) => R,
  ): R =>
    f.private !== ""
      ? onPrivate(f.name || "private file", f.private)
      : f.media_type === "application/pdf"
        ? onPdf(f.name || "PDF")
        : onPhoto(attachmentUrl(f.id))

  const chip = (icon: SVGSVGElement, name: string, title: string): HTMLElement =>
    el("span", { class: "filechip", title }, icon, el("span", { class: "filename" }, name))

  export const view = (f: AttachmentRow): HTMLElement =>
    fold(
      f,
      url => el("img", { class: "turnphoto", src: url, alt: "photo", loading: "lazy" }),
      name => chip(docIcon(), name, name),
      (name, path) => chip(lockIcon(), name, `Kept private in your Drive at ${path}`),
    )
}
export default attachment
