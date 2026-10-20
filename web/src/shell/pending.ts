// The todo page's notices about what this browser holds that the box has not taken: one when the box refused an act, one
// when acts wait for this device to be trusted. Each todo's line says the same of its own act, and the shell says it
// once on any page (`shell/outbox.ts`).

import { el } from "../view/dom";
import { awaitingTrust, losses, stranded, stuckBehind } from "../prodrome/replica";
import { held } from "../prodrome/sync";

/**
 * A refused object is kept, never retried or discarded, and said in the box's sentence verbatim, with how many later
 * acts rest on it; so is an act written in a store the box no longer serves. Resetting the replica is the one escape: it
 * says how many acts it loses, every unsent act included, and hands the reset exactly those names, so a row holding one
 * more by the time it is pressed refuses it.
 */
export function refusalNotice(reset: (shown: ReadonlySet<string>) => void): HTMLElement | null {
  const replica = held();
  const { held: blocked, reasons } = stuckBehind(replica);
  const away = stranded(replica).length;
  if (reasons.length === 0 && away === 0) return null;
  const lost = losses(replica);
  const others = blocked - reasons.length;
  const button = el("button", { type: "button", class: "sincedismiss" }, "Reset this replica from the box");
  button.addEventListener("click", () => reset(new Set(lost.map((object) => object.hash))));
  return el(
    "div",
    { class: "sincebox refusedbox", role: "alert" },
    ...(reasons.length === 0
      ? []
      : [
          el(
            "p",
            { class: "sincewhat" },
            others > 0
              ? `The box refused ${reasons.length} of this browser's objects, and ${others} written since ${others === 1 ? "rests" : "rest"} on ${reasons.length === 1 ? "it" : "them"} — none of them can be sent.`
              : `The box refused ${reasons.length} of this browser's ${reasons.length === 1 ? "object" : "objects"} — ${reasons.length === 1 ? "it" : "they"} cannot be sent.`,
          ),
          el(
            "ul",
            { class: "sincelist" },
            reasons.map((one) => el("li", null, one.reason)),
          ),
        ]),
    ...(away === 0
      ? []
      : [el("p", { class: "sincewhat" }, `${away} of this browser's unsent ${away === 1 ? "acts was" : "acts were"} written in a store the box no longer serves.`)]),
    el(
      "p",
      { class: "sincewhat losses" },
      `A reset drops every change this browser holds that has not reached the box: ${lost.length} in all.`,
    ),
    button,
  );
}

/** Acts the box will take once this device is trusted: kept and said, with no reset offered, since nothing is lost. */
export function waitingNotice(): HTMLElement | null {
  const waiting = awaitingTrust(held());
  if (waiting === 0) return null;
  return el(
    "div",
    { class: "sincebox waitingbox", role: "status" },
    el(
      "p",
      { class: "sincewhat" },
      `${waiting} of this browser's ${waiting === 1 ? "acts waits" : "acts wait"} for this device to be trusted. Trust it from Settings → This device with the code from the box, or ask there and tap its card on a trusted device; once it is trusted, ${waiting === 1 ? "it goes" : "they go"} to the box unchanged.`,
    ),
  );
}
