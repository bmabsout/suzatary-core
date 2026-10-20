/**
 * What a key does in the compose box. Ctrl/⌘-Enter sends at every width, the one chord a phone and a
 * wide screen share; on a wide screen a bare Enter sends too, while on a phone it stays a newline
 * because the send is the button. Escape stops a running round; ↑ in an empty box edits the last question.
 */

export type ComposeDecision = "send" | "newline" | "stop" | "edit" | "none";

/** What the box holds when the key lands — ↑ means two things. */
export type Box = "empty" | "written";

export interface ComposeKeyEvent {
  readonly key: string;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
  readonly shiftKey: boolean;
}

export function composeKey(event: ComposeKeyEvent, wide: boolean, box: Box): ComposeDecision {
  if (event.key === "Escape") return "stop";
  if (event.key === "ArrowUp") {
    const bare = !event.ctrlKey && !event.metaKey && !event.shiftKey;
    return bare && box === "empty" ? "edit" : "none";
  }
  if (event.key !== "Enter") return "none";
  if (event.ctrlKey || event.metaKey) return "send";
  if (!wide) return "newline";
  return event.shiftKey ? "newline" : "send";
}
