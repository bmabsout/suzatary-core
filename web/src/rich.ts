/**
 * The one door from a string to markup; `web-static.sh` refuses any other markup-parsing assignment.
 *
 * Safe because it only ever parses server-rendered typst:
 * 1. The agent cannot author markup: `escape_markup` backslashes every structural typst character, `#` included.
 * 2. Only a trusted actor's content is rendered; an untrusted record arrives as source (rendered means confirmed).
 * 3. typst HTML-escapes text, so the only elements are its own markup's.
 * 4. `render::suspicious` refuses a script element, an inline handler or an executable URL, and `harden` keeps an
 *    href only of a scheme that navigates, as the browser parses it.
 * A secretary's answer replaces 1 and 2 with `renderable`, which refuses the `html` module and `eval`;
 * a page the box printed holds every datum as a string literal. Never hand it a string a page assembled.
 * A page the tab compiles (`checked`) holds every datum in a JSON string read by a kit that evaluates only
 * markup the box itself rendered for a trusted actor (2 above), and passes the box's own tripwire
 * (`suzatary_core::tripwire`, through the core) before it is parsed.
 */

import { el } from "./view/dom";
import href from "./view/href";
import { hrefOf } from "./shell/route";

/** `#link("todo:<id>")` becomes the app's own route; an outbound anchor keeps only a scheme that navigates, opens away
 * from the app and cannot reach back through `window.opener`; any other loses its href. Done on nodes, never the string. */
function harden(root: DocumentFragment | HTMLElement): void {
  for (const anchor of root.querySelectorAll("a[href]")) {
    href.fold(href.of(anchor.getAttribute("href") ?? "", location.href), {
      Todo: (id) => {
        anchor.setAttribute("href", hrefOf({ type: "todo", id }));
        anchor.classList.add("todolink");
      },
      Outbound: () => {
        anchor.setAttribute("rel", "noopener noreferrer");
        anchor.setAttribute("target", "_blank");
      },
      Inert: () => anchor.removeAttribute("href"),
    });
  }
}

/** typst's HTML to nodes, through an inert `<template>`: nothing is fetched or run while it parses. */
function parse(html: string): DocumentFragment {
  const template = document.createElement("template");
  template.innerHTML = html;
  harden(template.content);
  return template.content;
}

/** The box's tripwire, handed in from the core: one implementation, never a copy here. */
export type Tripwire = (html: string) => string | undefined;

/** A page the tab compiled: its body's elements, or the tripwire's reason. Refused whole, never edited. */
export type Checked =
  | { readonly type: "Elements"; readonly elements: Element[] }
  | { readonly type: "Refused"; readonly reason: string };

/** From the first `<body>` to the last `</body>`, the box's `body_of`; a fragment is its own body. */
function bodyOf(document: string): string {
  const open = document.indexOf("<body>");
  const close = document.lastIndexOf("</body>");
  return open === -1 || close < open ? document : document.slice(open + "<body>".length, close);
}

/** typst's whole HTML document, compiled in this tab, through the tripwire and then the door. */
export function checked(document: string, tripwire: Tripwire): Checked {
  const reason = tripwire(document);
  return reason === undefined ? { type: "Elements", elements: [...parse(bodyOf(document)).children] } : { type: "Refused", reason };
}

/** A page typst rendered whole, as its top-level elements, so the caller can place each one. */
export function elements(html: string): Element[] {
  return [...parse(html).children];
}

/** Rendered prose, or the stored source as text when the server sent no rendering; one function, so the fallback cannot be forgotten. */
export function richOrText(rich: string | null, source: string, attrs: Record<string, string> = {}): HTMLElement {
  const box = el("div", { ...attrs, class: `${attrs["class"] ?? ""} ${rich === null ? "assource" : "rich"}`.trim() });
  if (rich === null) box.textContent = source;
  else box.appendChild(parse(rich));
  return box;
}

/** The same, inline, so a card's clamped body keeps the line-clamp's box model. */
export function richOrTextInline(rich: string | null, source: string, className: string): HTMLElement {
  const box = el("span", { class: `${className} ${rich === null ? "assource" : "rich"}` });
  if (rich === null) box.textContent = source;
  else box.appendChild(parse(rich));
  return box;
}
