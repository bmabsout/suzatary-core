/**
 * The Prodrome's `typst-wasm` glue, declared: a build output copied beside the bundle (`build-web.sh`), so
 * `import("./typst.js")` resolves here for `tsc` while esbuild keeps the specifier external. The names are
 * the Rust ones, documented in the Prodrome's `typst-wasm/src/lib.rs`.
 */

/** `{html, diagnostics}` in JSON: `html` is `null` when an error stopped the compile. */
export function compile_html(source: string, files_json: string): string;

/** Every face in one font file, for every later compile; answers how many it held. */
export function add_font(data: Uint8Array): number;

/** Instantiate the module once per tab: from a response it streams, from bytes it does not. */
export default function init(init: { module_or_path: Response | BufferSource }): Promise<unknown>;

/** `[{from, to, tag}]` in JSON, UTF-16, sorted and disjoint: the syntax highlighting of `source`. */
export function highlight(source: string): string;

/** A world kept between compilations; `restricted()`'s library has no `html` and no `plugin`. */
export class Project {
  constructor();
  static restricted(): Project;
  /** Set the file at `key`; `undefined`, or why the key names no file. */
  set(key: string, text: string): string | undefined;
  remove(key: string): boolean;
  /** `compile_html`'s answer for the file at `main`, `state` read as `/state.json`. */
  compile(main: string, state: string): string;
  /** `{from, items}` in JSON, or `null`. */
  complete(key: string, cursor: number, explicit?: boolean): string;
  /** `{kind, text}` in JSON, or `null`. */
  hover(key: string, cursor: number): string;
  free(): void;
}
