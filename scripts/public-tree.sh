#!/usr/bin/env bash
# The tree a public repository of Suzatary would start from, and the guard
# over it (docs/DATASPLIT-PLAN-2026-09-14.md §9). Publishing is the owner's
# decision: this writes a directory and reads files, and never touches a git
# remote or a repository's visibility.
#
#   scripts/public-tree.sh <out> [<rev>]          materialise the public tree of <rev> (HEAD)
#   scripts/public-tree.sh --store [<rev>]        the same, added to the Nix store as a
#                                                 deploy fetches it; prints its store path
#   scripts/public-tree.sh --check <dir>          the leak guard, every rule
#   scripts/public-tree.sh --check --full ...     the same, reading every file again (THE
#                                                 PASSES, below, are not read, and are kept;
#                                                 SUZATARY_GUARD_FULL set is the same)
#   scripts/public-tree.sh --check --strict <dir> the same, failing where a rule would skip
#   scripts/public-tree.sh --check --lists [--admitted <manifest> | --text] <dir>
#                                                 the guard another public repository's range
#                                                 passes (scripts/public-scan.sh): the secret
#                                                 scanner, the admitted files and the lists;
#                                                 `--text` for words a person wrote (a commit
#                                                 message), where nothing is admitted
#   scripts/public-tree.sh --check --terms <dir>  the data repository's lists alone (this
#                                                 private repository's own check)
#
# THE PUBLIC TREE is a FUNCTION OF A COMMIT'S TREE: <rev>'s (`git archive`, so no
# uncommitted edit in a shared checkout is ever published) MINUS the private
# history, which `.gitattributes` declares `export-ignore` (the reports, the
# CLAUDE.md history and the changelog), with `scripts/public-changelog.md` as
# its changelog: one that starts at the public release, at the workspace's
# version and the date the real changelog gives it, else `unreleased` (never
# the clock's date or the commit's, so one tree writes one tree on any day and
# from any commit holding it). Every file it reads is <rev>'s. The template is itself in
# the public tree, so the tree it writes holds what writes it again. Nothing
# else is removed, so whatever else would leak is the
# guard's to find, not a list's to hide. The public tree hosts itself: run
# inside a commit of its own output, this writes that output again.
#
# THE GUARD'S INVARIANT: every byte of <dir> is EITHER plain text read by both
# checks, the secret scanner (gitleaks) and the term matcher (`suzatary
# leaks`), OR in a file `scripts/public-admitted.sha256` lists by its path and
# the SHA-256 of its bytes, a line a person admitted at a terminal
# (`scripts/train.sh publish --admit`). Neither check decodes for the other:
# what an admitted file holds encoded is the admitting person's to see.
#
# It reads every file of <dir> (what git tracks when <dir> is a work tree's
# root, every file otherwise) and every file's path, and fails on:
#   - a symbolic link, which a public tree never holds;
#   - an email address outside the reserved example domains (`example.com`,
#     `.org`, `.net` and their subdomains, `.test`, `.invalid`, `.example`,
#     `.localhost`, or a domain whose first label is `example`), the cast's
#     being all of them; a URL's userinfo (`scheme://user@`) is not an address;
#   - a `.ts.net` name whose tailnet is not a placeholder (`example`, `tailnet`);
#   - an absolute home path other than the cast owner's (`ada`) or `root`;
#   - a path to a file of the private history this checkout holds;
#   - this deployment's box, tailnet or Unix user, read from its configuration:
#     `<data>/FETCHER`, the instance file's `app_url`, and the `/home/<user>`
#     of its roots;
#   - a term of `personal-terms.txt` or `context-terms.txt`, the two lists in
#     the box-local tier `<data>/local`, which no push carries;
#   - a tailnet node's address (100.64.0.0/10, but the cast's 100.64.0.0/24)
#     or a telephone number (but the fictional 555 exchange);
#   - outside `--terms`, a secret gitleaks finds (`secrets` below), unless the
#     tree's `.gitleaksignore` names that finding by its fingerprint;
#   - outside `--terms`, a file the matcher cannot read as words, judged over
#     the whole file (`suzatary_core::leak::Opaque`: a binary, base64 or hex
#     however it is cut up, or text spelled with escapes), or one holding a finding
#     `.gitleaksignore` admits, that the manifest does not admit with these
#     bytes (a fingerprint names a path, a rule and a line, never the bytes),
#     the manifest being <dir>'s own unless `--admitted` names another;
#   - with every rule, a relative link or picture of a front page
#     (`README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `docs/README.md`) that
#     names no file of the tree, so what they show a stranger is there.
# A path holding a newline or another control character is refused (2) before
# anything reads it. The matcher reads text as words: compatibility-decomposed,
# lowercased, without accents or Unicode's invisible format characters, every
# run of whitespace (a line break and the comment marker opening the next line
# included) one space, and a term or a name matches as a WHOLE WORD of that (a
# word character is `[a-z0-9_]`), at the line it starts on. A binary is not
# read.
# <data> is the instance file's directory (`SUZATARY_INSTANCE`, else
# `instance.toml` beside this checkout); where there is none, the rules that
# need it skip with a line, or fail under `--strict`, the check a publication
# passes. A hit prints its file and line (`:0` for a path, a link or a file
# not admitted) and nothing else, so the guard never prints what it guards.
#
# THE CHECK'S VERDICT IS ITS STATUS, as grep's is: 0 the tree is clean, 1 the
# check ran whole and found a hit, 2 it could not run whole (a rule that would
# skip under `--strict`, a list missing, a scratch file not made, anything
# that failed). Only a 1 says the tree leaks; a caller that acts on a leak
# (`scripts/train.sh publish --skip`) never reads a 2 as one.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRONT_PAGES=(README.md CONTRIBUTING.md SECURITY.md docs/README.md)

