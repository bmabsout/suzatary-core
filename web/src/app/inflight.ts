// The writes on their way: a reload the page asks for itself waits until each has been kept or refused, so an act
// sealed into the replica or asked of the box is never cut off halfway.

export namespace inflight {
  const going = new Set<Promise<unknown>>()

  export const held = <A>(p: Promise<A>): Promise<A> => {
    going.add(p)
    const done = (): void => void going.delete(p)
    p.then(done, done)
    return p
  }

  // Writes started while it waits are waited for too.
  export const settled = async (): Promise<void> => {
    while (going.size > 0) await Promise.allSettled([...going])
  }
}
export default inflight
