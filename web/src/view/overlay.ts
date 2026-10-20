// One overlay at a time, and one place that closes it: not a route, since "a menu was open" is no URL to send.

import { el } from "./dom";
import { capture } from "./focus";

interface Open {
  readonly node: HTMLElement;
  readonly name: string;
  /** Releases the focus trap and gives the keyboard back to whatever opened this. */
  readonly release: () => void;
  /** What the overlay heard while open, let go. */
  readonly closed: () => void;
}

let open: Open | null = null;

export function overlayOpen(): boolean {
  return open !== null;
}

/** The name of what is open, "" for nothing, so a toggle needs no handle. */
export function openOverlayName(): string {
  return open === null ? "" : open.name;
}

export function closeOverlay(): boolean {
  if (open === null) return false;
  const closing = open;
  open = null;
  closing.node.remove();
  closing.release();
  closing.closed();
  return true;
}

/** The scrim is part of the overlay rather than a sibling, so there is no pair to leave half-removed. `closed` runs
 * once, however it closes. */
export function showOverlay(name: string, label: string, body: HTMLElement, closed: () => void = () => {}): HTMLElement {
  closeOverlay();
  const box = el("div", { class: "overlaybox", role: "dialog", "aria-modal": "true", "aria-label": label }, body);
  const node = el("div", { class: "overlay" }, box);
  node.addEventListener("click", (event: Event) => {
    // The scrim, not the box: a click inside the dialog is the dialog's.
    if (event.target === node) closeOverlay();
  });
  document.body.appendChild(node);
  // After the node is in the document, or there is nothing to focus.
  open = { node, name, release: capture(box), closed };
  return box;
}
