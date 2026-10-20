# shellcheck shell=bash
# nix_out <output>: the flake output of the checkout at `$root`, built and ROOTED under its
# gate-roots, so a collection of the store (a Codespace short of disk) keeps what the next run
# would build again; its path printed, and a failed build the caller's failure. Sourced by the
# merge gate and by scripts/web-inputs.sh, the one spelling of it.
nix_out() {
  local roots
  roots="$(git -C "$root" rev-parse --path-format=absolute --git-common-dir)/gate-roots" &&
    mkdir -p "$roots" &&
    nix build "$root#$1" --out-link "$roots/$1" --print-out-paths
}

# nix_outs <output>...: the outputs built in ONE `nix build`, so Nix builds them side by side
# where a nix_out each would build them one after another; each is then rooted and named by
# nix_out, which finds it built.
nix_outs() {
  local outputs=("${@/#/$root#}")
  nix build --no-link "${outputs[@]}"
}

# root_environment <dir>: every store path the running environment names (the shell's PATH, its
# fonts, its packages), rooted under <dir>, in place of what <dir> held. `nix develop` roots
# nothing, so the tools of the shell a gate runs in are held by no root while it runs, and a
# collection (the Codespace's room, or the daemon's short of disk) took one mid-gate on
# 2026-10-11: gitleaks, from under 14 guards. A collection keeps these paths' closures.
root_environment() {
  local dir="$1" paths
  mapfile -t paths < <(env | grep -o '/nix/store/[0-9a-z]\{32\}-[^/:"[:space:]]*' | sort -u |
    while read -r path; do [[ ! -e "$path" ]] || echo "$path"; done)
  rm -rf "$dir" && mkdir -p "$dir" &&
    { ((${#paths[@]} == 0)) || nix-store --add-root "$dir/path" --realise "${paths[@]}" >/dev/null; }
}
