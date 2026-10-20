// How long a read of the box may take, its body included: a request the box never answers is cancelled, so it frees its
// connection, and fails as the box not answering. The tab's own work after it is never bounded.

export namespace wait {
  export const MS = 30_000

  export const waited = (path: string) => (e: unknown): never => {
    throw e instanceof DOMException && e.name === "TimeoutError" ? new Error(`${path}: the box did not answer in ${MS / 1000} s`) : e
  }
}
export default wait
