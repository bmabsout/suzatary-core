// What a mail row says: the server's decisions, in the words a reader sees (`skip` prints as the agent moving on).

import type { Disposed, MailRow } from "../wire/generated/wire";

export interface Badge {
  readonly label: string;
  /** A `.chip` modifier: "ok" for a message that became a todo, "" otherwise. */
  readonly kind: "" | "ok";
}

/** What triage made of a message, as a badge, or `null` when it made nothing of it yet. */
export function badgeOf(d: Disposed): Badge | null {
  switch (d.kind) {
    case "undecided":
      return null;
    case "todo":
      return { label: "todo", kind: "ok" };
    case "noise":
      return { label: "noise", kind: "" };
    case "skip":
      return { label: "seen", kind: "" };
  }
}

/** The todo a message became, `null` for every other disposition. */
export function todoOf(d: Disposed): string | null {
  return d.kind === "todo" ? d.todo : null;
}

/** A missing subject says so, rather than leaving a gap. */
export function subjectOf(row: MailRow): string {
  return row.subject.trim() === "" ? "(no subject)" : row.subject;
}

/** The local part of an address; the whole address stays on the row's `title`. */
export function senderOf(row: MailRow): string {
  const at = row.sender.indexOf("@");
  return at <= 0 ? row.sender : row.sender.slice(0, at);
}

/** The oldest row's instant, "" for no page; the server treats it as exclusive, so no row repeats. */
export function cursorOf(rows: readonly MailRow[]): string {
  const last = rows.at(-1);
  return last === undefined ? "" : last.date;
}
