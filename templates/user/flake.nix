{
  description = "A secretary's user repository: the data a Suzatary deployment keeps, and the app it runs";

  # The app, followed by branch: `nix flake lock` records the commit, and every
  # deploy after that is a commit moving it (the app's scripts/deploy.sh, which
  # locks in a scratch copy). A git input rather than a github: one, because a
  # deploy pins a branch at a revision.
  #
  # This flake is read for its lock and nothing else. Evaluating it copies this
  # repository, data and all, into the world-readable Nix store, so it has no
  # outputs to tempt anyone: `./suzatary` runs the app the lock names, and the
  # units read the lock the same way.
  #
  # The address below is a placeholder that nothing answers, so `nix flake lock`
  # refuses until it names the repository you fetch the app from (an https
  # URL): `sed -i "s|https://example.invalid/suzatary-core|<that URL>|" flake.nix`.
  inputs.suzatary.url = "git+https://example.invalid/suzatary-core?ref=master";

  outputs = _: { };
}
