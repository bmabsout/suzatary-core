// The one-line "why, or what next" field beside every act, one builder for every door so they never disagree.
// Empty is the norm, so the placeholder names what the field is for rather than marking it optional.

import { el } from "./dom";

/** What travels: `""` when there is nothing to say, trimmed otherwise. */
export const noteOf = (value: string): string => value.trim();

/** Is there something to say? */
export const hasNote = (value: string): boolean => noteOf(value) !== "";

const PLACEHOLDER = "Why, or what next";

/** The field itself, sharing placeholder, label and class across every act. */
export function noteField(extraClass = ""): HTMLInputElement {
  return el("input", {
    type: "text",
    class: `noteline ${extraClass}`.trim(),
    placeholder: PLACEHOLDER,
    "aria-label": PLACEHOLDER,
  });
}
