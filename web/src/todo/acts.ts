// A todo's acts: a lifecycle event this tab's core spells and appends to its replica, or one the box authors
// (a confirmation, an undo, a redo: rules that must stay one function on the box) and this tab appends.

import { authorConfirm, authorRedo, authorUndo } from "../wire/api";
import type { Lifecycle } from "../prodrome/core";
import { lifecycleEvent, loadCore } from "../prodrome/core";
import act from "../view/act";
import { checkIcon, confirmIcon, crossIcon, undoIcon } from "../view/icons";
import time from "../app/time";
import intentsync from "../prodrome/intentsync";
import { toast } from "../view/toast";
import app from "../app/app";
import type { ActionName } from "./todos";
import lo from "../algebra/lo";

/** Told once the object is appended here, not when the box answers. */
export type Acted = () => void;

interface Verb {
  readonly label: string;
  readonly icon: () => SVGSVGElement;
}

export const VERBS: Record<ActionName, Verb> = {
  done: { label: "Done", icon: checkIcon },
  tend: { label: "Did it", icon: checkIcon },
  cancel: { label: "Cancel", icon: crossIcon },
  reopen: { label: "Reopen", icon: undoIcon },
  confirm: { label: "Confirm", icon: confirmIcon },
};

export const KIND: Record<Exclude<ActionName, "confirm">, Lifecycle> = {
  done: "Completed",
  tend: "Tended",
  cancel: "Cancelled",
  reopen: "Reopened",
};

export const SAID: Record<ActionName, string> = {
  done: "marked done",
  tend: "done again",
  cancel: "cancelled",
  reopen: "reopened",
  confirm: "confirmed",
};

// The one path from this browser to the chain: a monotone intent, sealed into the core's `Change` over the todo's own
// heads, so no head of the replica is picked and several are no reason to wait. The same event asked twice is one
// object. It throws; every caller toasts.
export const sealAndAppend = (event: string): Promise<void> => intentsync.asked({ type: "Seal", chain: app.now().chain, event });

/** One act; never throws, because the only useful place for a failure is the toast. */
export async function perform(name: ActionName, todo: string, acted: Acted, note = ""): Promise<void> {
  const actorName = app.actor(app.now());
  if (actorName === null) {
    toast("this login may read this app but not write to it");
    return;
  }
  try {
    const core = await loadCore();
    const event =
      name === "confirm"
        ? (await authorConfirm(todo, note)).event
        : lifecycleEvent(core, KIND[name], todo, time.moment(), actorName, note);
    await sealAndAppend(event);
    acted();
    // Undo is offered for a lifecycle move only: a confirm trades actors, and the box refuses to undo a tending.
    if (name !== "confirm" && name !== "tend") {
      toast(SAID[name], { label: "Undo", onClick: () => void undoOrRedo("undo", todo, acted) });
    }
  } catch (error: unknown) {
    toast(lo.reason(error));
  }
}

// Always through the box: undoing a spec or a record needs the todo's whole stream, which the wasm core does not fold.
export async function undoOrRedo(which: "undo" | "redo", todo: string, acted: Acted): Promise<void> {
  const actorName = app.actor(app.now());
  if (actorName === null) {
    toast("this login may read this app but not write to it");
    return;
  }
  try {
    const built = which === "undo" ? await authorUndo(todo) : await authorRedo(todo);
    await sealAndAppend(built.event);
    acted();
  } catch (error: unknown) {
    toast(lo.reason(error));
  }
}

/** `note` reads the row's one-line field when pressed, so every button beside it shares one field. */
export const actButton = (name: ActionName, todo: string, acted: Acted, extra = "", note: () => string = () => ""): HTMLElement =>
  act.view({
    label: [VERBS[name].icon(), VERBS[name].label],
    cls: `act act-${name} ${extra}`.trim(),
    run: () => act.again(perform(name, todo, acted, note())),
  });