usage() {
  echo "usage: public-tree.sh <out> [<rev>] | public-tree.sh --store [<rev>] | public-tree.sh --check [--full] [--strict | --lists [--admitted <manifest> | --text] | --terms] <dir>" >&2
  exit 2
}

# The files of a tree, NUL-separated and relative to it.
files_of() {
  if [ "$(git -C "$1" rev-parse --show-toplevel 2>/dev/null)" = "$(cd "$1" && pwd -P)" ]; then
    git -C "$1" ls-files -z
  else
    (cd "$1" && find . -type f -print0 | sed -z 's#^\./##')
  fi
}

# <rev>'s public changelog at its release's version (app/release.toml; app/Cargo.toml for a
# commit from before the release left the manifest), dated as its changelog dates that version.
fresh_changelog() {
  local rev="$1" version date
  version="$( { git -C "$REPO" show "$rev:app/release.toml" 2>/dev/null \
    || git -C "$REPO" show "$rev:app/Cargo.toml"; } | sed -n 's/^version = "\(.*\)"$/\1/p' | head -1)"
  date="$(git -C "$REPO" show "$rev:CHANGELOG.md" | sed -n "s/^## \[$version\] - \(.*\)$/\1/p" | head -1)"
  [ -n "$date" ] || date=unreleased
  git -C "$REPO" show "$rev:scripts/public-changelog.md" | sed "s/@VERSION@/$version/; s/@DATE@/$date/"
}

