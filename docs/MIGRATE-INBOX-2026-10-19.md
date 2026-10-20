# Carrying the inbox's withdrawals and prices

Before this release the inbox stored two things in forms the program no longer reads:

- A card the box or the secretary took back was stored as a `Dismissed`, like the owner's
  no, and told apart only by its words. A card a later one replaced said
  `superseded by proposal <n>`. A card nothing needed any more carried whatever reason the
  box or the secretary wrote, as the actor `assistant`; the box's own began
  `the box withdrew it: ` when a tap could never perform the card.
- An asking that served a todo could also carry a spec of its own. Nothing read it, since a
  proposal is priced by the todo it serves.

The inbox now stores a withdrawal as `Superseded()` or `NoLongerNeeded(why)`
(`Responded::Withdrawn`, `secretary::responded::Withdrawal`), and a price as either the
todo it serves or a spec of its own, never both (`secretary::inbox::Price`). This build's
reader leaves an asking that carries both out of the inbox, with every response resting on
it, and would read a box withdrawal as the owner's no, so a server of this build refuses to
start while the inbox holds anything the run would carry, and names the verb
(`migrate_inbox::uncarried`, asked by `suzatary serve`). It is carried once, with the owner's yes, by
`suzatary secretary migrate-inbox [--dry]` (`app/cli/src/commands/migrate_inbox.rs`).

## What the run does

- It reads every object file of the inbox as a literal, past the inbox's reader, since that
  reader refuses the old asking.
- It carries each event once, by its writer and then its words:
  - a `Dismissed` written by anyone but `assistant` is a person's no and is kept as it is,
    whatever its words;
  - a `Dismissed` written as `assistant` whose reason is `superseded by proposal <n>` becomes
    `Superseded()`;
  - any other `Dismissed` written as `assistant` becomes `NoLongerNeeded(why)`, its words
    kept, less the box's `the box withdrew it: `;
  - an `Asked`, `Noticed` or `Offered` that names a todo in `serves` and a spec in `spec`
    keeps its todo, and its spec becomes `None`.
- It rewrites the store as its image under its own genesis (`suzatary_core::store::image`).
  Every other object keeps its bytes and its name. Every object resting on a carried one is
  sealed again with its new deps. A proposal's id is never re-derived, so every
  conversation line still names its card.
- It writes the new objects first, then removes the old ones, children first. A run stopped
  part way is finished by running it again, and a store with nothing to carry writes
  nothing, so running it again does nothing.
- After writing, the inbox must report no finding. The box no longer holds a head it served,
  so the inbox's epoch steps on the next answer (`suzatary_core::epoch`), and every device
  drops its copy and takes the inbox again whole.
- Nothing in the run stops a server already running, and the server that runs before the
  deploy is the previous build's, which knows nothing of this run. So step 5 stops the server
  and the timers by hand, and they stay stopped until the deploy (step 9). The new build's
  server, which the deploy starts, refuses to start until the run is done.
- `--dry` prints the same lines and writes nothing.

It prints one line per event it carries (`superseded:`, `no longer needed:` with the words
kept, `spec dropped:`), a summary of what it writes and removes, and one `removed:` line per
object removed.

## The run order

`<data>` is the data repository's checkout, `dirname "$(realpath "$SUZATARY_INSTANCE")"`.
`<inbox>` is the inbox's directory under it: the instance's `[secretary] inbox`, `inbox` when
the file names none. `suzatary` is the new build's binary.

1. Build the rev without moving anything.
2. Prove the run on a scratch copy, never the live tree. In one shell:

   ```sh
   data=<data>; inbox=<inbox>; scratch=$(mktemp -d)/data
   ( cd "$data" && find "$inbox/objects" -type f | sort | xargs sha256sum ) > "$scratch.live-before"
   cp -a "$data" "$scratch"
   sed -i "s|$data|$scratch|g" "$scratch/instance.toml"
   ! grep -nF "$data" "$scratch/instance.toml"   # must print nothing
   export HOME="$scratch.home" XDG_STATE_HOME="$scratch.home/state" \
     XDG_CACHE_HOME="$scratch.home/cache" XDG_RUNTIME_DIR="$scratch.home/run"
   export SUZATARY_INSTANCE="$scratch/instance.toml"
   mkdir -p "$XDG_RUNTIME_DIR"
   suzatary --instance "$SUZATARY_INSTANCE" doctor   # names no path under $data
   ```

   Against the copy, run `secretary migrate-inbox --dry`, `secretary migrate-inbox`,
   `verify`, and `secretary migrate-inbox` again. The run and `verify` must print no
   finding, and the second run must print `nothing written`. Read every
   `no longer needed:` line: each is a card the box or the secretary took back, never the
   owner's no. Then check that the live inbox did not move:

   ```sh
   ( cd "$data" && sha256sum -c "$scratch.live-before" ) > /dev/null && echo "live inbox unchanged"
   ```

   Leave that shell before step 5.
3. The owner's yes: this is a live data migration.
4. Commit and push the data repository first, so the migration's commit holds nothing
   else: `scripts/safe-push.sh` leaves the tree clean.
5. Stop everything that writes the inbox. From an ssh shell, `systemctl --user` needs the
   user's bus first: `export XDG_RUNTIME_DIR=/run/user/$(id -u)`. Then:

   ```sh
   systemctl --user stop suzatary-web suzatary-mail-idle suzatary-triage.timer \
     suzatary-plugins.timer
   ```

   Wait until `systemctl --user is-active suzatary-triage.service` answers `inactive` or
   `failed`. Nothing refuses a run beside a running server, so check that
   `systemctl --user is-active suzatary-web` answers `inactive` before step 6. A timer's
   `secretary review` waits on the inbox's lock and appends after the run, onto the new heads.
6. Run it against the live instance. Keep its output whole and check its exit status (a pipe
   into `tee` would answer `tee`'s status, not the run's):

   ```sh
   suzatary secretary migrate-inbox > /tmp/migrate-inbox.out; echo "exit $?"
   ```

   It must say `exit 0`, and the file ends `inbox: carried, and it verifies` or
   `inbox: nothing written`. On any other exit, run it again; it finishes a stopped run.
7. `suzatary verify` names nothing under the inbox. Then
   `git -C <data> status --short -- <inbox>/` names the removed objects as ` D` and the new
   ones as `??`, as many as the summary line said, and nothing modified.
8. The migration's one data commit, under the push lock `safe-push.sh` takes. `safe-push.sh`
   refuses a deletion of a stored object on the routine path, which is right, so this commit
   is made by hand:

   ```sh
   cd <data>
   exec 9>"$(git rev-parse --git-dir)/suzatary-push.lock"; flock 9
   git add -A "<inbox>/objects/"
   git commit -m "inbox: withdrawals and prices carried (secretary migrate-inbox)"
   git push || { git pull --rebase && git push; }
   exec 9>&-
   ```

   `git show --stat HEAD` holds only the inbox's objects.
9. `scripts/deploy.sh <rev>`, which restarts the web on the new build. Its first answer of
   the inbox is in the stepped epoch.
10. Start the timers again:

    ```sh
    systemctl --user start suzatary-mail-idle suzatary-triage.timer suzatary-plugins.timer
    ```

Once this has run on the box, a later series deletes the verb, its module, its test and this
run order, and with them the refusal in `suzatary serve`.
