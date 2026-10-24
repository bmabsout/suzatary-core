# Run order: the triage agent's open batches become Inbox cards

This page is the run order for `suzatary proposals carry`. The follow-up pull request deletes
it, together with the verb, `secretary::batch_cards` and the replica's files.

## What changes at the deploy

From the deploy on, the triage agent's `todo_add`, `todo_update` and `record` file cards in the
Inbox. The batches it wrote before the deploy stay in each chain's replica (`events-triage/`).
The app no longer draws a batch: a held batch's decision shows in the Inbox as a row that says
"batch … is not a card yet". Nothing is lost while the batches wait.

## Steps

1. Deploy the commit: `scripts/deploy.sh <rev>`.
2. Stop the triage agent until the carry is done. Otherwise a pass could file a card about a
   todo that so far exists only in a batch, and the tap of that card would refuse it. The idle
   watcher starts the triage service directly, so stop it too, then wait for a running pass:

   ```
   systemctl --user stop suzatary-triage.timer suzatary-mail-idle.service
   while systemctl --user is-active --quiet suzatary-triage.service; do sleep 5; done
   ```

3. Run the dry run at the box's terminal. It reads every chain and writes nothing:

   ```
   nix develop .#triage -c scripts/app-bin.sh proposals carry
   ```

   Each line names a chain, a todo, and either the acts its card will hold or `NOT CARRIED`
   with the reason.
4. Show the owner the dry run's output. Run the next step only after the owner says yes.
5. Carry the batches:

   ```
   nix develop .#triage -c scripts/app-bin.sh proposals carry --apply
   ```

   For each batch the verb files one triage card, completes the decision that held the batch
   (its note names the card), and drops the batch's objects from the replica. A batch marked
   `NOT CARRIED` stays in the replica and the verb goes on to the next; at the end it exits
   non-zero so the reasons are read.
6. Run step 5 again. Every chain must print `no open batch` (or only the batches step 5 named
   `NOT CARRIED`), and the Inbox must hold the same cards as before. The second run writes
   nothing.
7. Start the triage agent again:

   ```
   systemctl --user start suzatary-triage.timer suzatary-mail-idle.service
   ```

8. The next `safe-push.sh` commits the inbox, the conversations, the completed decisions and
   the replica. Check the commit before the follow-up pull request deletes the migration.

## If a batch is not carried

A batch is refused when its events cannot be written as acts again: a price that is not one of
the prices the verbs write, a subtodo reference without the child's add, or an act the chain
would refuse now. Its objects stay in the replica, and the triage agent still reads them as if
they were on the chain until the follow-up removes the replica. Tell the owner what the batch
asked for, and settle it by hand at the terminal (`suzatary todo …`) if he wants it.
