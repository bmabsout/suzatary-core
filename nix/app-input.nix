# The app a user repository runs: the `suzatary` input it declares in its
# flake.nix (WHERE the app comes from: a publication, suzatary-core's master) and
# the commit of it its flake.lock names (WHICH version runs).
#
# A function of those two files alone, so reading it never copies the user
# repository (its stores) into the Nix store. The roots refresh asks for the
# locked tree; scripts/deploy.sh asks where the declared input comes from and
# for it pinned at a revision.
#
#   nix eval --raw --file nix/app-input.nix --argstr lock <user>/flake.lock source.outPath
#   nix eval --raw --file nix/app-input.nix --argstr flake <user>/flake.nix --argstr rev <sha> [--argstr ref <branch>] pinned
{ lock ? null, flake ? null, rev ? null, ref ? null }:
let
  need = what: arg: if arg == null then throw "${what} needs --argstr ${what}" else arg;
  nodes = (builtins.fromJSON (builtins.readFile (need "lock" lock))).nodes;
  input = nodes.${nodes.root.inputs.suzatary};
  # The flake is read as the attribute set it is, for its inputs alone: no
  # output is evaluated and nothing beside it is copied.
  declared = builtins.parseFlakeRef (import (need "flake" flake)).inputs.suzatary.url;
  branch = if ref != null then ref else declared.ref or "master";
in
{
  # In the store already when the lock was written on this box: the narHash
  # names it, so nothing is fetched.
  source = builtins.fetchTree input.locked;
  # The commit the lock runs, the remote it came from and the branch it was
  # pinned from: what the roots run checks against the branch this box
  # follows (scripts/followed.sh), and a rollback deploys again.
  revision = input.locked.rev;
  lockedUrl = input.locked.url;
  lockedRef = input.original.ref or "master";

  # Where the declared input is fetched from and the branch it follows:
  # scripts/deploy.sh resolves a rev there itself (one of the branch's commits,
  # or a source a commit of it publishes), since Nix's fetch answers from its
  # cache any rev it has ever fetched.
  remote = declared.url;
  inherit branch;

  # The declared input at `rev`, on its own branch or the one `ref` names.
  pinned =
    if declared.type != "git" then
      throw "the suzatary input is a ${declared.type} input; a deploy pins a git one"
    else
      "git+${declared.url}?ref=${branch}&rev=${need "rev" rev}";
}
