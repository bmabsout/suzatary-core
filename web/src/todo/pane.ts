// The pane beside the list at 62rem and up: one todo's page, or the hint while none is picked.
// The stylesheet shows it only at that width; below it the router pushes the todo page instead.

import { clear, el } from "../view/dom";
import { type Mounted, inert } from "../view/page";
import frame from "../view/frame";
import todoview from "./todoview";

export interface DetailPane {
  readonly node: HTMLElement;
  // Showing what is already shown does nothing, so a second tap on the same card does not refold.
  show: (id: string) => void;
  destroy: () => void;
}

export function detailPane(): DetailPane {
  const node = el("div", { class: "detailpane" });
  let shown = "";
  let view: Mounted = inert;

  const hint = (): void => {
    clear(node);
    node.appendChild(
      el(
        "div",
        { class: "panehint" },
        el("p", { class: "panehintlead" }, "Pick a todo."),
        el("p", { class: "legend" }, "Its detail, what it costs, why that number, and every event on the chain that made it."),
      ),
    );
  };

  hint();

  return {
    node,
    show: (id: string): void => {
      if (id === shown) return;
      view.destroy();
      view = inert;
      shown = id;
      if (id === "") {
        hint();
        return;
      }
      view = frame.open(node, todoview.page(id, false));
    },
    destroy: () => {
      view.destroy();
      view = inert;
    },
  };
}
