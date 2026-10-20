// A trusted device vouching for a new one (SPEC law 42): its key's signature of the registration the card asks for,
// read off this tab's replica of the first chain, which is where the box keeps it.

import app from "../app/app"
import { keyAdded, loadCore } from "./core"
import { CHAIN, heldRow } from "./objects"
import signer from "./signer"

export namespace vouch {
  export type Endorsement = { readonly key: string; readonly signature: string }

  export const endorsement = async (actor: string, key: string): Promise<Endorsement> => {
    const first = app.now().served[0]
    const read = first === undefined ? null : await heldRow(CHAIN, first)
    if (read?.type === "Unreadable") throw new Error(`what this device keeps of the first chain does not read (${read.why}), so it cannot vouch`)
    const replica = read?.type === "Held" ? read.v : null
    if (replica === null || replica.objects.length === 0) throw new Error("this device holds no copy of the first chain yet, so it cannot vouch")
    const held = await signer.held()
    if (held.type !== "Held") throw new Error(`this browser cannot sign (${held.e}), so it cannot vouch`)
    const added = keyAdded(await loadCore(), replica.objects, actor, key)
    return { key: held.key.public, signature: await signer.sign(held.key, added) }
  }
}
export default vouch
