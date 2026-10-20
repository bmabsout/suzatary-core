// Every route the box answers, named once: its path, its method, and the decoder its answer passes through `http`.

import app from "../app/app";
import type { AboutPayload, AcceptedPayload, AttachmentRow, Authoring, AuthoredPayload, BlockRender, BriefingPayload, BriefingsPayload, CalendarPayload, ChainPayload, ConversationPayload, DrawnPayload, Erasing, FoldPayload, InboxPayload, KeyAsked, MailMessage, MailSearchPayload, MePayload, MemoryPayload, NewTodo, ObjectsPayload, Overview, Pairing, PickedObjects, PostedObjects, ProposalsPayload, Reasoning, ReceiptPayload, RejectedPayload, ReleasesPayload, SayPayload, Saying, StorePayload, Tapping, ThreadPayload, Titling, TodoPayload, UpdateTodo, Uploaded, Uploading, UsagePayload, WaitingPayload } from "./generated/wire";
import { print } from "../chat/refs";
import type { Reference } from "../chat/refs";
import heads from "../prodrome/heads";
import http from "./http";
import wait from "./wait";
import once from "../algebra/once";
import d from "./decoder";
import payload from "./generated/payload";

// Every ordinary page asks about the chain it is on; the Inbox asks every chain, so it names one.
const onChain = (route: string, chain?: string): string =>
  chain === undefined ? `${app.apiBase(app.now())}/${route}` : `/api/${encodeURIComponent(chain)}/${route}`;

// --- the chain ---------------------------------------------------------------

// The biggest answer, kept while the tip stands still; a rejected fetch is never kept.
const chainKept = once.of((): Promise<ChainPayload> => http.get(onChain("chain"), payload.ChainPayload));

export const chain = chainKept.get;

export const forgetChain = chainKept.forget;

export const fold = (chain?: string): Promise<FoldPayload> => http.get(onChain("fold", chain), payload.FoldPayload);

export const todo = (id: string): Promise<TodoPayload> =>
  http.get(onChain(`todo/${encodeURIComponent(id)}`), payload.TodoPayload);

/** Who the gate says we are on this chain: `actor: null` is a viewer, who gets no act buttons. */
export const me = (): Promise<MePayload> => http.get(onChain("me"), payload.MePayload);

/** Not per chain: the build that is running. */
export const about = (): Promise<AboutPayload> => http.get("/api/about", payload.AboutPayload);
export const getReleases = (): Promise<ReleasesPayload> => http.get("/api/releases", payload.ReleasesPayload);

/** The objects outside the ancestry of every head in `since` the box holds; none asks for all. */
export const objectsSince = (since: heads.Heads, chain?: string): Promise<ObjectsPayload> =>
  http.get(
    since.length === 0 ? onChain("objects", chain) : onChain(`objects?since=${since.map(encodeURIComponent).join(",")}`, chain),
    payload.ObjectsPayload,
  );

// The heads our own last sync produced, so `/events` does not announce our write back; asking consumes them.
let ourHeads: heads.Heads | null = null;

export const wasOurWrite = (h: heads.Heads): boolean => {
  if (ourHeads === null || !heads.same(ourHeads, h)) return false;
  ourHeads = null;
  return true;
};

export const ours = (h: heads.Heads): void => {
  ourHeads = heads.of(h);
  forgetChain();
};

/** Literals only: the box rehashes each, since a name is what the bytes are, never what a client says. */
export const postObjects = (objects: readonly string[]): Promise<ReceiptPayload> =>
  http.act(onChain("objects"), { objects: [...objects] } satisfies PostedObjects, payload.ReceiptPayload).then(receipt => {
    if (receipt.heads.length > 0) ours(receipt.heads);
    return receipt;
  });

// The authoring a replica cannot do for itself. It appends nothing: sealing the answer is the replica's job,
// and no body names an actor, which the box takes from the peer the gate named.
const author = (body: Authoring): Promise<AuthoredPayload> => http.act(onChain("author"), body, payload.AuthoredPayload);

export const authorTodo = (todo: NewTodo): Promise<AuthoredPayload> => author({ add: todo });
export const authorConfirm = (todo: string, note = ""): Promise<AuthoredPayload> => author({ confirm: { todo, note } });
/** Only the fields the caller sets travel; the rest are untouched on the box's own reading. */
export const authorUpdate = (edit: UpdateTodo, note = ""): Promise<AuthoredPayload> => author({ update: { ...edit, note } });
// Undo asks the box even for a lifecycle event: undoing an edit needs the whole stream's folds, which the wasm lacks.
export const authorUndo = (todo: string): Promise<AuthoredPayload> => author({ undo: todo });
export const authorRedo = (todo: string): Promise<AuthoredPayload> => author({ redo: todo });

// --- the secretary's stores, as replicas ---------------------------------------

/** A piece of what a replica of the inbox or the lines holding `since` lacks, the keys of the words it sends, and how much is left.
 * Bounded by silence, not length: one object may be larger than a piece, which a phone on a slow network takes longer than `wait.MS` to pull. */
