/**
 * The Typst list's draw, run inside a page over states an e2e scene hands it: the page's own tree, fold, algebra,
 * door and placement, on a host of its own beside the app, each redraw timed and reported. Bundled for the browser
 * by the scene that measures (`typstview.test.ts`), never by the app's build.
 */

import type { Core } from "../src/prodrome/core";
import memo from "../src/algebra/memo";
import table from "../src/algebra/table";
import { checked } from "../src/rich";
import compiler from "../src/typst/compiler";
import draw from "../src/typst/draw";
import place from "../src/typst/place";
import type viewstate from "../src/typst/state";
import viewtree from "../src/typst/tree";

type Glue = typeof import("../src/typst/typst.js");

export type Redraw = {
  readonly ms: number;
  readonly tree: number;
  readonly fold: number;
  readonly place: number;
  readonly compiled: number;
  readonly computed: readonly { readonly type: string; readonly key: string | null; readonly depth: number }[];
  readonly byDepth: readonly memo.Counts[];
  readonly placed: place.Counts;
};

export type Drawer = { readonly draw: (s: viewstate.State) => Redraw; readonly host: HTMLElement; readonly done: () => void };

// A drawer with an empty cache and a host of its own, appended to the page so placement lays out as the page's does.
export const drawer = (glue: Glue, core: Core, files: compiler.Files, main: string): Drawer => {
  const named = memo.tabulatedNamer((own, kids) => core.memo_name(own, kids), table.of(table.lru(8192)));
  let compiled = 0;
  const fx: draw.Effects = { world: compiler.world(new glue.Project(), files), main, door: (h) => checked(h, core.suspicious), owned: table.of(table.lru(2048)), compiled: (n) => (compiled += n) };
  const algebra = draw.algebra(named(JSON.stringify(files), []), fx, viewtree.part);
  const cache = table.of<draw.Drawn>(table.lru(4096));
  const host = document.createElement("div");
  host.className = "typstframe";
  document.body.append(host);
  return {
    host,
    done: () => host.remove(),
    draw: (s) => {
      compiled = 0;
      const t0 = performance.now();
      const t = viewtree.of(s, named);
      const t1 = performance.now();
      const [d, r] = memo.fold(algebra, t, cache);
      const t2 = performance.now();
      const placed = place.into(host, d);
      const t3 = performance.now();
      const index = new Map<string, viewtree.Node>();
      const walk = (n: viewtree.Tree): void => (index.set(n.name, n.v.node), n.kids.forEach(walk));
      walk(t);
      const computed = r.visits
        .filter((x) => x.outcome === "Computed")
        .map((x) => {
          const n = index.get(x.name);
          return { type: n?.type ?? "", key: n === undefined ? null : viewtree.key(n), depth: x.depth };
        });
      return { ms: t3 - t0, tree: t1 - t0, fold: t2 - t1, place: t3 - t2, compiled, computed, byDepth: memo.byDepth(r), placed };
    },
  };
};
