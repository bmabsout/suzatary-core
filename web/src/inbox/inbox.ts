// The Inbox's rows: each thing waiting on the owner, resolved to the card it draws against what the page fetched.
// A row whose card cannot be found is `unresolved`: a request this side cannot name, it must not offer to perform.

import type { ConversationPayload, ProposalRow } from "../wire/generated/wire";
import waiting from "./waiting";
import data from "../algebra/data";
import asker from "./asker";

export type Row =
  | {
      readonly kind: "proposal";
      readonly w: waiting.Waiting;
      readonly conversation: ConversationPayload;
      readonly proposal: ProposalRow;
    }
  | { readonly kind: "unresolved"; readonly w: waiting.Waiting; readonly problem: string };

// Pure: every lookup is a map read, so what a reference names and what this deployment holds are two questions.
export function rowOf(
  w: waiting.Waiting,
  conversations: ReadonlyMap<string, ConversationPayload>,
): Row {
  const at = waiting.where(w);
  switch (at.type) {
    case "Unresolved":
      return { kind: "unresolved", w, problem: at.problem };
    case "Proposal": {
      const conversation = conversations.get(at.conversation);
      const proposal = conversation?.proposals.find((row) => row.index === at.index);
      return conversation === undefined || proposal === undefined
        ? { kind: "unresolved", w, problem: `proposal ${at.index} of conversation ${at.conversation} could not be found` }
        : { kind: "proposal", w, conversation, proposal };
    }
    // A triage batch is no card: it becomes one when the migration carries it into the Inbox.
    case "Batch":
      return { kind: "unresolved", w, problem: `batch ${at.head} on ${at.chain} is not a card yet` };
  }
}

// Two readings of the rows are the same reading when every row is the same data, whatever of it its card draws.
export const sameRows = (a: readonly Row[], b: readonly Row[]): boolean => data.same(a, b);

// The rows the owner's work is, then the setup cards (a secret the box asks for in its setup conversation), each in the
// order it came: the Inbox draws the second as one row, and every other card of the box's stands among the work.
export const splitSetup = (rows: readonly Row[]): { readonly work: Row[]; readonly setup: Row[] } => {
  const setup = (r: Row): boolean => r.kind === "proposal" && asker.isSetup(r.proposal.card.by);
  return { work: rows.filter((r) => !setup(r)), setup: rows.filter(setup) };
};

/** What waits, counted as the page shows it: the work, then the setup steps folded under it; "" for nothing. */
export const waitingSentence = (work: number, steps: number): string => {
  const setup = `${steps} setup step${steps === 1 ? "" : "s"}`;
  return work + steps === 0 ? ""
    : steps === 0 ? `${work} thing${work === 1 ? "" : "s"} waiting on you.`
    : work === 0 ? `${setup} waiting on you.`
    : `${work} waiting on you, and ${setup}.`;
};