export const storeObjects = (store: string, since: heads.Heads): Promise<StorePayload> =>
  http.get(
    `/api/stores/${encodeURIComponent(store)}/objects${since.length === 0 ? "" : `?since=${since.map(encodeURIComponent).join(",")}`}`,
    payload.StorePayload,
    { type: "Silence", ms: wait.MS },
  );

// --- the Inbox ---------------------------------------------------------------

/** The Inbox badge's count, read on the box. */
export const waitingCount = (): Promise<WaitingPayload> => http.get("/api/waiting", payload.WaitingPayload);

/** Every batch standing against one chain. */
export const proposals = (chain?: string): Promise<ProposalsPayload> =>
  http.get(onChain("proposals", chain), payload.ProposalsPayload);

/** The set must be ancestor-closed within its batch; the page closes it with each row's `needs`, and the box refuses an open one. */
export const acceptProposals = (chain: string, objects: readonly string[]): Promise<AcceptedPayload> =>
  http.act(onChain("proposals/accept", chain), { objects: [...objects] } satisfies PickedObjects, payload.AcceptedPayload);

/** The one delete this app asks for, allowed because nothing anyone holds rests on a batch's objects. */
export const rejectProposals = (chain: string, objects: readonly string[]): Promise<RejectedPayload> =>
  http.act(onChain("proposals/reject", chain), { objects: [...objects] } satisfies PickedObjects, payload.RejectedPayload);

// --- mail and briefings ------------------------------------------------------

const paged = (account: string, before: string): string =>
  (account === "" ? "" : `&account=${encodeURIComponent(account)}`) +
  (before === "" ? "" : `&before=${encodeURIComponent(before)}`);

/** Newest first; `before` is the ISO date of the oldest row shown, and "" asks for the newest. */
export const inbox = (account: string, limit: number, before = ""): Promise<InboxPayload> =>
  http.get(`/api/mail/inbox?limit=${limit}${paged(account, before)}`, payload.InboxPayload);

/** A notmuch query, passed through, paged by `before` like the inbox. */
export const mailSearch = (q: string, account: string, limit: number, before = ""): Promise<MailSearchPayload> =>
  http.get(`/api/mail/search?q=${encodeURIComponent(q)}&limit=${limit}${paged(account, before)}`, payload.MailSearchPayload);

/** By notmuch id or by a todo's `source.hash`: the box resolves both. */
export const mailMessage = (id: string): Promise<MailMessage> =>
  http.get(`/api/mail/message/${encodeURIComponent(id)}`, payload.MailMessage);

export const mailThread = (id: string, limit: number): Promise<ThreadPayload> =>
  http.get(`/api/mail/thread/${encodeURIComponent(id)}?limit=${limit}`, payload.ThreadPayload);

export const briefings = (): Promise<BriefingsPayload> => http.get("/api/briefings", payload.BriefingsPayload);

export const briefing = (date: string): Promise<BriefingPayload> =>
  http.get(`/api/briefings/${encodeURIComponent(date)}`, payload.BriefingPayload);
export const calendar = (date: string): Promise<CalendarPayload> =>
  http.get(`/api/calendar/${encodeURIComponent(date)}`, payload.CalendarPayload);

// --- usage and settings ------------------------------------------------------

export const usage = (): Promise<UsagePayload> => http.get("/api/usage", payload.UsagePayload);

export const getOverview = (): Promise<Overview> => http.get("/api/overview", payload.Overview);

// --- the secretary ----------------------------------------------------------
//
// Not under `app.apiBase`: a conversation reads every chain and the mail, so it is no chain's.

const conversationPath = (id: string, rest = ""): string =>
  `/api/secretary/conversations/${encodeURIComponent(id)}${rest}`;
const proposalPath = (id: string, index: number, verb: string): string =>
  conversationPath(id, `/proposals/${index}/${verb}`);

/** A blank title is left for the box to name after the first answer. */
export const openConversation = (title: string): Promise<ConversationPayload> =>
  http.act("/api/secretary/conversations", { title } satisfies Titling, payload.ConversationPayload);

export const renameConversation = (id: string, title: string): Promise<ConversationPayload> =>
  http.act(conversationPath(id, "/title"), { title } satisfies Titling, payload.ConversationPayload);

/** The owner's tap: the turns' words destroyed on the box and, at their next pull, on every device. */
export const eraseTurns = (id: string, turns: readonly number[]): Promise<ConversationPayload> =>
  http.act(conversationPath(id, "/erase"), { turns: [...turns] } satisfies Erasing, payload.ConversationPayload);

// The chips travel by value, not inside the sentence: the box parses each, so one it cannot resolve is a 400 rather than a dead link.
/** 202 with the person's turn on it; the model runs behind that. `after` is how many turns its asker saw, so the
 * box answers the same question asked twice with the first one's line. */
export const say = (
  id: string,
  text: string,
  references: readonly Reference[] = [],
  attachments: readonly AttachmentRow[] = [],
  after?: number,
): Promise<SayPayload> =>
  http.act(conversationPath(id, "/say"), { ...asked(text, references, attachments), ...(after === undefined ? {} : { after }) } satisfies Saying, payload.SayPayload);

