# The operator's notes: core's settings for the gate, and where a deploy's builds come from

The root [`OPERATOR.md`](../OPERATOR.md) is the triage agent's manual. This
file is for the person who runs `scripts/train.sh`: what the public repository
`suzatary-core` (this checkout's remote `core`) must allow before
`scripts/train.sh gate --remote github` can run a train's tree half on
GitHub's runners there.

## What runs where

`gate --remote github` splits the merge gate's tree half
(`scripts/merge-gate.sh --no-live`) between the box and core:

| Step | Where | Why |
|---|---|---|
| the leak guard (`public-tree.sh --check --strict`) | the box, first | it reads the data repository's lists, and a tree that fails it sends nothing |
| `public-tree` | the box | it builds the public tree from this private repository (`public-tree.sh --store HEAD`) |
| `deployed-shape` | the box | it runs the binary `public-tree` built |
| `changelog` | the box | it compares with this repository's `origin/master` |
| `fmt`, `clippy`, `clippy-fixture`, `doc` | core, job `checks` | |
| `build`, `fence-plugin`, `test`, `plugin-fence`, `plugin-sync` | core, job `tests` | |
| `guards` | core, job `guards` | |
| `web-inputs`, `web-test`, `build-web` | core, job `web` | |
| `e2e` (with `web-inputs` and `build-web` again) | core, jobs `e2e-1` to `e2e-4` | each runs a quarter of the scenes (`scripts/e2e-part.sh`) |
| `nix-web`, `nix-module`, `release-free` | core, job `nix` | |

The box's list is `box_steps` in `scripts/train.sh`, the jobs' are the matrix
of `.github/workflows/gate.yml`, and `app/cli/tests/guards/gate_yml.rs` fails
when the two do not hold every step of the tree half between them. Every job
runs on `ubuntu-24.04-arm`: the box is aarch64, and no step needs x86_64.

The train pushes one commit, the public tree of the train commit on core's
`master`, to the branch `gate/<train sha>`, never to `master`, waits for the
run, and deletes the branch. The live half runs on the box meanwhile, as it
always does.

## The settings, once

Run `scripts/core-settings.sh` at a terminal, with `gh` logged in as an
administrator of core and `CACHIX_AUTH_TOKEN` in secretspec (`secretspec set
CACHIX_AUTH_TOKEN`, the push token of the `bmabsout` cache, scoped to that
cache). It sets:

- **Actions on**, allowing GitHub's own actions and
  `cachix/install-nix-action` and `cachix/cachix-action` alone; `gate.yml`
  pins each by its commit.
- **Every workflow but `gate.yml` disabled.** The public tree carries the
  private repository's workflows too (`quick.yml` would run on every
  publication, `update-flake-lock.yml` on its schedule).
- **The runs' token reads only** (`default_workflow_permissions=read`) and
  cannot approve a pull request. `gate.yml` asks for `contents: read` itself,
  and its checkout keeps no credential.
- **An outside contributor's run waits for an approval**
  (`all_external_contributors`). `gate.yml` runs on a push to `gate/**` alone,
  so no pull request runs it; this covers a workflow added later.
- **The secret `CACHIX_AUTH_TOKEN`**, read from secretspec and given to `gh
  secret set` on its stdin, so it is never in an argument or printed.

Nothing else in the gate sets any of these. Core's ruleset, which keeps
`master` fast-forward only, stays as it is: a gate branch is never `master`.

The credential the train pushes to core with must be allowed to push a
change under `.github/workflows/` (a fine-grained token's Workflows
permission, read and write), since every publication and every gate commit
carries `gate.yml`.

## Who can reach the Cachix token

Only a run on a push to `gate/**`, and only a person with write access to core
can push one. A fork's push runs in the fork with the fork's own secrets, and
no pull request event runs `gate.yml`. The jobs that hold the token run the
train's code: pull requests the owner reviewed and put on the train, the code
a land then deploys. What they build is pushed to the `bmabsout` cache, which
the box and the Codespace read.

## What the verdict means

A run's verdict is the tree half's, recorded under the train commit's tree as
any gate's: success is green; red only where a job's `red` step failed, which
it does only when its `scripts/merge-gate.sh` exited 1; anything else (a
cancel, a runner lost, a timeout, a job that never reached its gate, a gate
that exited 75 because it did not start) is no verdict, so the train gates
again, never lands, and never calls the tree red.

## A deploy downloads its builds

Every commit of suzatary-core's `master` is a publication
(`scripts/train.sh publish`), and each one runs
`.github/workflows/publication.yml` on GitHub's runners: it builds, for the
box's system (aarch64-linux), the app, the web bundle and the plugins, and
pushes them to the `bmabsout` Cachix, which the box substitutes from (its
NixOS configuration lists it).

`scripts/deploy.sh <rev>` waits for that run on the commit it pins before it
builds, and says which way it went:

- the store holds the builds already (the merge gate's `deploy-build` made
  them): no wait;
- the run succeeded: the build downloads every output;
- the run failed, was still going after `SUZATARY_DEPLOY_CACHE_TIMEOUT`
  seconds (1200, about what a build here takes), never appeared within `SUZATARY_DEPLOY_CACHE_APPEAR`
  seconds (120), or GitHub could not be asked: the box builds them itself,
  as it always did.

A commit deployed with `--ref <branch>` is not on `master`, so no run builds
it and the box does.
