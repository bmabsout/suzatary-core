# The secretary in the owner's repositories, by permission — 2026-09-24

The owner, 2026-09-24: the secretary may change his repositories (CV, website,
any repo, even Suzatary's code) "upon permission", and list them. Chain:
`secretarycode` (project chain `suzatary`).

## 1. Reading is free, publishing is a tap, and nothing runs a shell

A push publishes, an outward act like sending mail: the tap confirms, and
exactly the diff the card showed is pushed. Listing and reading need no
tap: repository text stands with the chain, mail and documents it reads.

The secretary never gets a shell, not even in a scratch clone: it reads
attacker-controlled mail, and CLAUDE.md forbids such an agent a shell that
can reach this box's tailnet address (where a request is the owner's login).
It writes a diff, never a command; the box runs `git apply`, and the only
code run is the repository's declared check, over a tree the owner approved.

## 2. Rules, each a mechanism

1. **Personal repositories only, by construction.** Reads go through GitHub
   with the box's fine-grained token, owned by `bmabsout`, which cannot
   reach organisation repositories. `RepoName` is `owner/name` with the
   token's owner, so an org name is refused before any request. The token's
   grant is the secretary's view (five repositories today; "any repo" means
   widening it to all of his at `rotategithubpat`, expiry ~2026-11-04).
2. **Reads are over a bare mirror, never a working tree.**
   `~/.cache/suzatary/repos/<owner>/<name>.git`, fetched on first use and
   when older than a few minutes. `repos_list` (name, visibility, default
   branch, description), `repo_tree` (paths under a directory at a ref),
   `repo_read` (one text file at a ref, capped), `repo_grep` (git grep at a
   ref, capped): `git` subprocesses, so no repository code runs on a read.
3. **A change is a patch against a named commit.** `propose_patch { repo,
   base, patch, message, why, sources }` stores `Act::Patch`: `base` the
   commit read, `patch` a unified diff (text files only, ≤ 64 KB) that must
   `git apply --check` against `base` before storing, `message` the commit
   message; `why` and `sources` required.
4. **The card is the diff**: repository and visibility in words ("public:
   anyone can read this once pushed"), base, message, every hunk; a `Card`
   drawn by the one card function.
5. **The tap applies, checks, commits and pushes, or says why not.** In a
   scratch worktree of the mirror at `base`: `git apply`; the declared
   `check` if any (for `resume_typst`, compiling the CV); a commit with
   the owner's identity and a `Proposed-by:` trailer naming conversation and
   proposal; a push. Non-fast-forward is refused (the branch moved) and
   said. Never force-push, never merge. The outcome names the commit.
6. **Where it pushes is declared per repository.** `[[secretary.repos]]` in
   the instance file may set `check` and `publish`: `"default"` (default)
   pushes to the default branch; `"branch"` pushes to
   `secretary/<conversation>-<n>` for a person to merge. Suzatary is
   `"branch"`: master is the running app, and merging takes the full gates
   (workspace clippy with the fixture feature, `verify.sh`, the web build,
   the browser suite), run today by a person.
7. **Nothing private goes public.** SECRETARY.md: no visas, money, legal,
   medical matters or anyone's contact details in a public repository. The
   card's visibility line is the owner's second look.

## 3. What this does not change

The secretary gets no shell, file tools or network of its own; its fence
(`secretary-guard.sh`, the settings deny list) is unchanged. Triage gains
nothing. No bundle gains a mail-sending trait.

## 4. Not in this series: a coding session

A Suzatary change bigger than a reviewable diff needs a shell and a build,
which §1 forbids on this box. It waits for a place with no tailnet route (a
Claude cloud session started from the box, or a network-namespaced sandbox)
and its own plan, reusing the diff card, apply-and-push and branch
publishing built here.

## 5. The series

1. core: `RepoName`, `Patch` (bounded, text-only), `Act::Patch`, its card.
2. io: the mirror, the four reads over `git`, `apply --check`, and the
   apply-check-commit-push performer, over a temp remote in tests.
3. core: the verbs (`repos_list`, `repo_tree`, `repo_read`, `repo_grep`,
   `propose_patch`), the surface snapshot, SECRETARY.md.
4. cli: `[[secretary.repos]]`, the tap performing a patch, the outcome in
   the conversation.
5. web: the diff card; e2e over a fixture repository.

## 6. Since: attachments and pull requests (2026-10-05, `repoattachpr`)

Three rules above moved. Rule 1's universe is now the instance file's closed
list, `[[secretary.repos]]` and `[coding] repo` (`repos::Declared`, all of
one account), met with the token's listing of the owner's own; an
undeclared name is refused before the token is resolved. Rules 5 and 6: the
tap no longer pushes to a default branch; every patch is committed on
`secretary/<conversation>-<n>` and opened as a pull request into it, so
`publish` is gone. And a repository's file, binary included, attaches to a
proposed email, resolved to a commit and kept by its SHA-256 when proposed,
so the tap sends the bytes the card showed.
