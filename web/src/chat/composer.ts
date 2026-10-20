/**
 * The composer: chips, the box, Send and its keys; it never fetches, a frame wires `onSend` to a `Source`.
 * A login that may not write gets no box, since saying is a write, and one line in its place saying where to write from;
 * both are drawn when `/api/me` answers, never assumed.
 */

import type { AttachmentRow, TurnRow } from "../wire/generated/wire";
import type { Reference } from "./refs";
import { upload } from "../wire/api";
import app from "../app/app";
import { composeKey } from "./compose";
import readonly from "../view/readonly";
import { clear, el } from "../view/dom";
import { clipIcon, sendIcon } from "../view/icons";
import attachment from "./attachment";
import { isWide } from "../shell/layout";
import { encoded, reencode } from "./photo";
import { print } from "./refs";
import { toast } from "../view/toast";
import type drafts from "./drafts";
import draftbox from "./draftbox";
import again from "./again";
import type { Answering } from "./source";
import lo from "../algebra/lo";
import source from "../algebra/source";
import composing from "./composing";

export interface ComposerOptions {
  /** Chips attached at mount: a hash's `refs`, an `about` reference, or both. */
  readonly seeded: readonly Reference[];
  /** Take the keyboard the first time the box exists for a writing login. */
  readonly focus: boolean;
  /** `files` are ones the box already keeps (`upload`). */
  readonly onSend: (
    text: string,
    references: readonly Reference[],
    files: readonly AttachmentRow[],
  ) => Promise<void>;
  /** Escape stops the running round; `"idle"` when none runs lets the key go on to close a sheet. */
  readonly onStop: () => Answering;
  /** Asking the last round again: the question ↑ would bring back, and sending one reworded. */
  readonly again: {
    readonly offer: () => again.Offer;
    readonly edit: (text: string, references: readonly Reference[], files: readonly AttachmentRow[]) => Promise<void>;
  };
  /** What was typed and not sent, kept per conversation. */
  readonly draft: drafts.Draft;
}

export interface Composer {
  /** Chips row, textarea and Send, for a frame to place. */
  readonly root: HTMLElement;
  /** Attach a reference as a removable chip; attaching the same one twice is one chip. */
  attach: (reference: Reference) => void;
  /** Quote a turn's text into the box: a quote, not a reference, because a turn is not a destination. */
  quote: (text: string) => void;
  /** Bring a question back to be reworded — Edit on the transcript, and ↑. */
  edit: (question: TurnRow) => void;
  destroy: () => void;
}

