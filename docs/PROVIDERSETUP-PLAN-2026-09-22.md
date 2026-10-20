# A provider is set up from the app — 2026-09-22

The owner, 2026-09-22 (Databricks for DeepSeek V4.1 Flash): set up Databricks
from Settings. Chain: `providersetup` (project chain `suzatary`).

## 1. What exists, and what is missing

Settings (`docs/SETTINGS-PLAN-2026-09-19.md`) sets a declared secret; the
Inbox asks for one (`docs/SECRETASK-PLAN-2026-09-21.md`). Declaring a
`[[providers]]` entry was a hand edit of `instance.toml` plus a restart — a
shell a phone user lacks. A provider needs a workspace URL (data) and a
token (secret); only the token had a door.

## 2. Rules, each a mechanism

1. **A preset, not a free-form provider.** `Databricks` (later presets are
   closed variants): from the workspace URL the app derives
   `name = "databricks"`, `base_url = <workspace>/serving-endpoints`,
   `key = "DATABRICKS_TOKEN"`, `attested = false`. `WorkspaceUrl` accepts
   only `https://` hosts under `cloud.databricks.com`,
   `azuredatabricks.net` or `gcp.databricks.com`, refusing others by name;
   no free-form `base_url`, so a browser cannot aim the model anywhere.
2. **The instance file is edited, never rewritten.** `settings::Edits` (in
   no bundle, like `Writes`) appends one `[[providers]]` table with
   `toml_edit`, preserving every comment and byte, atomically (temp file +
   rename); a duplicate name is refused. Never rewrite from `Raw` (that is
   `init`, for a fresh file): the comments are the operator's docs.
3. **Same gate as sending.** The route takes a `settings::Witness`, as
   `set_secret` does, from the same tailscale gate.
4. **The closed list follows the file, without a restart.** After an add
   the server re-derives `Settables` from the file and swaps it in, so the
   token can be posted next. Never widen `Settables` by hand.
5. **Nothing routes.** A provider is a name for `--model` or `[models]`;
   this series changes no job and does not touch `[models]`.

## 3. The series

| # | Commit | Who |
|---|---|---|
| 1 | core: `settings::{Preset, WorkspaceUrl, ProviderSetup}`; `settings::Edits` trait; `routes::add_provider` (Witness, refuses bad host and duplicate, answers the row); wire `ProviderRow { name, base_url, key, key_set, attested }` on `wire::Settings`; laws over a fake `Edits` | Sonnet |
| 2 | cli: `toml_edit` `Edits` (a test pins the file minus the appended block byte-identical); `POST /api/settings/provider`; `Settables` behind a lock, re-derived after an add | Sonnet |
| 3 | web: Settings gains Providers (rows + "Set up Databricks": workspace URL, password-field token, one button posting provider then token); e2e over the isolated secretspec and a scratch instance file | Sonnet |
| 4 | docs: CLAUDE.md Containment gains the rule; HOSTING.md §7.1 gains the provider step; the chain | **main session** |

## 4. What not to do

No free-form `base_url` (rule 1); no rewrite from `Raw` (rule 2); no
hand-widened `Settables` (rule 4); no routing (rule 5).
