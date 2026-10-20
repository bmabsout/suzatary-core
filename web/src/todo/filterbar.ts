// The search and the filter chips and the `Filter` they hold; the caller places them and decides what a change means.
// `sync` only flips pressed states; `update` rebuilds the facet chips when the data moved, never on a tap,
// since rebuilding under a thumb loses scroll and focus.

import type { Group } from "../view/controls";
import { chipGroup } from "../view/controls";
import { clear, el } from "../view/dom";
import type { Entry } from "../wire/generated/wire";
import type { Filter, StateFilter } from "./todos";
import { categoriesOf, kindsOf } from "./todos";

const STATES: readonly { value: StateFilter; label: string }[] = [
  { value: "open", label: "Open" },
  { value: "done", label: "Done" },
  { value: "all", label: "All" },
];

export interface FilterBar {
  /** The search and the chips, for the caller to place. */
  readonly node: HTMLElement;
  /** The search field, so the keyboard's `/` can reach it. */
  readonly focusSearch: () => void;
  /** The filter as it stands. */
  readonly filter: () => Filter;
  /** The entries the chips are built from: rebuilt when they moved, and every control brought back in step. */
  readonly update: (entries: readonly Entry[]) => void;
  readonly destroy: () => void;
}

export interface FilterBarOptions {
  /** The filter's register, read and written as it stands; another tab's write is heard and drawn. */
  readonly filter: { readonly get: () => Filter; readonly set: (next: Filter) => void; readonly hear: (f: () => void) => () => void };
  /** The deployment's categories, in its order, as they stand when the chips are drawn. */
  readonly categories: () => readonly string[];
}

export function filterBar(options: FilterBarOptions): FilterBar {
  const filter = (): Filter => options.filter.get();

  /** The state segments live for the strip's whole life; the chip groups are rebuilt when the data changes. */
  const stateGroups: Group[] = [];
  let facetGroups: Group[] = [];

  const sync = (): void => {
    for (const group of [...stateGroups, ...facetGroups]) group.sync();
  };

  const change = (next: Filter): void => {
    options.filter.set(next);
    sync();
  };

  const search = el("input", {
    type: "search",
    class: "search",
    placeholder: "Search todos",
    "aria-label": "search todos by id or body",
    enterkeyhint: "search",
  });
  search.value = filter().query;
  search.addEventListener("input", () => {
    change({ ...filter(), query: search.value });
  });

  const stateRow = el(
    "div",
    { class: "segmented", role: "group", "aria-label": "state" },
    ...STATES.map(({ value, label }) => {
      const button = el("button", { type: "button", class: "segment" }, label);
      button.addEventListener("click", () => {
        change({ ...filter(), state: value });
      });
      stateGroups.push({
        row: button,
        sync: () => {
          const on = filter().state === value;
          button.classList.toggle("on", on);
          button.setAttribute("aria-pressed", on ? "true" : "false");
        },
      });
      return button;
    }),
  );

  /**
   * Only what the chain has not settled: unconfirmed content or a register in conflict. A toggle, not a
   * fourth segment, because it narrows whichever partition shows rather than partitioning.
   */
  const attentionButton = el(
    "button",
    { type: "button", class: "chipbtn attentionbtn" },
    "Needs a look",
  );
  attentionButton.addEventListener("click", () => {
    change({ ...filter(), attention: !filter().attention });
  });
  stateGroups.push({
    row: attentionButton,
    sync: () => {
      attentionButton.classList.toggle("on", filter().attention);
      attentionButton.setAttribute("aria-pressed", filter().attention ? "true" : "false");
    },
  });

  /** Only what waits on someone else, a fact about the record, so a toggle rather than a facet chip. */
  const waitingButton = el("button", { type: "button", class: "chipbtn waitingbtn" }, "Waiting");
  waitingButton.addEventListener("click", () => {
    change({ ...filter(), waiting: !filter().waiting });
  });
  stateGroups.push({
    row: waitingButton,
    sync: () => {
      waitingButton.classList.toggle("on", filter().waiting);
      waitingButton.setAttribute("aria-pressed", filter().waiting ? "true" : "false");
    },
  });

  /** Only what is due in the next fortnight (`todos.isDated`). */
  const datedButton = el("button", { type: "button", class: "chipbtn datedbtn" }, "Dated");
  datedButton.addEventListener("click", () => {
    change({ ...filter(), dated: !filter().dated });
  });
  stateGroups.push({
    row: datedButton,
    sync: () => {
      datedButton.classList.toggle("on", filter().dated);
      datedButton.setAttribute("aria-pressed", filter().dated ? "true" : "false");
    },
  });

  const facets = el("div", { class: "facets" });

  const drawFacets = (entries: readonly Entry[]): void => {
    clear(facets);
    facetGroups = [];
    const categories = chipGroup(
      categoriesOf(entries, options.categories()),
      "All",
      "uncategorised",
      () => filter().category,
      (choice) => {
        change({ ...filter(), category: choice });
      },
    );
    facets.appendChild(categories.row);
    facetGroups.push(categories);
    const kinds = kindsOf(entries);
    if (kinds.length > 1) {
      const kindGroup = chipGroup(
        kinds,
        "Any kind",
        "no kind",
        () => filter().kind,
        (choice) => {
          change({ ...filter(), kind: choice });
        },
      );
      facets.appendChild(kindGroup.row);
      facetGroups.push(kindGroup);
    }
  };

  let built: readonly Entry[] = [];
  const node = el(
    "div",
    { class: "filterbar" },
    el("div", { class: "searchbox" }, search),
    el("div", { class: "filterline" }, stateRow, attentionButton, waitingButton, datedButton),
    facets,
  );

  return {
    node,
    focusSearch: () => {
      search.focus();
      search.select();
    },
    filter,
    destroy: options.filter.hear(() => {
      if (search.value !== filter().query) search.value = filter().query;
      sync();
    }),
    update: (entries: readonly Entry[]): void => {
      if (entries !== built) drawFacets(entries);
      built = entries;
      sync();
    },
  };
}
