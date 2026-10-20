// The Inbox's rows: each thing waiting on the owner, resolved to the card it draws against what the page fetched.
// A row whose card cannot be found is `unresolved`: a request this side cannot name, it must not offer to perform.

import type { Entry } from "../wire/generated/wire";
import type { ConversationPayload, ProposalRow } from "../wire/generated/wire";
import type { ProposalsPayload, ProposedBatch } from "../wire/generated/wire";
import inboxState from "../pages/inbox.state";
import waiting from "./waiting";
import data from "../algebra/data";
import asker from "./asker";

// `reviewed` is what the holding decision saw: less than the batch when objects came after the hold.
export type Row =
  | {
      readonly kind: "proposal";
      readonly w: waiting.Waiting;
      readonly conversation: ConversationPayload;
      readonly proposal: ProposalRow;
    }
  | {
      readonly kind: "batch";
      readonly w: waiting.Waiting;
      readonly entry: Entry;
      readonly chain: string;
      readonly batch: ProposedBatch;
      readonly reviewed: ReadonlySet<string>;
    }
  | { readonly kind: "unresolved"; readonly w: waiting.Waiting; readonly problem: string };

// Pure: every lookup is a map read, so what a reference names and what this deployment holds are two questions.
export function rowOf(
  w: waiting.Waiting,
  conversations: ReadonlyMap<string, ConversationPayload>,
  batches: ReadonlyMap<string, ProposalsPayload>,
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
    // A decision names an object and is about the batch holding it: an object added after the hold renames the batch.
    case "Batch": {
      const batch = batches
        .get(at.chain)
        ?.batches.find((row) => row.objects.some((object) => object.hash === at.head));
      return batch === undefined || w.type !== "Decision"
        ? { kind: "unresolved", w, problem: `batch ${at.head} on ${at.chain} could not be found` }
        : {
            kind: "batch",
            w,
            entry: w.e,
            chain: at.chain,
            batch,
            reviewed: inboxState.closure(batch.objects, new Set(), at.head, true),
          };
    }
  }
}

// A batch held twice is drawn once, by the decision that saw the most; accepting the card settles both.
export function oneCardPerBatch(rows: readonly Row[]): Row[] {
  const widest = new Map<string, Row & { kind: "batch" }>();
  for (const row of rows) {
    if (row.kind !== "batch") continue;
    const key = `${row.chain}/${row.batch.head}`;
    const seen = widest.get(key);
    if (seen === undefined || row.reviewed.size > seen.reviewed.size) widest.set(key, row);
  }
  return rows.filter((row) => row.kind !== "batch" || widest.get(`${row.chain}/${row.batch.head}`) === row);
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
