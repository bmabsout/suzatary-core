// The secretary beside every page but its own at 62rem and up: a `chat.mount` in its own scroll box, open by the
// owner's rule (`aside.ts`); closed, only its toggle shows, in the same place. Below that width it is out of the
// document with no source open, not merely hidden.

import ui from "../app/ui";
import registers from "../app/registers";
import aside from "./aside";
import chat from "./chat";
import { clear, el } from "../view/dom";

const DEFAULT_CONVERSATION = "default";
const presses = ui.cell(registers.panel);

/** The header: "About <ref> ·", or "Nothing about <ref> yet ·" when nothing in the conversation is about it, beside a link to the whole conversation. */
function drawFrameHeader(target: HTMLElement, aboutText: string | undefined, fallback: boolean): void {
  clear(target);
  if (fallback) {
    target.appendChild(el("span", {}, `Nothing about ${aboutText ?? ""} yet · `));
    target.appendChild(el("a", { href: "#/secretary/default" }, "whole conversation →"));
    return;
  }
  target.appendChild(el("span", {}, aboutText === undefined ? "" : `About ${aboutText} · `));
  target.appendChild(el("a", { href: "#/secretary/default" }, "Whole conversation →"));
}

interface Chrome {
  readonly root: HTMLElement;
  readonly toggle: HTMLElement;
  /** Refilled on every mount, since `frameHeaderLine` is a new node each time. */
  readonly header: HTMLElement;
  readonly scrollBox: HTMLElement;
  readonly composeSlot: HTMLElement;
}

/** A conversation beside a page: `talk` is whether it holds talk about the page, and `page` moves between pages of many things. */
interface Mounted {
  readonly chrome: Chrome;
  page: aside.Page;
  talk: aside.Talk;
  readonly chat: chat.Chat;
  readonly unheard: (() => void)[];
}

type State = { readonly type: "Unmounted" } | { readonly type: "Mounted"; readonly m: Mounted };

/** The module's one mutable cell: `chrome` lives as long as the app once built, `state` is replaced whole on every remount. */
const cell: { chrome: Chrome | null; state: State } = { chrome: null, state: { type: "Unmounted" } };

function apply(m: Mounted): void {
  const open = aside.shown(m.page, presses.get(), m.talk);
  m.chrome.root.classList.toggle("panelclosed", !open);
  m.chrome.toggle.setAttribute("aria-expanded", open ? "true" : "false");
  m.chrome.toggle.textContent = open ? "Hide" : "Secretary";
}

function ensureChrome(): Chrome {
  if (cell.chrome !== null) return cell.chrome;
  const toggle = el("button", { type: "button", class: "panelcollapse" });
  const scrollBox = el("div", { class: "panelscroll" });
  const composeSlot = el("div", { class: "panelcompose" });
  const header = el("div", { class: "panelheader" });
  const body = el("div", { class: "panelbody" }, header, scrollBox, composeSlot);
  const root = el("aside", { class: "secretarypanel", "aria-label": "Secretary" }, toggle, body);
  document.body.appendChild(root);
  cell.chrome = { root, toggle, header, scrollBox, composeSlot };
  return cell.chrome;
}

function unmount(): void {
  if (cell.state.type !== "Mounted") return;
  cell.state.m.unheard.forEach((u) => u());
  cell.state.m.chat.destroy();
  cell.state = { type: "Unmounted" };
}

// The panel's stream goes with the document, as main.ts's own `pagehide` does.
window.addEventListener("pagehide", unmount);

function mount(chrome: Chrome, page: aside.Page): void {
  const about = page.type === "One" ? page.reference : undefined;
  const aboutText = page.type === "One" ? page.key : undefined;
  const frameHeaderLine = el("div", { class: "frameheader" });
  clear(chrome.header);
  chrome.header.appendChild(frameHeaderLine);
  const talk = chat.mount({
    id: DEFAULT_CONVERSATION,
    ...(about === undefined ? {} : { about }),
    seeded: about === undefined ? [] : [about],
    focus: false,
    scroll: chrome.scrollBox,
    place: (transcript, composer) => {
      chrome.scrollBox.replaceChildren(transcript);
      chrome.composeSlot.replaceChildren(composer);
    },
    loaded: (state) => drawFrameHeader(frameHeaderLine, aboutText, state.fallback),
  });
  const m: Mounted = { chrome, page, talk: "Quiet", chat: talk, unheard: [] };
  chrome.toggle.onclick = () => presses.set(aside.pressed(m.page, presses.get(), m.talk));
  m.unheard.push(
    presses.hear(() => apply(m)),
    talk.source.subscribe((state) => {
      if (state.kind !== "loaded" || aside.talk(state) === m.talk) return;
      m.talk = aside.talk(state);
      apply(m);
    }),
  );
  cell.state = { type: "Mounted", m };
  apply(m);
}

/** Remounts only when the conversation's slice moved, so an unchanged conversation keeps its node. */
export function updatePanel(placement: aside.Placement): void {
  if (placement.type === "Absent") {
    unmount();
    cell.chrome?.root.remove();
    return;
  }
  const chrome = ensureChrome();
  if (!chrome.root.isConnected) document.body.appendChild(chrome.root);
  // Two pages of many things show the same whole conversation, so moving between them keeps it and moves the page.
  const about = (page: aside.Page): string => (page.type === "One" ? page.key : "");
  if (cell.state.type === "Mounted" && about(cell.state.m.page) === about(placement.page)) {
    cell.state.m.page = placement.page;
    return apply(cell.state.m);
  }
  unmount();
  mount(chrome, placement.page);
}
