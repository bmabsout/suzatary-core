// The chains' names, and the one this browser remembers reading. Not in the hash, which says which
// page, so a todo link opens that id on whichever chain the reader is on.

import ui from "./ui";
import registers from "./registers";

/** The first chain, and what an unset selection means. */
export const DEFAULT_CHAIN = "life";

/** The project's own chain, reached at `#/project`. */
export const PROJECT_CHAIN = "suzatary";

/** A chain as the owner speaks of it; a chain this app has no words for is its own name. */
export const said = (name: string): string =>
  name === DEFAULT_CHAIN ? "My todos" : name === PROJECT_CHAIN ? "This app's work" : name;

/** A chain name is a slug because it is a path segment; a stale stored value must not become `/api/../fold`. */
export function isChainName(value: unknown): value is string {
  return typeof value === "string" && /^[a-z][a-z0-9-]{0,31}$/.test(value);
}

/** The chain a reload opens on: the last one read, or the first. */
export function remembered(): string {
  const held = ui.cell(registers.chain).get();
  return isChainName(held) ? held : DEFAULT_CHAIN;
}

export function remember(name: string): void {
  ui.cell(registers.chain).set(name);
}
