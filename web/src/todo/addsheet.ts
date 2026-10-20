// Where a todo is born: a docked bottom sheet, where the thumb is, over a scrim, over the draft's form.
// It reads no fold: the list hears of a new todo through the store, as of every other change.

import { sealAndAppend } from "./acts";
import { authorTodo } from "../wire/api";
import type { Draft } from "./draft";
import { draft, emptyDraft, preselect } from "./draft";
import { el } from "../view/dom";
import form from "../view/form";
import { capture } from "../view/focus";
import { chevronIcon, plusIcon } from "../view/icons";
import { toast } from "../view/toast";
import ui from "../app/ui";
import registers from "../app/registers";

export interface AddSheet {
  readonly node: HTMLElement;
  /** A sibling of the sheet, since it covers the page. */
  readonly scrim: HTMLElement;
  readonly open: () => void;
  /** Open with this body and an otherwise empty draft: a share is an ask made a second ago, not an addition to what was half-typed. */
  readonly prefill: (body: string) => void;
  readonly close: () => void;
  readonly isOpen: () => boolean;
  /** Open to add a subtodo of `parent`, in its category. */
  readonly under: (parent: Parent) => void;
  /** The deployment's categories moved: a closed sheet is drawn again from them, an open one keeps what is typed. */
  readonly recategorised: () => void;
  /** The page is going: the sheet's keyboard trap goes with it, and its register stays. */
  readonly destroy: () => void;
}

export interface Parent {
  readonly todo: string;
  readonly title: string;
  readonly category: string;
}

// `categories` is the deployment's vocabulary as it stands; the form is drawn again from it when it has moved
// since (`/api/me` lands after the first paint), while the sheet is closed or as it opens, before anything is typed.
// `narrowed` is the category the list is narrowed to, "" for none: a sheet opened over it starts there.
export function addSheet(categories: () => readonly string[], narrowed: () => string, on: string): AddSheet {
  const up = ui.cell(registers.sheet, on);
  const last = ui.cell(registers.category);
  let parent: Parent | null = null;
  const subof = el("p", { class: "subof", hidden: true });
  const drawParent = (): void => {
    subof.hidden = parent === null;
    subof.replaceChildren(
      ...(parent === null
        ? []
        : [
            "A subtodo of ",
            el("strong", null, parent.title),
            el(
              "button",
              {
                type: "button",
                class: "subofclear",
                "aria-label": "add it as a todo of its own",
                onclick: () => {
                  parent = null;
                  drawParent();
                },
              },
              "×",
            ),
          ]),
    );
  };

  const sheet = el("div", { class: "picker addsheet", id: "addsheet" });
  const scrim = el("div", { class: "scrim", hidden: true, "aria-hidden": "true" });
  const handle = el(
    "button",
    { type: "button", class: "pickerhandle", "aria-controls": "addsheet", "aria-expanded": "false" },
    el("span", { class: "addicon" }, plusIcon()),
    el("strong", null, "Add a todo"),
    chevronIcon(),
  );
  // Modal while open: Tab must not reach a list nobody can see, and closing gives the keyboard back.
  let releaseSheet: (() => void) | null = null;

  // The sheet drawn from its register; `asked` when the reader opened it now, so it takes the keyboard.
  const draw = (asked: boolean): void => {
    const open = up.get();
    sheet.classList.toggle("open", open);
    scrim.hidden = !open;
    handle.setAttribute("aria-expanded", open ? "true" : "false");
    releaseSheet?.();
    releaseSheet = null;
    if (open) {
      fresh();
      releaseSheet = capture(sheet);
      // The delay is the slide-up transition.
      if (asked)
        window.setTimeout(() => {
          const body = f.at("body");
          (body.querySelector("textarea") ?? body).focus();
        }, 60);
      return;
    }
    parent = null;
    drawParent();
  };
  const setSheet = (open: boolean): void => {
    if (open === up.get()) return;
    up.set(open);
    draw(open);
  };
  handle.addEventListener("click", () => {
    if (up.get()) setSheet(false);
    else opened();
  });
  scrim.addEventListener("click", () => {
    setSheet(false);
  });

  // The box authors (the escape, the term, the id) and this replica seals; a subtodo is the child, then the parent's revision.
  const mount = (vocabulary: readonly string[]) =>
    form.mount(form.of(emptyDraft(vocabulary)), {
      read: draft,
      send: async (todo, dispatch) => {
        const under = parent;
        const authored = await authorTodo(under === null ? todo : { ...todo, parent: under.todo });
        await sealAndAppend(authored.event);
        if (authored.revision !== null) await sealAndAppend(authored.revision);
        last.set(todo.category);
        dispatch({ type: "Reset", to: form.of(emptyDraft(vocabulary)) });
        seed();
        setSheet(false);
        toast(under === null ? `added ${authored.todo}` : `added ${authored.todo} under ${under.todo}`);
      },
    });
  let made = categories();
  let f = mount(made);
  const head = el("div", { class: "pickerhead draftform" });
  const fill = (): void =>
    head.replaceChildren(
      subof,
      f.at("body"),
      f.saidBy("body"),
      el("p", { class: "meta" }, "Typst markup: *bold*, _emphasis_, #link(\"https://…\")[text]. The preview is what the list shows."),
      f.at("category"),
      f.saidBy("category"),
      f.at("price"),
      f.saidBy("price"),
      f.at("rationale"),
      f.said,
      f.button([plusIcon(), "Add todo"], "addbtn"),
    );
  // A form with no category chosen starts in the one the list is narrowed to, or the last one used; on a wide screen the
  // form is always shown, so it is seeded when drawn and after each todo it adds, not only when the sheet opens.
  const seed = (): void => {
    if (f.form.state().fields.category.v === "") f.form.dispatch({ type: "Typed", name: "category", v: preselect(made, narrowed(), last.get()) });
  };
  const fresh = (): void => {
    const now = categories();
    if (now.length === made.length && now.every((c, i) => c === made[i])) return;
    made = now;
    f = mount(made);
    fill();
    seed();
  };
  fill();
  seed();
  const opened = (): void => {
    fresh();
    seed();
    setSheet(true);
  };
  const start = (vs: form.Values<Draft>): void => {
    fresh();
    f.form.dispatch({ type: "Reset", to: form.of(form.fill(emptyDraft(made), vs)) });
    seed();
    setSheet(true);
  };

  sheet.appendChild(handle);
  sheet.appendChild(el("div", { class: "pickerbody" }, head));
  // A list drawn again with its sheet up (back to it, say) draws it up, once the page has placed it.
  queueMicrotask(() => {
    if (up.get()) draw(false);
  });

  return {
    node: sheet,
    scrim,
    open: opened,
    prefill: (body: string) => start({ body }),
    close: () => {
      setSheet(false);
    },
    isOpen: () => up.get(),
    destroy: () => {
      releaseSheet?.();
      releaseSheet = null;
    },
    recategorised: () => {
      if (!up.get()) fresh();
    },
    under: (p: Parent) => {
      start({ category: categories().includes(p.category) ? p.category : "" });
      parent = p;
      drawParent();
    },
  };
}