/** Replace the last round's question: `say`'s body, the old question and all after it set aside first. */
export const editQuestion = (
  id: string,
  text: string,
  references: readonly Reference[] = [],
  attachments: readonly AttachmentRow[] = [],
): Promise<ConversationPayload> =>
  http.act(conversationPath(id, "/edit"), asked(text, references, attachments), payload.ConversationPayload);

const asked = (text: string, references: readonly Reference[], attachments: readonly AttachmentRow[]): Saying => ({
  say: text,
  references: references.map(print),
  attachments: attachments.map(({ id, name }) => ({ id, name })),
});

/** A 409 in the box's words when nothing runs. */
export const stopAnswer = (id: string): Promise<ConversationPayload> =>
  http.act(conversationPath(id, "/stop"), {}, payload.ConversationPayload);

export const regenerate = (id: string): Promise<ConversationPayload> =>
  http.act(conversationPath(id, "/regenerate"), {}, payload.ConversationPayload);

/** A conversation's proposals drawn against the todo chains, which no replica in the tab holds whole: each the box's whole row, in one read. */
export const drawnProposals = (id: string, indices: readonly number[]): Promise<DrawnPayload> =>
  http.get(conversationPath(id, `/drawn?indices=${indices.join(",")}`), payload.DrawnPayload);

/** A block of an answer, streaming or stored, named by its sha and never by its text, so the box renders only what the model wrote. */
export const block = (id: string, sha: string): Promise<BlockRender> =>
  http.get(conversationPath(id, `/blocks/${encodeURIComponent(sha)}`), payload.BlockRender);

/** A block by its source: the route names it by the sha-256 of its source. */
export const blockBySource = async (id: string, source: string): Promise<BlockRender> =>
  block(id, [...new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(source)))].map((b) => b.toString(16).padStart(2, "0")).join(""));

/** Base64, since the write gate takes JSON only; a `say` then names the id. `private` is the owner's
 * say-so for a file the box held back: it goes to his Drive, not the box. */
export const upload = (base64: string, name: string, keepPrivate = false): Promise<Uploaded> =>
  http.act("/api/secretary/attachments", { file: base64, name, private: keepPrivate } satisfies Uploading, payload.Uploaded);

/** A URL for an `<img src>`, never a fetch. */
export const attachmentUrl = (id: string): string => `/api/secretary/attachments/${encodeURIComponent(id)}`;

/**
 * The one call that ends in an irreversible act. It carries an index, never a body that could change what is
 * performed; `note` is a line for the conversation, and `value` belongs to a secret act alone.
 */
/** A send answers in seconds; past this the card asks the box what happened. */
export const TAP_BOUND = 30_000;

export const tapProposal = (
  id: string,
  index: number,
  note = "",
  value?: string,
  always = false,
  endorsement?: { readonly key: string; readonly signature: string },
): Promise<ConversationPayload> =>
  http.act(
    proposalPath(id, index, "tap"),
    // `always` and `endorsement` only when set: every other tap's body stays what it was.
    {
      note,
      ...(value === undefined ? {} : { value }),
      ...(always ? { always } : {}),
      ...(endorsement === undefined ? {} : { endorsement }),
    } satisfies Tapping,
    payload.ConversationPayload,
    TAP_BOUND,
  );

/** Behind the tap's own gate; the reason goes into the transcript the secretary reads. */
export const dismissProposal = (id: string, index: number, reason: string): Promise<ConversationPayload> =>
  http.act(proposalPath(id, index, "dismiss"), { reason } satisfies Reasoning, payload.ConversationPayload);

/** The owner has checked Sent for a send that may have gone; it performs nothing. */
export const acknowledgeProposal = (id: string, index: number): Promise<ConversationPayload> =>
  http.act(proposalPath(id, index, "acknowledge"), {}, payload.ConversationPayload);

/** One todo linking a reported coding session's branch; nothing is merged. */
export const handSession = (id: string, index: number): Promise<ConversationPayload> =>
  http.act(proposalPath(id, index, "hand"), {}, payload.ConversationPayload);

/** Let a coding session go, with the reason the secretary reads. */
export const dropSession = (id: string, index: number, reason: string): Promise<ConversationPayload> =>
  http.act(proposalPath(id, index, "drop"), { reason } satisfies Reasoning, payload.ConversationPayload);

export const getMemory = (): Promise<MemoryPayload> => http.get("/api/secretary/memory", payload.MemoryPayload);

/** A device asks to be trusted: the box files one Inbox card, which a tap from a trusted device answers. */
export const askTrust = (key: string): Promise<{ proposed: number }> =>
  http.act("/api/act/key", { key } satisfies KeyAsked, d.obj({ proposed: d.num }));

/** The code `suzatary device pair` printed at the box: the box trusts the key that signs this request, and answers it. */
export const pairDevice = (code: string): Promise<{ trusted: string }> =>
  http.act("/api/act/pair", { code } satisfies Pairing, d.obj({ trusted: d.str }));

export const forgetFact = (id: number): Promise<MemoryPayload> =>
  http.act(`/api/secretary/memory/${id}/forget`, {}, payload.MemoryPayload);
