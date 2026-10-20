// Where the focus is and where it goes back to: the keyboard is never left somewhere the eye is not.

const FOCUSABLE = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "textarea:not([disabled])",
  "select:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(", ");

/** The focusable descendants in tab order, skipping hidden ones (`offsetParent` is null). */
export function focusables(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (node) => node.offsetParent !== null || node === document.activeElement,
  );
}

/** Take the keyboard into `root` and keep Tab inside it; the returned function hands it back. */
export function capture(root: HTMLElement): () => void {
  const before = document.activeElement instanceof HTMLElement ? document.activeElement : null;

  const onKey = (event: KeyboardEvent): void => {
    if (event.key !== "Tab") return;
    const list = focusables(root);
    // An empty trap swallows Tab: there is nowhere inside to go, and letting it out is what a trap may not do.
    const first = list[0];
    const last = list[list.length - 1];
    if (first === undefined || last === undefined) {
      event.preventDefault();
      return;
    }
    const active = document.activeElement;
    const inside = active instanceof Node && root.contains(active);
    if (event.shiftKey && (!inside || active === first)) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && (!inside || active === last)) {
      event.preventDefault();
      first.focus();
    }
  };

  document.addEventListener("keydown", onKey, true);
  focusables(root)[0]?.focus();
  return () => {
    document.removeEventListener("keydown", onKey, true);
    // The opener may be gone; the browser then leaves focus on <body>.
    if (before !== null && before.isConnected) before.focus();
  };
}

/** Focus the new page's heading, so a screen reader announces it; `tabindex="-1"` and `preventScroll` keep Tab and scroll out of it. */
export function announce(view: HTMLElement): void {
  const node = view.querySelector("h1") ?? view;
  // Focusable only while it holds the focus, so a heading announced draws as one never announced.
  if (!node.hasAttribute("tabindex")) {
    node.setAttribute("tabindex", "-1");
    node.addEventListener("blur", () => node.removeAttribute("tabindex"), { once: true });
  }
  node.focus({ preventScroll: true });
}
