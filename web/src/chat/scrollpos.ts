/** Whether the reader is at the bottom, from a scroll container's three numbers. */

/** Never negative: a container shorter than its viewport is at the bottom. */
export function distanceFromBottom(scrollTop: number, scrollHeight: number, clientHeight: number): number {
  return Math.max(0, scrollHeight - clientHeight - scrollTop);
}

/** Stuck unless the reader scrolled up more than a screen; past that a new round shows a pill instead of pulling them down. */
export function staysAtBottom(scrollTop: number, scrollHeight: number, clientHeight: number): boolean {
  return distanceFromBottom(scrollTop, scrollHeight, clientHeight) <= clientHeight;
}