# `page:line` for each relative link of a front page of <dir> (Markdown's
# `](target)`, HTML's `src=` and `href=`) that names no file of <dir>.
dangling_links() {
  local dir="$1" page line target path
  for page in "${FRONT_PAGES[@]}"; do
    [ -f "$dir/$page" ] || continue
    while IFS=$'\t' read -r line target; do
      case "$target" in '' | '#'* | *://* | mailto:*) continue ;; esac
      path="$(realpath -m "$dir/$(dirname "$page")/${target%%#*}")"
      case "$path" in "$(realpath "$dir")"/*) [ -e "$path" ] && continue ;; esac
      echo "$page:$line"
      echo "public-tree: $page:$line links to nothing the tree holds" >&2
    done < <(awk '{
      s = $0
      while (match(s, /\]\([^) ]+\)|(src|href)="[^"]+"/)) {
        t = substr(s, RSTART, RLENGTH); s = substr(s, RSTART + RLENGTH)
        if (t ~ /^\]/) t = substr(t, 3, length(t) - 3); else { sub(/^[a-z]+="/, "", t); sub(/"$/, "", t) }
        print FNR "\t" t
      }
    }' "$dir/$page")
  done
}

materialise() {
  local out="$1" rev
  rev="$(git -C "$REPO" rev-parse --verify --quiet "${2:-HEAD}^{commit}")" || {
    echo "public-tree: ${2:-HEAD} names no commit" >&2
    exit 1
  }
  [ ! -e "$out" ] || [ -z "$(ls -A "$out")" ] || {
    echo "public-tree: $out exists and is not empty" >&2
    exit 1
  }
  mkdir -p "$out"
  git -C "$REPO" archive --format=tar "$rev" | tar -x -C "$out"
  # A directory all of whose files are `export-ignore` is left empty, and git
  # holds no empty directory.
  find "$out" -mindepth 1 -type d -empty -delete
  fresh_changelog "$rev" >"$out/CHANGELOG.md"
  echo "public-tree: wrote $(find "$out" -type f | wc -l) files to $out" >&2
}

# THE STORE PATH OF <rev>'s PUBLIC TREE, as `nix store add-path --name source` adds it. The tree
# is a function of <rev>'s git tree and of this script, so the path is recorded under the hash of
# the two in <git-common-dir>/public-trees, and a recorded path the store still holds is printed
# without materialising the tree again.
store_path() {
  local rev tree key record path dir code=0
  rev="$(git -C "$REPO" rev-parse --verify --quiet "${1:-HEAD}^{commit}")" || {
    echo "public-tree: ${1:-HEAD} names no commit" >&2
    exit 1
  }
  tree="$(git -C "$REPO" rev-parse "$rev^{tree}")"
  key="$({ echo "$tree"; sha256sum <"$REPO/scripts/public-tree.sh"; } | sha256sum | cut -c1-40)"
  record="$(git -C "$REPO" rev-parse --path-format=absolute --git-common-dir)/public-trees/$key"
  path="$(cat "$record" 2>/dev/null || true)"
  if [ -n "$path" ] && nix-store --check-validity "$path" 2>/dev/null; then
    echo "public-tree: the public tree of ${rev:0:12} is in the store already" >&2
    echo "$path"
    return
  fi
  dir="$(mktemp -d)"
  { materialise "$dir/tree" "$rev" && path="$(nix store add-path --name source "$dir/tree")"; } || code=$?
  rm -rf "$dir"
  [ "$code" -eq 0 ] || exit "$code"
  mkdir -p "$(dirname "$record")" && echo "$path" >"$record"
  echo "$path"
}

# THE SECRET SCANNER is gitleaks, offline: its maintained rules and this project's additions
# (scripts/secret-rules.toml, the guard's own, read from this checkout as the matcher is), over
# every file of <dir> and what each holds base64-, hex- or percent-encoded, three decodings deep.
# It reads a COPY in which every file is named by a number alone, so no path rule of its own
# (its default allowlist passes over `*.svg`, lock files, `node_modules/` and a path naming its
# own config) can pass over a file, and each sealed slot the matcher recognises as ciphertext
# (`suzatary_core::leak::slots`) is a placeholder, so its key's name is no finding while the
# rest of its line is read (`suzatary leaks scan-copy`); the numbers are mapped back to paths
# in what it prints. A
# finding the tree's own `.gitleaksignore` names by its fingerprint (path, rule, line) is
# admitted, that file rewritten to the numbers, and a `gitleaks:allow` comment admits nothing.
# Each finding prints as `file:line`, never the secret, its rule on stderr. Its status is 0 or 3
# (found) when it ran; anything else is a scan that did not.
secrets() {
  local code=0
  mkdir "$found.tree"
  tr '\n' '\0' <"$found.todo" | (cd "$1" && "$bin" leaks scan-copy "$found.tree") >"$found.index" || return 2
  # A fingerprint of a path the tree holds, by its number; any other line admits nothing.
  : >"$found.ignore"
  if [ -f "$1/.gitleaksignore" ]; then
    awk -F'\t' 'NR == FNR { n[$2] = $1; next }
      { r = $0; if (match(r, /:[^:]*:[0-9]+$/) && (substr(r, 1, RSTART - 1) in n))
          print n[substr(r, 1, RSTART - 1)] substr(r, RSTART) }' \
      "$found.index" "$1/.gitleaksignore" >"$found.ignore" || return 2
  fi
  printf '{{ range . }}{{ .File }}\t{{ .StartLine }}\t{{ .RuleID }}\n{{ end }}' >"$found.template"
  (cd "$found.tree" && gitleaks dir . --config "$REPO/scripts/secret-rules.toml" --no-banner --no-color \
    --log-level error --redact --max-decode-depth 3 --ignore-gitleaks-allow --exit-code 3 \
    --gitleaks-ignore-path "$found.ignore" --report-format template \
    --report-template "$found.template" --report-path "$found.secrets") || code=$?
  [ "$code" -eq 0 ] || [ "$code" -eq 3 ] || return 2
  awk -F'\t' 'NR == FNR { p[$1] = $2; next } {
      f = $1; sub(/^\.\//, "", f)
      print p[f] ":" $2; printf "public-tree: %s:%s holds a secret (gitleaks rule %s)\n", p[f], $2, $3 > "/dev/stderr" }' \
    "$found.index" "$found.secrets"
}

# One term per line, blank and `#` lines skipped, folded to lowercase.
terms_of() {
  grep -vE '^[[:space:]]*(#|$)' "$1" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//' | tr '[:upper:]' '[:lower:]'
}

# A rule that cannot run: a line, or under --strict a check that did not run whole.
skipped() {
  if [ -n "$strict" ]; then
    echo "public-tree: $1, so the check is not whole (--strict)" >&2
    exit 2
  fi
  echo "public-tree: skipped $1" >&2
}

# The check with <rules>: `whole` (every rule), `lists` (the secret scanner, the admitted files
# and the term lists) or `terms` (the term lists alone), the manifest <admitted> or <dir>'s own.
check() {
  local rules="$1" dir="$2" manifest="$3" instance data hits repo
  instance="${SUZATARY_INSTANCE-$REPO/instance.toml}"
  terms="" words="" links="" found="" leaked="" failed=""
  # Any failure but a hit is a check that did not run: 2, never a hit's 1.
  trap 'status=$?; rm -rf "$terms" "$words" "$links" "$found" "$found".*; [ "$status" -eq 0 ] || [ -n "$leaked" ] || exit 2' EXIT
  terms="$(mktemp)"
  words="$(mktemp)"
  links="$(mktemp)"
  found="$(mktemp)"
  # THE MATCHER is the binary's (`suzatary leaks`, `suzatary_core::leak`), one for every
  # input: files, printing `file:line` (and `file:0` for one the manifest does not admit); with
  # `--paths`, one path per line on stdin. `SUZATARY_BIN` names it (a test's own build), else
  # this checkout's program, which scripts/app-bin.sh builds.
  local bin="${SUZATARY_BIN:-}"
  [ -n "$bin" ] || bin="$("$REPO/scripts/app-bin.sh")/bin/suzatary"
  [ -x "$bin" ] || { echo "public-tree: no matcher at $bin" >&2; exit 2; }
  if [ -n "$instance" ] && [ -f "$instance" ]; then
    data="$(dirname "$(realpath "$instance")")"
    # The box-local tier, `suzatary_cli::instance::Local::DIR`.
    local_dir="$data/local"
    [ -f "$local_dir/personal-terms.txt" ] || {
      echo "public-tree: $local_dir/personal-terms.txt is missing" >&2
      exit 2
    }
    terms_of "$local_dir/personal-terms.txt" >>"$terms"
    if [ -f "$local_dir/context-terms.txt" ]; then
      terms_of "$local_dir/context-terms.txt" >>"$terms"
    else
      skipped "context-terms.txt: the box-local tier has none yet"
    fi
    # Each value as the binary reads it (`suzatary instance get`): `repo`, which every instance
    # names, so a file that does not read stops the check, and `app_url` and `data` where named.
    repo="$("$bin" --instance "$instance" instance get repo)" || {
      echo "public-tree: the binary ($bin) read no repo from $instance, so the check did not run whole" >&2
      exit 2
    }
    {
      [ -f "$data/FETCHER" ] && tr -d '[:space:]' <"$data/FETCHER" && echo
      { "$bin" --instance "$instance" instance get web.app_url 2>/dev/null || true; } |
        sed -n 's#^[a-z]*://\([^/:]*\).*#\1#p' | tr '.' '\n' | grep -vxE 'ts|net|com|org|www|localhost|[0-9]+' || true
      { realpath "$instance"; echo "$repo"; "$bin" --instance "$instance" instance get data 2>/dev/null || true; } |
        sed -n 's#^/\(home\|Users\)/\([^/]*\).*#\2#p'
    } | tr '[:upper:]' '[:lower:]' | grep -E '^[a-z0-9_-]+$' | sort -u >"$words" || true
  else
    skipped "the data repository's lists and this deployment's names: no instance file"
  fi

  if [ "$rules" = whole ]; then
    # A file of the private history below the root (a root file the public
    # tree writes afresh, or never needs, is not a link).
    git -C "$REPO" ls-files | git -C "$REPO" check-attr --stdin export-ignore |
      sed -n 's/: export-ignore: set$//p' | grep / >"$links" || true
  fi

  # A path holding a newline or another control character is refused before anything reads
  # it: every reader after this one (the matcher, the train's paths) reads lines.
  local odd
  odd="$(files_of "$dir" | while IFS= read -r -d '' path; do
    if [[ "$path" =~ [[:cntrl:]] ]]; then printf '%q\n' "$path"; fi
  done)" || failed=yes
  if [ -n "$odd" ]; then
    printf 'public-tree: a path holds a control character, refused:\n%s\n' "$odd" >&2
    exit 2
  fi
  local match=("$bin" leaks --terms "$terms" --words "$words" --links "$links") scope="$rules"
  local ignore admitted=()
  if [ "$rules" != terms ]; then
    [ -n "$manifest" ] || manifest="$(cd "$dir" && pwd -P)/scripts/public-admitted.sha256"
    [ -f "$manifest" ] || manifest=/dev/null
    ignore="$(cd "$dir" && pwd -P)/.gitleaksignore"
    [ -f "$ignore" ] || ignore=/dev/null
    admitted=(--admitted "$manifest" --findings "$ignore" --admit-by "$admit_by")
  fi
  # THE PASSES. The matcher and the secret scanner judge each file by its path, its bytes and
  # the rules alone, so a file that passed under the same rules is not read again. The rules
  # key hashes everything that judges a file: the mode, the three lists, the matcher's and
  # gitleaks's executables, the secret rules, the manifest, the tree's `.gitleaksignore` and
  # this script. A change to any of them names another table, which holds no pass. The table
  # holds `<sha256>  <path>` lines and only clean files; a hit is never kept. The paths, the
  # links and the symbolic links are checked whole every time.
  local passes key table read_from
  passes="${SUZATARY_GUARD_PASSES:-${XDG_CACHE_HOME:-$HOME/.cache}/suzatary/public-guard}"
  key="$({
    printf '%s\n' "$rules" "$strict" "${admit_by:-}"
    sha256sum "$terms" "$words" "$links" "$bin" "$(command -v gitleaks || echo /dev/null)" \
      "$REPO/scripts/secret-rules.toml" "$REPO/scripts/public-tree.sh" "${manifest:-/dev/null}" \
      "${ignore:-/dev/null}" | cut -c1-64
  } | sha256sum | cut -c1-64)" || exit 2
  table="$passes/$key"
  files_of "$dir" | (cd "$dir" && xargs -0 -r sha256sum -z -- 2>/dev/null || true) | tr '\0' '\n' |
    LC_ALL=C sort -u >"$found.sums"
  read_from="$table"
  [ -z "$full" ] && [ -f "$table" ] || read_from=/dev/null
  LC_ALL=C comm -12 "$found.sums" "$read_from" | cut -c67- >"$found.kept"
  { files_of "$dir" | tr '\0' '\n' | grep -vxFf "$found.kept" || true; } >"$found.todo"
  echo "public-tree: reading $(wc -l <"$found.todo") files; $(wc -l <"$found.kept") passed these rules before" >&2

  # Every reader's status is checked where it runs: the hits go to a file, never through a
  # pipe's last command, so a matcher that fails (an older binary, a read error) is a check
  # that did not run, never a clean tree.
  {
    # A symbolic link: refused whatever it names.
    if [ "$rules" = terms ]; then
      :
    elif [ "$(git -C "$dir" rev-parse --show-toplevel 2>/dev/null)" = "$(cd "$dir" && pwd -P)" ]; then
      git -C "$dir" ls-files -s | awk '$1 == "120000" { sub(/^[^\t]*\t/, ""); print $0 ":0" }' || failed=yes
    else
      (cd "$dir" && find . -type l | sed 's#^\./##; s#$#:0#') || failed=yes
    fi
    # Every path, then every file: its lines, and outside `--terms`, where the matcher cannot
    # read it as words, its admission by the manifest.
    files_of "$dir" | tr '\0' '\n' | "${match[@]}" --scope "$scope" --paths || failed=yes
    tr '\n' '\0' <"$found.todo" | (cd "$dir" && xargs -0 -r "${match[@]}" --scope "$scope" "${admitted[@]}" --) ||
      failed=yes
    [ "$rules" = terms ] || [ ! -s "$found.todo" ] || secrets "$dir" || failed=yes
    [ "$rules" != whole ] || dangling_links "$dir" || failed=yes
  } >"$found" || failed=yes
  if [ -n "$failed" ]; then
    echo "public-tree: the matcher ($bin) or the secret scanner failed, so the check did not run whole" >&2
    exit 2
  fi
  hits="$(awk '!seen[$0]++' "$found")"
  # Every file no hit names passed. The table grows by union and starts again from this tree's
  # passes past 100000 lines; a lost pass costs a read, never a verdict.
  local fresh=""
  mkdir -p "$passes" && fresh="$(mktemp "$passes/.fresh.XXXXXX")" &&
    {
      { [ ! -f "$table" ] || [ "$(wc -l <"$table")" -gt 100000 ] || cat "$table"
        sed 's/:[0-9]*$//' "$found" | LC_ALL=C sort -u >"$found.hit"
        awk 'FILENAME == ARGV[1] { hit[$0]; next } !(substr($0, 67) in hit)' "$found.hit" "$found.sums"
      } | LC_ALL=C sort -u >"$fresh" && mv "$fresh" "$table"
    } || { [ -z "$fresh" ] || rm -f "$fresh"; echo "public-tree: could not keep the passes in $passes" >&2; }
  if [ -n "$hits" ]; then
    printf '%s\n' "$hits"
    echo "public-tree: $(printf '%s\n' "$hits" | wc -l) line(s) above would leak or link to nothing" >&2
    leaked=yes
    return 1
  fi
  echo "public-tree: $dir is clean" >&2
}

case "${1:-}" in
  --check)
    shift
    rules=whole strict="" admitted="" admit_by=train full="${SUZATARY_GUARD_FULL:+1}"
    if [ "${1:-}" = --full ]; then full=1 && shift; fi
    case "${1:-}" in
      --lists) rules=lists && shift ;;
      --terms) rules=terms && shift ;;
      --strict) strict=1 && shift ;;
    esac
    if [ "$rules" = lists ] && [ "${1:-}" = --admitted ]; then
      [ $# -ge 2 ] && [ -f "$2" ] || usage
      admitted="$(realpath "$2")" admit_by=scan && shift 2
    elif [ "$rules" = lists ] && [ "${1:-}" = --text ]; then
      admitted=/dev/null admit_by=never && shift
    fi
    [ $# -eq 1 ] && [ -d "$1" ] || usage
    check "$rules" "$1" "$admitted"
    ;;
  --store)
    shift
    [ $# -le 1 ] || usage
    store_path "$@"
    ;;
  "" | -*) usage ;;
  *)
    [ $# -le 2 ] || usage
    materialise "$@"
    ;;
esac
