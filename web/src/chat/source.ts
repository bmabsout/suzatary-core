/**
 * One conversation's loaded window with its live stream folded in: a pure `step` over `SourceState`,
 * and `createSource`, the effectful shell whose every effect ends in `dispatch`.
 * The shell's only closure state is the stream's closing, never the conversation's.
 * When `about` finds no turns the shell refetches the whole conversation; `step` records that as a
 * monotone `fallback`, so a page never un-says "nothing about this yet".
 */

import type { AttachmentRow, ConversationPayload, ConversationState, Frame, ProposalRow, TurnRow, UnplacedProposal } from "../wire/generated/wire";
import type { Reference } from "./refs";
import asks from "../prodrome/asks";
import intentsync from "../prodrome/intentsync";
import dressed from "./dressed";
import storesync from "../prodrome/storesync";
import d from "../wire/decoder";
import payload from "../wire/generated/payload";
import { print } from "./refs";
import sources from "../algebra/source";
import stream from "../algebra/stream";
import latest from "../algebra/latest";
import answer from "./answer";
import time from "../app/time";

export interface SourceDeps {
  readonly fetchWindow: (
    id: string,
    window: { before?: number; limit?: number; about?: string },
  ) => Promise<ConversationPayload>;
  /** One act on the conversation, an intent: resolves once the box took it and the replicas hold it, rejects with why not. */
  readonly ask: (a: asks.Ask) => Promise<void>;
  readonly openStream: (id: string) => stream.Socket;
  /** What says a given-up stream may open again. */
  readonly wake: stream.Wake;
  /** Bring the replicas level with the box, before a read that must see what it just wrote. */
  readonly pull: () => Promise<void>;
  /** Wake on every change of what the replicas hold; returns its own removal. */
  readonly watch: (heard: () => void) => () => void;
}

const frameOf = d.json(payload.Frame);

export const defaultDeps: SourceDeps = {
  fetchWindow: dressed.window,
  pull: storesync.sync,
  watch: storesync.watch,
  ask: intentsync.asked,
  openStream: id => stream.of(`/api/secretary/conversations/${encodeURIComponent(id)}/stream`)(),
  wake: stream.browser,
};

// --- the state sum -----------------------------------------------------------

export interface Loading {
  readonly kind: "loading";
  readonly id: string;
  readonly about: Reference | undefined;
}

/** `problem` is a fetch that failed after a window loaded: the transcript stays on screen, since a dated answer beats none. */
export interface Loaded {
  readonly kind: "loaded";
  readonly id: string;
  readonly about: Reference | undefined;
  /** Monotone: `step` never clears it once set. */
  readonly fallback: boolean;
  readonly problem: string | null;
  readonly title: string;
  readonly model: string;
  readonly opened: string;
  readonly turns: readonly TurnRow[];
  readonly first: number;
  readonly total: number;
  readonly proposals: readonly ProposalRow[];
  /** Proposal lines the inbox's reading cannot place, each drawn with why. */
  readonly unplaced: readonly UnplacedProposal[];
  readonly conversationState: ConversationState;
  /** The question's turn index the box would ask again from; `null` when it would not. */
  readonly again: number | null;
  readonly answer: answer.T;
}

/** Reachable only from `Loading`: a failure after a load stays `Loaded` with `problem`. */
export interface Failed {
  readonly kind: "failed";
  readonly id: string;
  readonly about: Reference | undefined;
  readonly reason: string;
}

export type SourceState = Loading | Loaded | Failed;

export type SourceEvent =
  | {
      readonly kind: "window";
      readonly mode: "older" | "newest";
      readonly payload: ConversationPayload;
      /** The shell measured this fetch as the fallback's second leg; `step` only ORs it in. */
      readonly fallback: boolean;
      /** When it arrived, for the answer's clock. */
      readonly at: number;
    }
  | { readonly kind: "frame"; readonly frame: Frame; readonly at: number }
  | { readonly kind: "failed"; readonly reason: string };

/** Whether Stop could end a round: the one reading the Stop control and Esc share. */
export type Answering = "running" | "idle";

export const answering = (state: SourceState): Answering =>
  state.kind === "loaded" && state.conversationState !== "idle" ? "running" : "idle";

function initialState(id: string, about: Reference | undefined): SourceState {
  return { kind: "loading", id, about };
}

