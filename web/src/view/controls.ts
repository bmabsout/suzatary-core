// A filter's chip row, built once: each redraw toggles `.on` on buttons that exist, so a tap keeps its focus and the row its scroll.

import type { Choice } from "../todo/todos";
import { ANY, is } from "../todo/todos";
import { el } from "./dom";

export interface Group {
  readonly row: HTMLElement;
  readonly sync: () => void;
}

// "" is a real category, so it gets a name rather than hiding behind "any".
const nameOf = (value: string, empty: string): string => (value === "" ? empty : value);

export function chipGroup(
  values: readonly string[],
  anyLabel: string,
  emptyLabel: string,
  read: () => Choice,
  write: (choice: Choice) => void,
): Group {
  const options: { choice: Choice; button: HTMLElement }[] = [
    { choice: ANY, button: el("button", { type: "button", class: "chipbtn" }, anyLabel) },
    ...values.map((value) => ({
      choice: is(value),
      button: el("button", { type: "button", class: "chipbtn" }, nameOf(value, emptyLabel)),
    })),
  ];
  const same = (a: Choice, b: Choice): boolean =>
    a.tag === "any" ? b.tag === "any" : b.tag === "is" && a.value === b.value;
  const sync = (): void => {
    const current = read();
    for (const option of options) {
      const on = same(option.choice, current);
      option.button.classList.toggle("on", on);
      option.button.setAttribute("aria-pressed", on ? "true" : "false");
    }
  };
  for (const option of options) {
    option.button.addEventListener("click", () => {
      write(same(option.choice, read()) ? ANY : option.choice);
    });
  }
  return { row: el("div", { class: "chiprow" }, ...options.map((option) => option.button)), sync };
}
