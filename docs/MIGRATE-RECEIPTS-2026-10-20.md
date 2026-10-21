# Carrying the conversations' receipts into their tool lines

Before this release a tool line's shape, the part of a conversation line stored in the clear,
held `refused: bool`. The fact a `remember` call wrote, its receipt, was read back out of the
answer's words, which are locked. So when the owner erased those words, the receipt went with
them.

The shape now holds an `Outcome`: `Answered`, `Refused`, or `Remembered { fact }`
(`suzatary_core::secretary::outcome`). The receipt is read from the shape alone, so it still reads
once the words are gone. This build's reader does not read a tool line in the old shape, so a
server of this build refuses to start while the conversations hold one, and names the verb
(`migrate_receipts::uncarried`, asked by `suzatary serve`). The conversations are carried once,
with the owner's yes, by `suzatary secretary migrate-receipts [--dry]`
(`app/cli/src/commands/migrate_receipts.rs`).

## What the run does

- It reads every object file of the conversations as a literal, past the lines' reader, since
  that reader refuses the old shape.
- It carries each tool line once:
  - a refused one is `Refused`;
  - an answer of any verb but `remember` is `Answered`;
  - a `remember` answer whose words still open is `Remembered { fact }` with the number its
    answer names, or `Answered` for a card's filing answer, which names a card and no fact;
  - a `remember` answer erased by forgetting its fact is `Remembered { fact }`, the fact the
    erasure names;
  - a `remember` answer the owner erased, or whose key is gone, names no fact any more. It is
    carried as `Answered`, and the run prints `receipt lost:` for it.
- A line's words are locked under a binding that holds the line's whole print but its words.
  So every carried line has its slots opened with their keys under the old binding and locked
  again, each under a fresh key, bound to the new line. A slot whose key is gone, or named by an
  erasure the sweep has not acted on yet, is carried as stored. The run prints
  `carried as stored:` for each such slot. Its old key, if the key store still holds one, stays
  for the sweep.
- It rewrites the store as its image under its own genesis (`suzatary_core::store::image`).
  A conversation with no tool line keeps every object's bytes and name. In a conversation with
  one, the tool line and every line after it gets a new name, so each carried line records the
  name it replaced (`Carried::From`). The digest reads that name as the line's leaf, so every
  summary keeps its node. The turn order and every proposal's id are unchanged.
- It writes each new object's keys and then the object, parents first. Then it removes the old
  keys it replaced, then the old objects, children first.
- A run stopped part way is finished by running it again. An object whose `Carried::From` names
  an object the store still holds was written by the stopped run, and the next run takes it as
  that object's image instead of drawing new keys. A run stopped between an object's keys and
  the object leaves keys no line names, which open nothing, as the store's own writer may. A
  store with nothing to carry writes nothing, so running it again does nothing.
- After writing, the conversations must report no finding. The box no longer holds a head it
  served, so the conversations' epoch steps on the next answer (`suzatary_core::epoch`), and
  every device drops its copy once and takes the conversations again whole.
- Nothing in the run stops a server already running, and the server that runs before the
  deploy is the previous build's, which knows nothing of this run. So step 5 stops the server
  and the timers by hand, and they stay stopped until the deploy (step 9). The new build's
  server, which the deploy starts, refuses to start until the run is done.
- `--dry` prints the same lines and writes nothing. It draws keys to plan with and keeps none.

It prints one line per tool line it carries (`answered:`, `refused:`, `remembered fact <n>:`,
`receipt lost:`), one `carried as stored:` line per slot it could not open, a summary of what
it writes, removes and replaces, and one `removed:` line per object removed.

## The run order

`<data>` is the data repository's checkout, `dirname "$(realpath "$SUZATARY_INSTANCE")"`.
`<conversations>` is the instance's `[secretary] conversations`. The lines are
`<conversations>-lines/` and their keys `<conversations>-lines-keys/`, both beside it.
`suzatary` is the new build's binary.

