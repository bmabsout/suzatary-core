/** A card for a test proposal that is not about its card: the shape
 * `secretary::card::of` gives an email, shared so no test spells its own. */

import type { Card } from "../src/wire/generated/wire";

export function aCard(over: Partial<Card> = {}): Card {
  return {
    title: "Proposed email",
    by: { kind: "secretary" },
    sentence: "",
    fields: [],
    body: "",
    rows: [],
    files: [],
    problem: "",
    answers: [
      { verb: "send", label: "Send", question: "Send?", yes: "Yes, send it", done: "sent", always: false, input: "", endorse: false },
    ],
    uncertain: "It may have been sent. Check Sent; it is never sent again.",
    readonly: "Only the owner's own device can send this.",
    link: null,
    ...over,
  };
}
