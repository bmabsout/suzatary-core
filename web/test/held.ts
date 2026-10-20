// A replica made the one way one is: from what it holds, read by `replicaOf`; a box word only when the test names one.

import type { RefusedObject, StoredObject } from "../src/wire/generated/wire"
import type { HeldChain } from "../src/prodrome/core"
import type { Replica } from "../src/prodrome/replica"
import { replicaOf } from "../src/prodrome/replica"

export type Over = {
  readonly objects?: readonly StoredObject[]
  readonly synced?: readonly string[]
  readonly refused?: readonly RefusedObject[]
  readonly generation?: number
  readonly boxHeads?: readonly string[]
  readonly untrusted?: readonly string[]
  readonly root?: string | null
  readonly fetchedAt?: number
  readonly geneses?: readonly string[] | null
  readonly epoch?: number
}

export const replicaWith = (o: Over = {}): Replica =>
  replicaOf({
    generation: o.generation ?? 0,
    objects: o.objects ?? [],
    synced: o.synced ?? [],
    refused: o.refused ?? [],
    words:
      o.boxHeads === undefined && o.untrusted === undefined && o.root === undefined && o.fetchedAt === undefined
        ? []
        : [{ epoch: o.epoch ?? 1, heads: o.boxHeads ?? [], untrusted: o.untrusted ?? [], root: o.root ?? null, fetchedAt: o.fetchedAt ?? 1, geneses: o.geneses ?? null }],
  })

// A chain held whole: `objects` under the policy `untrusted`, as a pull of them would leave it.
export const heldChain = (name: string, objects: readonly StoredObject[], untrusted: readonly string[]): HeldChain => ({
  name,
  replica: replicaWith({ objects, untrusted }),
})
