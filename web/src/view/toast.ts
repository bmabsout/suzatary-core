// One line at the top for what is worth interrupting a page: the chain moved, a write was refused (in the
// server's own sentence), or an act can be undone within `undo.ts`'s window.

import { byId, el } from "./dom";
import { UNDO_WINDOW_MS, undoStillLive } from "../todo/undo";
import time from "../app/time";

const DWELL_MS = 6000;

/** One button a toast can carry, alongside its message. */
export interface ToastAction {
  readonly label: string;
  readonly onClick: () => void;
}

let clearing = 0;

export function toast(message: string, action?: ToastAction): void {
  const box = byId("toast");
  const children: (string | HTMLElement)[] = [message];
  if (action !== undefined) {
    const shownAt = time.now();
    const button = el("button", { type: "button", class: "toastact" }, action.label);
    button.addEventListener("click", () => {
      // A click that raced the timeout by a tick fires nothing.
      if (!undoStillLive(shownAt, time.now())) return;
      window.clearTimeout(clearing);
      box.hidden = true;
      action.onClick();
    });
    children.push(button);
  }
  box.replaceChildren(...children);
  box.hidden = false;
  window.clearTimeout(clearing);
  clearing = window.setTimeout(
    () => {
      box.hidden = true;
    },
    action === undefined ? DWELL_MS : UNDO_WINDOW_MS,
  );
}