export function mountComposer(options: ComposerOptions): Composer {
  let destroyed = false;
  let focused = false;

  let box = draftbox.open(options.draft);
  const state = source.create(composing.of(box.text, options.seeded), composing.step);
  const dispatch = state.dispatch;

  const textbox = el("textarea", {
    class: "composebox",
    rows: "1",
    placeholder: "Ask, or say what to do",
  });
  const send = el("button", { type: "button", class: "composesend", title: "Send", "aria-label": "Send" }, sendIcon());
  const chips = el("div", { class: "composechips" });
  const picker = el("input", { type: "file", accept: "image/*,application/pdf", multiple: "", hidden: "" });
  const addFile = el(
    "button",
    { type: "button", class: "composeattach", title: "Attach a photo or PDF", "aria-label": "Attach a photo or PDF" },
    clipIcon(),
  );
  const thumbs = el("div", { class: "composethumbs" });
  const cancelEdit = el("button", { type: "button", class: "composecancel", onclick: () => dispatch({ type: "Cancel" }) }, "Cancel");
  const editingNote = el(
    "div",
    { class: "composeediting", hidden: true },
    el("span", {}, "Editing your question — Send asks it again"),
    cancelEdit,
  );
  const field = el("div", { class: "composefield unsaid" });
  const elsewhere = el("div", { class: "composeelsewhere", hidden: true });
  const composer = el("div", { class: "compose" });

  // The box over its draft register (`draftbox`): what the state's words become is written there, once per change, and
  // what the register hears comes back as words; a conflict is shown whole until the reader keeps the box or takes the
  // other words.
  const persist = (s: composing.T): void => {
    if (box.text !== s.draft.text) box = draftbox.typed(options.draft, box, s.draft.text);
  };
  const boxed = (b: draftbox.T): void => {
    box = b;
    dispatch({ type: "Typed", text: b.text });
    drawDraft();
  };
  const settle = (text: string): void => boxed(draftbox.settled(options.draft, text));
  const drawDraft = (): void => {
    const g = options.draft.read();
    const text = state.state().draft.text;
    elsewhere.hidden = g.type !== "Conflict";
    elsewhere.replaceChildren(
      ...(g.type !== "Conflict"
        ? []
        : [
            el("p", { class: "legend" }, "This draft was also typed elsewhere. Keep what is in the box, or take the other words:"),
            el("button", { type: "button", class: "composekeep", onclick: () => settle(state.state().draft.text) }, "Keep mine"),
            ...g.vs
              .filter((other) => other !== text)
              .map((other) =>
                el("button", { type: "button", class: "composeuse", onclick: () => settle(other) }, other === "" ? "(nothing)" : other),
              ),
          ]),
    );
  };

  const chip = (reference: Reference): HTMLElement =>
    el(
      "span",
      { class: `composechip ref-${reference.kind}` },
      print(reference),
      el("button", { type: "button", class: "chipdrop", title: "Remove", onclick: () => dispatch({ type: "Detached", reference }) }, "×"),
    );

  const thumb = (file: AttachmentRow): HTMLElement =>
    el(
      "span",
      { class: "composethumb" },
      attachment.view(file),
      el("button", { type: "button", class: "chipdrop", title: "Remove", onclick: () => dispatch({ type: "Unfiled", id: file.id }) }, "×"),
    );

  // The box held a file back: say why, and keep it privately in the Drive or drop it.
  const asking = (h: composing.Held): HTMLElement => {
    const w = h.w;
    const keepPrivate = (): void => {
      dispatch({ type: "Keeping", w });
      void upload(w.base64, w.name, true)
        .then((answer) => dispatch(answer.kind === "kept" ? { type: "Kept", w, file: answer.file } : { type: "Dropped", w }))
        .catch((error: unknown) => {
          dispatch({ type: "Unkept", w });
          toast(lo.reason(error));
        });
    };
    return el(
      "div",
      { class: "composeheld" },
      el("p", {}, `${w.name}: ${w.why}.`),
      el(
        "p",
        {},
        `Keep it in your Drive at ${w.folder}? The secretary reads its text with those numbers blacked out, and reading it again or sending it asks you first.`,
      ),
      el(
        "div",
        { class: "composeheldacts" },
        el("button", { type: "button", class: "composeheldkeep", disabled: h.type === "Keeping", onclick: keepPrivate }, "Keep it private"),
        el("button", { type: "button", class: "composeheldrop", onclick: () => dispatch({ type: "Dropped", w }) }, "Don't send it"),
      ),
    );
  };

  // A function of the state alone: drawn twice, the page is the same. Each part is drawn again only when its slice of
  // the state is a new value, so a keystroke rebuilds no chip or photo. The box grows with its text up to the
  // stylesheet's max-height; the field is quiet while there is nothing to send.
  let drawn: composing.T | null = null;
  const draw = (s: composing.T): void => {
    if (textbox.value !== s.draft.text) textbox.value = s.draft.text;
    editingNote.hidden = s.mode.type === "Asking";
    if (drawn?.draft.references !== s.draft.references) chips.replaceChildren(...s.draft.references.map(chip));
    if (drawn?.held !== s.held || drawn.draft.files !== s.draft.files || drawn.uploading !== s.uploading)
      thumbs.replaceChildren(
        ...s.held.map(asking),
        ...s.draft.files.map(thumb),
        ...(s.uploading.size > 0 ? [el("span", { class: "composethumb pending" }, "…")] : []),
      );
    drawn = s;
    textbox.style.height = "auto";
    textbox.style.height = `${textbox.scrollHeight}px`;
    field.classList.toggle("unsaid", s.draft.text.trim() === "" && s.draft.files.length === 0);
    drawDraft();
  };
  state.subscribe(persist);
  const undrawn = state.subscribe(draw);
  const unheard = options.draft.hear(() => boxed(draftbox.heard(options.draft, box)));
  textbox.addEventListener("input", () => dispatch({ type: "Typed", text: textbox.value }));

  let picks = 0;
  async function keep(file: File): Promise<void> {
    const id = (picks += 1);
    dispatch({ type: "Picked", id });
    try {
      const pdf = file.type === "application/pdf";
      const base64 = pdf ? await encoded(file) : await reencode(file);
      const name = pdf ? file.name : "";
      const answer = await upload(base64, name);
      dispatch(
        answer.kind === "kept"
          ? { type: "Filed", id, file: { ...answer.file, name } }
          : { type: "Withheld", id, w: { base64, name, why: answer.why, folder: answer.folder } },
      );
    } catch (error: unknown) {
      dispatch({ type: "Lost", id });
      toast(lo.reason(error));
    }
  }

  addFile.addEventListener("click", () => picker.click());
  picker.addEventListener("change", () => {
    const chosen = [...(picker.files ?? [])];
    picker.value = "";
    for (const file of chosen) void keep(file);
  });

  const atEnd = (): void => {
    textbox.focus();
    textbox.setSelectionRange(textbox.value.length, textbox.value.length);
  };

  function attach(reference: Reference): void {
    dispatch({ type: "Attached", reference });
    textbox.focus();
  }

  function quote(text: string): void {
    dispatch({ type: "Quoted", text });
    atEnd();
  }

  function edit(question: TurnRow): void {
    dispatch({ type: "Edit", question });
    atEnd();
  }

  function submit(): void {
    composing.fold(composing.sending(state.state()), {
      Blank: () => {},
      Uploading: () => toast("A file is still uploading."),
      Held: () => toast("Answer the file above first."),
      Ready: ({ sent }) => {
        // Never disabled: the transcript is the queue, so a `say` mid-round is the next line the worker delivers.
        const say = sent.mode.type === "Editing" ? options.again.edit : options.onSend;
        dispatch({ type: "Send" });
        void say(sent.draft.text.trim(), sent.draft.references, sent.draft.files).catch((error: unknown) => {
          dispatch({ type: "Failed", sent });
          toast(lo.reason(error));
        });
      },
    });
  }

  send.addEventListener("click", submit);
  textbox.addEventListener("keydown", (event: KeyboardEvent) => {
    const s = state.state();
    const written = s.draft.text === "" && s.draft.files.length === 0 ? "empty" : "written";
    switch (composeKey(event, isWide(window.innerWidth), written)) {
      case "send":
        event.preventDefault();
        submit();
        return;
      case "stop":
        if (s.mode.type === "Editing") dispatch({ type: "Cancel" });
        else if (options.onStop() === "idle") return;
        event.preventDefault();
        event.stopPropagation();
        return;
      case "edit":
        again.fold(
          options.again.offer(),
          () => {},
          (question) => {
            event.preventDefault();
            edit(question);
          },
        );
        return;
      case "newline":
      case "none":
        return;
    }
  });

  const drawComposer = (): void => {
    clear(composer);
    const reach = app.reach(app.now());
    if (reach.type !== "Writes") {
      const line = readonly.line(reach);
      if (line !== null) composer.appendChild(line);
      return;
    }
    composer.appendChild(editingNote);
    composer.appendChild(elsewhere);
    composer.appendChild(chips);
    composer.appendChild(thumbs);
    field.replaceChildren(addFile, textbox, send);
    composer.appendChild(field);
    composer.appendChild(picker);
    draw(state.state());
    // Deferred a tick so it lands after `main.ts`'s post-mount focus move instead of racing it.
    if (options.focus && !focused) {
      focused = true;
      setTimeout(() => {
        if (!destroyed) textbox.focus();
      }, 0);
    }
  };
  drawComposer();
  const unwatch = app.hear(app.reach, drawComposer, app.sameReach);

  return {
    root: composer,
    attach,
    quote,
    edit,
    destroy: () => {
      destroyed = true;
      unwatch();
      unheard();
      undrawn();
      // `persist` stays: a send still on its way when the composer goes hands a failure back to the conversation's draft.
    },
  };
}
