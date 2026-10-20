// Whether this page is being looked at, told to the box, which pushes the phone only when none is.
// A ping, not a held stream: a browser allows six connections per host.

/** Well inside the box's 70-second window, so one late ping is not absence. */
const PING_MS = 30_000;

export function installPresence(): void {
  let timer: ReturnType<typeof setInterval> | null = null;
  const ping = (): void => {
    void fetch("/api/presence").catch(() => {
      // Offline or refused: the box does not hear this page, and pushes the phone instead.
    });
  };
  const follow = (): void => {
    if (document.visibilityState === "visible") {
      if (timer === null) {
        ping();
        timer = setInterval(ping, PING_MS);
      }
    } else if (timer !== null) {
      clearInterval(timer);
      timer = null;
    }
  };
  document.addEventListener("visibilitychange", follow);
  follow();
}
