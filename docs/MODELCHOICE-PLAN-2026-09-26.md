# Which model answers is chosen in Settings — 2026-09-26

The owner, 2026-09-26: "allow somehow the option to put in settings whether its
a databricks or a claude subscription or something, allow it to be
configurable". Chain: `modelchoice` (project chain `suzatary`). Builds on
`docs/PROVIDERSETUP-PLAN-2026-09-22.md` (a provider is added from the app)
and `docs/MODELROUTER-PLAN-2026-09-17.md` (`[models]`, per-job orders,
spill by `reserve`). PROVIDERSETUP's rule 5 ("nothing routes") was that
series' scope, not a law; this plan is the routing half.

## 1. What exists, and what is missing

- The server already answers a job with the first usable entry of its
  `[models]` order (`choose_brain`, per turn through `SpillingBrain`):
  `claude-cli/<model>` (the subscription, `CLAUDE_CODE_OAUTH_TOKEN`),
  `anthropic/<model>` (the API when `ANTHROPIC_API_KEY` resolves, else the
  CLI), or `<declared provider>/<model>` (OpenAI-compatible).
- Settings can declare a Databricks provider and set any declared key.
- Missing: choosing, from the phone, which of these answers each job.
  Today that is a hand edit of `[models]` plus a restart. This box has no
  `[models]` table, so every job runs today's fallback.

## 2. Rules, each a mechanism

1. **A closed list of backends, as one sum.** `settings::Preset` grows
   from `Databricks` to:
   `ClaudeSubscription` (the CLI; key `CLAUDE_CODE_OAUTH_TOKEN`),
   `AnthropicApi` (key `ANTHROPIC_API_KEY`), `Databricks { workspace }`,
   `Phala` (fixed `https://inference.phala.com/v1`, attested),
   `OpenRouter` (fixed `https://openrouter.ai/api/v1`). Fixed-URL presets
   take no URL from the browser. The two Anthropic ones are BUILT IN: no
   `[[providers]]` row, only a key to set.
2. **Spell each backend once.** No backward compatibility: `anthropic/`
   means the Messages API ONLY and `claude-cli/` the subscription ONLY. The
   "API if the key resolves, else the CLI" guess goes; a job that wants both
   lists both, in order. `doctor` and the bench keep working through the same
   names. Migrate this box's instance file in the same commit (it has no
   `[models]`, so nothing to rewrite, but the fallback in `anthropic_brain`
   becomes an explicit default order: `["claude-cli/<secretary.model>"]`).
3. **An order per job, edited, never rewritten.** `settings::Edits` gains
   `set_order(job, entries)`: `toml_edit` replaces one key of `[models]`
   (creating the table if absent), keeping every other byte. `reserve` gets
   `set_reserve(f64)` in `[0, 1]`, parsed as a newtype, not checked.
4. **An entry is parsed at the route, once.** `ModelEntry` =
   `{ backend: Backend, model: ModelName }`, where `Backend` is a built-in or a
   DECLARED provider name, and `ModelName` is `[A-Za-z0-9._:/-]{1,128}`. A
   model name cannot move bytes anywhere: the backend fixes the endpoint.
   An undeclared backend is refused by name.
5. **Same gate as sending.** Every write takes the `settings::Witness`.
6. **Live without a restart.** The server's `Models` sits behind the same
   lock as `Settables`, re-derived from the file after an edit, and
   `SpillingBrain` reads it per turn (it already chooses per turn; today it
   holds a boot-time `Instance`). A test pins it: set an order, the next
   `name()` answers the new entry, with no restart.
7. **A change is visible.** The instance file is tracked (the data repo
   after `datasplit`), so an edit is a diff; the route also logs one
   `tracing::info!` line (job, from, to, who). MODELROUTER §2: "a job's
   model is the owner's decision, and a change is recorded".

## 3. The page

Settings, "Models" (above "Model providers"):
- One row per job: Chat, Review, Triage, Titles. Each shows its order as
  chips (`Claude subscription · opus-5.5`, `Databricks · deepseek-v4.1-flash`)
  with ↑ ↓ × and an "Add" select over the backends whose key resolves (a
  backend with a missing key is listed, disabled, with "key missing").
- A reserve slider (the account's share the automated jobs never take).
- "Backends": one card per `Preset`, whether its key is set, its secret
  field, and for Databricks the workspace URL. It replaces the single
  "Set up Databricks" form.
- An empty order says what it falls back to, in words.

## 4. The series

| # | Commit | Who |
|---|---|---|
| 1 | core: `Preset` widened, `ModelEntry`/`Backend`/`ModelName`/`Reserve` parsed; `Edits::{set_order,set_reserve}`; `routes::{set_order,set_reserve}` (Witness); wire `ModelsRow { job, entries, fallback }` + `reserve` on `wire::Settings`; laws over a fake `Edits` | agent |
| 2 | cli: `anthropic/` = API only, `claude-cli/` = CLI only, `anthropic_brain`'s guess removed; `toml_edit` impls (a test pins the file minus the edited key byte-identical); `POST /api/settings/models` and `/reserve`; `Models` behind the lock, `SpillingBrain` reads it per turn | agent |
| 3 | web: the §3 page, TS-STYLE; e2e: add an entry, reorder, remove, reserve, refusal of an undeclared backend; screenshots at 412 px | agent |
| 4 | docs: CLAUDE.md, HOSTING §7.1, MODELROUTER §2 pointer, CHANGELOG | agent, reviewed by the main session |

## 5. What not to do

No free-form base URL (PROVIDERSETUP rule 1); no rewrite from `Raw`; no
second place the order lives (the instance file is the one); no guard for a
backend that cannot be named (the parse at the route is the guard); no
per-turn re-read of the file (the lock is swapped on write).
