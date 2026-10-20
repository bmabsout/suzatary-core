// Time to cards on screen, from the cache and from the network, printed once each in a `--dev` build;
// `__DEV__` is an esbuild define, so production folds it away.

/** The first time is the measurement; a later redraw would bury it. */
const said = new Set<string>();

export function timed(label: string): void {
  if (!__DEV__ || said.has(label)) return;
  said.add(label);
  console.log(`suzatary perf: ${label} at ${performance.now().toFixed(1)} ms`);
}