/** A gap between the loaded turns and a newest window trusts the new window whole rather than drawing a hole. */
function mergedTurns(
  priorTurns: readonly TurnRow[],
  priorFirst: number,
  payload: ConversationPayload,
  mode: "older" | "newest",
): { turns: TurnRow[]; first: number } {
  if (mode === "older") {
    return { turns: [...payload.turns, ...priorTurns], first: payload.first };
  }
  if (priorTurns.length === 0 || payload.first > priorFirst + priorTurns.length) {
    return { turns: payload.turns.slice(), first: payload.first };
  }
  if (payload.first >= priorFirst) {
    const keepBefore = payload.first - priorFirst;
    return { turns: [...priorTurns.slice(0, keepBefore), ...payload.turns], first: priorFirst };
  }
  // Impossible on an append-only transcript; keeping the wider range held is safer than discarding it.
  return { turns: priorTurns.slice(), first: priorFirst };
}

function applyWindow(
  state: SourceState,
  event: Extract<SourceEvent, { kind: "window" }>,
): Loaded {
  const priorTurns = state.kind === "loaded" ? state.turns : [];
  const priorFirst = state.kind === "loaded" ? state.first : 0;
  const priorFallback = state.kind === "loaded" ? state.fallback : false;
  const merged = mergedTurns(priorTurns, priorFirst, event.payload, event.mode);
  return {
    kind: "loaded",
    id: state.id,
    about: state.about,
    fallback: priorFallback || event.fallback,
    problem: null,
    title: event.payload.title,
    model: event.payload.model,
    opened: event.payload.opened,
    turns: merged.turns,
    first: merged.first,
    total: event.payload.total,
    proposals: event.payload.proposals,
    unplaced: event.payload.unplaced,
    conversationState: event.payload.state,
    again: event.payload.again,
    answer: answer.settle(state.kind === "loaded" ? state.answer : answer.done, event.payload.state, event.at),
  };
}

/** Turns are only appended, so a newest window holding fewer was read earlier (`say`'s answer can land after the refetch) and is dropped. */
const past = (state: SourceState, event: Extract<SourceEvent, { kind: "window" }>): boolean =>
  state.kind === "loaded" && event.mode === "newest" && event.payload.total < state.total;

function applyFrame(state: Loaded, frame: Frame, at: number): Loaded {
  const next = answer.step(state.answer, frame, at);
  const conversationState = frame.kind === "state" ? frame.state : state.conversationState;
  return next === state.answer && conversationState === state.conversationState
    ? state
    : { ...state, answer: next, conversationState };
}

/** A frame before the first load is dropped (the stream can connect first), and a later failure never discards what is on screen. */
export function step(state: SourceState, event: SourceEvent): SourceState {
  switch (event.kind) {
    case "window":
      return past(state, event) ? state : applyWindow(state, event);
    case "frame":
      return state.kind === "loaded" ? applyFrame(state, event.frame, event.at) : state;
    case "failed":
      if (state.kind === "loaded") return { ...state, problem: event.reason };
      return { kind: "failed", id: state.id, about: state.about, reason: event.reason };
  }
}

// --- the effectful shell -------------------------------------------------

/** What a page can do to the conversation; every effect ends in the fold's `dispatch`. */
export interface SourceEffects {
  /** Safe to call once. */
  start: () => void;
  refresh: () => Promise<void>;
  /** Rejects on a fetch failure; the caller decides how to say so. */
  earlier: () => Promise<void>;
  /** Resolves once the person's turn is on the page; rejects with the box's refusal. */
  say: (text: string, references: readonly Reference[], files?: readonly AttachmentRow[]) => Promise<void>;
  /** Rejects with the box's refusal (409 when the round had already ended). */
  stop: () => Promise<void>;
  /** Ask the last round again; rejects with the box's refusal. */
  regenerate: () => Promise<void>;
  /** Replace the last round's question; rejects with the box's refusal. */
  edit: (text: string, references: readonly Reference[], files?: readonly AttachmentRow[]) => Promise<void>;
  destroy: () => void;
}

export type Source = sources.Source<SourceState, SourceEvent> & SourceEffects;

