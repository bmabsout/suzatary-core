// A todo's edit and defer forms, and its checklist. The box authors each change and this replica seals it.

import { sealAndAppend } from "./acts";
import type { Acted } from "./acts";
import { authorUpdate } from "../wire/api";
import type { Authored } from "../wire/generated/wire";
import { el } from "../view/dom";
import placeholder from "../view/placeholder";
import form from "../view/form";
import parsed from "../algebra/parsed";
import { diffOf, formOf } from "./edit";
import type { Edit } from "./edit";
import edit from "../typst/edit";
import { loadCore } from "../prodrome/core";
import { chevronIcon } from "../view/icons";
import { noteOf } from "../view/noteline";
import time from "../app/time";
import app from "../app/app";
import { toast } from "../view/toast";
import lo from "../algebra/lo";

// A draft is of one version of a todo: the edit of a todo another writer has since changed starts afresh.
const EDITS = form.drafts<Edit>("edit");
const DEFERS = form.drafts<{ readonly day: form.Day }>("defer");

// Closed until opened: editing is an author's choice to make a change, not a reader's need to see one. The form is built
// the first time it opens, once the core can read an escaped title back into the prose it was.
export function editPanel(todo: string, authored: Authored, unconfirmed: boolean, redraw: Acted, categories: () => readonly string[]): HTMLElement {
  const holder = el("div", { class: "pickerhead draftform" }, placeholder.view(placeholder.LINES));
  const details = el("details", { class: "why editbox" }, el("summary", null, "Edit", chevronIcon()), holder);
  const build = (prose: (markup: string) => string | undefined): Node[] => {
    const body = edit.opened(authored.body, prose);
    const detail = edit.opened(authored.detail, prose);
    const opening = { body: body.text, detail: detail.text };
    const blank = formOf(authored, categories(), opening);
    const f = form.mount(form.of(blank), {
      draft: form.draft(EDITS, `${todo}\n${JSON.stringify(form.values(blank))}`),
      read: (fs) => parsed.map(diffOf(todo, authored, fs, opening), (e) => ({ edit: e, note: noteOf(fs.note.v) })),
      send: async ({ edit: e, note }, dispatch) => {
        const authoredEvent = await authorUpdate(e, note);
        await sealAndAppend(authoredEvent.event);
        toast(`updated ${authoredEvent.todo}`);
        dispatch({ type: "Typed", name: "note", v: "" });
        details.open = false;
        redraw();
      },
    });
    const cancel = (): void => {
      f.form.dispatch({ type: "Reset", to: form.of(formOf(authored, categories(), opening)) });
      details.open = false;
    };
    return [
      unconfirmed ? el("p", { class: "meta editprov" }, "Written by triage and not yet confirmed: saving an edit makes it yours, like confirming it.") : null,
      body.wasProse || detail.wasProse
        ? el("p", { class: "meta" }, "Saved in plain text before, so it opens the way you would type it; characters like $ # @ * now read in Typst, which the preview shows.")
        : null,
      f.at("body"),
      el("p", { class: "meta" }, "Typst markup: *bold*, _emphasis_, - lists, #link(\"https://…\")[text]. The preview is what the list shows."),
      f.at("detail"),
      f.at("category"),
      f.at("waiting"),
      f.at("reprice"),
      f.when("reprice", "editprice", f.at("price"), f.at("rationale")),
      el(
        "div",
        { class: "editactions" },
        el("button", { type: "button", class: "cancelbtn", onclick: cancel }, "Cancel"),
        f.button("Save changes", "addbtn"),
        f.at("note"),
      ),
      f.said,
    ].filter((n): n is HTMLElement => n !== null);
  };
  let built = false;
  details.addEventListener("toggle", () => {
    if (!details.open || built) return;
    built = true;
    void loadCore().then(
      (core) => holder.replaceChildren(...build(core.prose)),
      (e: unknown) => ((built = false), holder.replaceChildren(el("p", { class: "problem" }, `the editor could not open: ${String(e)}`))),
    );
  });
  return details;
}

// The box defers the todo's current term and writes the rationale itself, since the date is the reason.
// It refuses a date not after today, so the picker starts tomorrow.
export function deferControl(todo: string, redraw: Acted): HTMLElement {
  const tomorrow = time.date(app.now().zone, time.nextDay(app.now().zone, time.now()));
  const f = form.mount(form.of({ day: form.day({ cls: "draftfield", label: "defer to this date" }, tomorrow) }), {
    draft: form.draft(DEFERS, todo),
    read: (fs) => (fs.day.v === "" ? parsed.refused("Pick a date to defer to.") : parsed.ok({ todo, defer_to: fs.day.v })),
    send: async (update) => {
      const authoredEvent = await authorUpdate(update);
      await sealAndAppend(authoredEvent.event);
      toast(`deferred ${authoredEvent.todo} to ${update.defer_to}`);
      details.open = false;
      redraw();
    },
  });
  const details = el(
    "details",
    { class: "why editbox deferbox" },
    el("summary", null, "Defer to…", chevronIcon()),
    el("div", { class: "pickerhead draftform" }, el("div", { class: "draftrow" }, f.at("day"), f.button("Defer", "addbtn")), f.said),
  );
  return details;
}

// A tick sends the whole checklist with one line flipped: the wire has no shape for one line.
export function subtodoChecklist(todo: string, authored: Authored, redraw: Acted): HTMLElement {
  return el(
    "ul",
    { class: "checklist" },
    authored.subtodos.map((sub, index) => {
      const id = `subtodo-${todo}-${index}`;
      const checkbox = el("input", { type: "checkbox", id, checked: sub.done });
      checkbox.addEventListener("change", () => {
        if (checkbox.hasAttribute("disabled")) return;
        checkbox.setAttribute("disabled", "");
        void toggleSubtodo(todo, authored, index)
          .then(redraw)
          .catch((error: unknown) => {
            toast(lo.reason(error));
            checkbox.checked = sub.done;
            checkbox.removeAttribute("disabled");
          });
      });
      return el("li", { class: sub.done ? "done" : "" }, checkbox, el("label", { for: id }, sub.body));
    }),
  );
}

async function toggleSubtodo(todo: string, authored: Authored, index: number): Promise<void> {
  const subtodos = authored.subtodos.map((sub, i) => (i === index ? { body: sub.body, done: !sub.done } : sub));
  const authoredEvent = await authorUpdate({ todo, subtodos });
  await sealAndAppend(authoredEvent.event);
}
