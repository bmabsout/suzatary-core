/**
 * `suzatary-wasm`'s glue, declared: the module is a build output copied beside the bundle, and a `tsc` that
 * needed a Nix build first would not be a type gate. `core.ts` restates it as `Core` so node tests can hand
 * it the nodejs glue; the names are the Rust ones, documented in `app/wasm/src/`.
 */

// A `.d.ts` beside where the `.js` will be, not a `declare module`, so `import("./prodrome.js")` resolves
// here while `build-web.sh` keeps that specifier external.

/** A set of todo objects, `[{hash, text}]`, read once; `free()` gives its memory back. */
export class Todos {
  constructor(objects: string);
  /** Rehash, parse, derive the tips, walk from every one, linearise. */
  verify(): string;
  /** The folds at an instant, under a trust policy. */
  fold(at: string | null, untrusted: string): string;
  /** The box's fold answer: every todo as its route composes the row. */
  list(at: string | null, untrusted: string, chain: string, others: Chains): string;
  /** `event` sealed into this replica, a `Change`: `{hash, objects}`, the objects the box lacks of it. */
  append(event: string, genesis: string | null): string;
  free(): void;
}

/** The todo chains a tab holds, the set every row is priced through. */
export class Chains {
  constructor();
  hold(name: string, objects: string, untrusted: string): string;
  drop(name: string): void;
  free(): void;
}

/** A set of inbox objects, `[{hash, text}]`, read once at the proposal schema. */
export class Proposals {
  constructor(objects: string);
  /** Every proposal's registers, each its maximal writes. */
  readings(at: string | null, untrusted: string): string;
  /** The Inbox's list at an instant. */
  inbox(at: string): string;
  free(): void;
}

/** A set of conversation objects, `[{hash, text}]`, read once at the conversation schema. */
export class Conversations {
  constructor(objects: string);
  /** Every conversation's transcript register, its heads. */
  readings(at: string | null, untrusted: string): string;
  /** Every conversation, newest first. */
  list(inbox: Proposals): string;
  /** One conversation's window, the box's route's reading. */
  conversation(
    id: string,
    inbox: Proposals,
    before?: string,
    limit?: string,
    about?: string,
    memory?: string,
  ): string;
  /** Every conversation's turns holding a query. */
  search(q: string): string;
  free(): void;
}

/** What a term is worth at an instant; `undefined` for `∅`. */
export function fulfillment(term: string, now: string, env: string): number | undefined;

/** Every `ref` bound to the todo it names. */
export function link(term: string, specs: string): string;

/** A stored term's canonical print, as the JSON shape. */
export function term_json(literal: string): string;

/** The curve over a window, and whether the knots are it. */
export function series_knots(term: string, from: string, to: string, history: string): string;

/** One lifecycle event, through its own `mk_*`, as its canonical print. */
export function lifecycle(kind: string, todo: string, at: string, actor: string, note: string): string;

/** The prose escaped markup stands for, or `undefined` when it is markup a person wrote. */
export function prose(markup: string): string | undefined;

/** The box's tripwire over typst's HTML: what is wrong with it, or `undefined`. */
export function suspicious(html: string): string | undefined;

/** Every function above throws until this resolves, so `core.ts` alone calls it and memoises it. */
export default function init(init: { module_or_path: string }): Promise<unknown>;