1. Build the rev without moving anything.
2. Prove the run on a scratch copy, never the live tree. In one shell:

   ```sh
   data=<data>; lines=<conversations>-lines; keys=<conversations>-lines-keys
   scratch=$(mktemp -d)/data
   ( cd "$data" && find "$lines/objects" "$keys" -type f | sort | xargs sha256sum ) \
     > "$scratch.live-before"
   cp -a "$data" "$scratch"
   sed -i "s|$data|$scratch|g" "$scratch/instance.toml"
   ! grep -nF "$data" "$scratch/instance.toml"   # must print nothing
   export HOME="$scratch.home" XDG_STATE_HOME="$scratch.home/state" \
     XDG_CACHE_HOME="$scratch.home/cache" XDG_RUNTIME_DIR="$scratch.home/run"
   export SUZATARY_INSTANCE="$scratch/instance.toml"
   mkdir -p "$XDG_RUNTIME_DIR"
   suzatary --instance "$SUZATARY_INSTANCE" doctor   # names no path under $data
   ```

   The keys are box-local, and the copy must hold them: check that
   `"$scratch/$keys"` holds as many files as `"$data/$keys"`. Against the copy, run
   `secretary migrate-receipts --dry`, `secretary migrate-receipts`, `verify`, and
   `secretary migrate-receipts` again. The run and `verify` must print no finding, and the
   second run must print `nothing written`. Read every `receipt lost:` line: each is a
   `remember` answer whose words are gone and no forget erased, and the fact it wrote is still in the memory, only no
   longer shown under that turn. Then check that the live store did not move:

   ```sh
   ( cd "$data" && sha256sum -c "$scratch.live-before" ) > /dev/null && echo "live store unchanged"
   ```

   Leave that shell, and delete the scratch copy with its keys (`rm -rf "$scratch"
   "$scratch.home"`), before step 5.
3. The owner's yes: this is a live data migration.
4. Commit and push the data repository first, so the migration's commit holds nothing
   else: `scripts/safe-push.sh` leaves the tree clean.
5. Stop everything that writes the conversations. From an ssh shell, `systemctl --user` needs
   the user's bus first: `export XDG_RUNTIME_DIR=/run/user/$(id -u)`. Then:

   ```sh
   systemctl --user stop suzatary-web suzatary-mail-idle suzatary-triage.timer \
     suzatary-plugins.timer
   ```

   Wait until `systemctl --user is-active suzatary-triage.service` answers `inactive` or
   `failed`. Nothing refuses a run beside a running server, so check that
   `systemctl --user is-active suzatary-web` answers `inactive` before step 6.
6. Run it against the live instance. Keep its output whole and check its exit status (a pipe
   into `tee` would answer `tee`'s status, not the run's):

   ```sh
   suzatary secretary migrate-receipts > /tmp/migrate-receipts.out; echo "exit $?"
   ```

   It must say `exit 0`, and the file ends `conversations: carried, and they verify` or
   `conversations: nothing written`. On any other exit, run it again; it finishes a stopped run.
7. `suzatary verify` names nothing under the conversations. Then
   `git -C <data> status --short -- <conversations>-lines/` names the removed objects as ` D`
   and the new ones as `??`, as many as the summary line said, and nothing modified. The keys
   directory is not tracked, so it shows nothing.
8. The migration's one data commit, under the push lock `safe-push.sh` takes. `safe-push.sh`
   refuses a deletion of a stored object on the routine path, which is right, so this commit
   is made by hand:

   ```sh
   cd <data>
   exec 9>"$(git rev-parse --git-dir)/suzatary-push.lock"; flock 9
   git add -A "<conversations>-lines/objects/"
   git commit -m "conversations: receipts carried into their tool lines (secretary migrate-receipts)"
   git push || { git pull --rebase && git push; }
   exec 9>&-
   ```

   `git show --stat HEAD` holds only the conversations' objects.
9. `scripts/deploy.sh <rev>`, which restarts the web on the new build. Its first answer of
   the conversations is in the stepped epoch, so every device takes them again whole once.
10. Start the timers again:

    ```sh
    systemctl --user start suzatary-mail-idle suzatary-triage.timer suzatary-plugins.timer
    ```

The vault's next daily copy holds the new keys. Once this has run on the box, a later series
deletes the verb, its module, its test and this run order, and with them the refusal in
`suzatary serve`.
