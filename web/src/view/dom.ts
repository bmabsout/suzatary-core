// The one door from a string to the page: every string may come from attacker-controlled mail, and a DOM
// node has no parser, so text goes through `textContent` and attributes through `setAttribute`; `svg` builds icons the same way.

export type Child = Node | string | number | null | undefined | false | readonly Child[];

/** `on*` values are listeners; `false`, `null` and `undefined` drop the attribute, so a conditional needs no branch. */
export type Attrs = Record<string, string | number | boolean | null | undefined | ((event: Event) => void)>;

function append(node: Element, child: Child): void {
  if (child === null || child === undefined || child === false) return;
  if (Array.isArray(child)) {
    for (const each of child) append(node, each as Child);
    return;
  }
  if (child instanceof Node) {
    node.appendChild(child);
    return;
  }
  node.appendChild(document.createTextNode(String(child)));
}

function fill<T extends Element>(node: T, attrs: Attrs | null | undefined, children: readonly Child[]): T {
  for (const [key, value] of Object.entries(attrs ?? {})) {
    if (typeof value === "function") {
      node.addEventListener(key.replace(/^on/, ""), value);
    } else if (value === null || value === undefined || value === false) {
      continue;
    } else if (value === true) {
      node.setAttribute(key, "");
    } else {
      node.setAttribute(key, String(value));
    }
  }
  for (const child of children) append(node, child);
  return node;
}

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs?: Attrs | null,
  ...children: Child[]
): HTMLElementTagNameMap[K] {
  return fill(document.createElement(tag), attrs, children);
}

const SVG_NS = "http://www.w3.org/2000/svg";

export function svg<K extends keyof SVGElementTagNameMap>(
  tag: K,
  attrs?: Attrs | null,
  ...children: Child[]
): SVGElementTagNameMap[K] {
  return fill(document.createElementNS(SVG_NS, tag), attrs, children);
}

export function clear(node: Element): void {
  node.replaceChildren();
}

/** `document.getElementById`, with a miss reported rather than a `null` three frames later. */
export function byId(id: string): HTMLElement {
  const found = document.getElementById(id);
  if (found === null) throw new Error(`no element #${id}`);
  return found;
}