export function createSource(
  id: string,
  about: Reference | undefined,
  deps: SourceDeps = defaultDeps,
  at?: number,
): Source {
  const fold = sources.create<SourceState, SourceEvent>(initialState(id, about), step);
  let destroyed = false;
  let close = (): void => {};

  // Once fallen back, stays fallen back, so the whole conversation never narrows under the reader on a later reading.
  let effectiveAbout: Reference | undefined = about;

  async function fetchNewestApplyingFallback(): Promise<{ payload: ConversationPayload; fallback: boolean }> {
    const aboutText = effectiveAbout === undefined ? undefined : print(effectiveAbout);
    const first = await deps.fetchWindow(id, aboutText === undefined ? {} : { about: aboutText });
    if (aboutText === undefined || first.turns.length > 0) {
      return { payload: first, fallback: false };
    }
    effectiveAbout = undefined;
    const whole = await deps.fetchWindow(id, {});
    return { payload: whole, fallback: true };
  }

  let unwatch = (): void => {};

  // The box said a round moved, or a write was taken: pull, then read.
  async function refresh(): Promise<void> {
    await deps.pull();
    await read();
  }

  // The replicas only grow, so a reading asked later is never older than one asked before it; the two stores sync
  // apart, so two readings can hold the same turns and differ in their cards, and only the order asked tells them apart.
  // A reading that lands after a later-asked one has is dropped, its failure included.
  const readings = latest.of<{ payload: ConversationPayload; fallback: boolean }>((l) =>
    fold.dispatch(
      l.type === "Val"
        ? { kind: "window", mode: "newest", payload: l.v.payload, fallback: l.v.fallback, at: time.now() }
        : { kind: "failed", reason: l.e },
    ),
  );
  const newest = readings.ask;

  const read = (): Promise<void> => newest(fetchNewestApplyingFallback);

  /** A search hit's first window ends at its turn, so stick-to-bottom lands on it; later refreshes ask for the newest. */
  const openAt = (turn: number): Promise<void> =>
    newest(async () => ({ payload: await deps.fetchWindow(id, { before: turn + 1 }), fallback: false }));

  async function earlier(): Promise<void> {
    const current = fold.state();
    if (current.kind !== "loaded" || current.first <= 0 || destroyed) return;
    const aboutText = effectiveAbout === undefined ? undefined : print(effectiveAbout);
    const payload = await deps.fetchWindow(id, {
      before: current.first,
      ...(aboutText === undefined ? {} : { about: aboutText }),
    });
    if (destroyed) return;
    fold.dispatch({ kind: "window", mode: "older", payload, fallback: false, at: time.now() });
  }

  // How many turns this page has seen: what a question or a stop is asked after.
  const seen = (): number => {
    const current = fold.state();
    return current.kind === "loaded" ? current.total : 0;
  };

  const question = (text: string, references: readonly Reference[], files: readonly AttachmentRow[]): asks.Question => ({
    conversation: id,
    after: seen(),
    text,
    references: references.map(print),
    files,
  });

  // A write's answer is not read: what it wrote reaches the page through the replica like anyone else's,
  // and the intent is held once the replicas have it, so reading now shows it.
  async function acting(a: asks.Ask): Promise<void> {
    await deps.ask(a);
    if (destroyed) return;
    await read();
  }

  // A stream given up opens again when the network may be back; meanwhile the replicas `watch` hears still move it.
  const heard = (data: string): void => {
    const decoded = frameOf(data);
    if (decoded.type !== "Val") return;
    const frame = decoded.v;
    fold.dispatch({ kind: "frame", frame, at: time.now() });
    // A `turn` frame refetches rather than drawing itself: cards, references and `queued` come only from the payload.
    if (frame.kind === "turn") void refresh();
  };

  return {
    ...fold,
    start: () => {
      void (at === undefined ? refresh() : openAt(at));
      close = stream.open(() => deps.openStream(id), { frame: heard }, deps.wake);
      unwatch = deps.watch(() => void read());
    },
    refresh,
    earlier,
    say: (text, references, files = []) => acting({ type: "Question", kind: "Say", question: question(text, references, files) }),
    stop: () => acting({ type: "Round", kind: "Stop", conversation: id, after: seen() }),
    regenerate: () => acting({ type: "Round", kind: "Regenerate", conversation: id, after: seen() }),
    edit: (text, references, files = []) => acting({ type: "Question", kind: "Edit", question: question(text, references, files) }),
    destroy: () => {
      destroyed = true;
      readings.close();
      unwatch();
      close();
    },
  };
}
