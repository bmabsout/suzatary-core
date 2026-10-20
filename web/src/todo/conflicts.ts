// `Entry.conflicts` as data: each register whose frontier the one reading (`register.of`) reads as a conflict, with
// its concurrent hashes, sorted by register name so the card's chip and the todo page read one shape.

import type { Entry } from "../wire/generated/wire";
import register from "../algebra/register";

export interface Conflict {
  readonly kind: string;
  readonly hashes: readonly string[];
}

export function describeConflicts(entry: Entry): Conflict[] {
  return Object.entries(entry.conflicts)
    .filter(([, hashes]) => register.of(hashes).type === "Conflict")
    .map(([kind, hashes]) => ({ kind, hashes }))
    .sort((a, b) => (a.kind < b.kind ? -1 : a.kind > b.kind ? 1 : 0));
}
