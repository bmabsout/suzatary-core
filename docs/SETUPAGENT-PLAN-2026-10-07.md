# Guided setup: an agent that connects accounts, and Settings as what is connected (2026-10-07)

The owner, 2026-10-07: "I expect in settings an agent to guide me around and tell me to click links and authenticate and stuff … take it an account at a time, and the agent should know how to set up something new … I'm not sure it's worth showing settings everything like that." And later: "same thing for editing current settings … I shouldn't be bothered with what I'm not using, just given the option to ask and being told what's connected, data storage, mail, calendar, etc. Lets make this app super high quality and smart."

This is a design and nothing is built. It continues VISION-2.0 §3 ("Connecting components is a conversation, not a settings page") and PLUGINS-PLAN stage 5's unbuilt half (the routine taught to build a plugin). It keeps every rule in CLAUDE.md's Irreversibles and Containment.

**The answer in one sentence.** Settings becomes a short overview of what is connected, as readings with one health line each. Every add, change and removal is a card the owner taps. The card is filed either by a setup conversation (a model) or by the box's own rules when something breaks (no model). Each card applies a template: a piece of data that says which instance tables to write, which secrets to ask for, where to send the owner, and how to prove the connection works.

## 1. What an account means (Conal Elliott)

- **A connection is a source of observations under a grant.** It is a pair `(grant, observe)`.
  - The grant is what it may reach: origins, credential names, a consent slot.
  - `observe` is a function from time to what its store knows: a maildir, a plugin's cache store, a Drive listing.
- **"Connected" is a reading, never a flag.** It is a pure function of three things:
  - what the instance declares;
  - whether each declared secret resolves;
  - the join of the connection's sync results (the `mail::health` semilattice, generalised).
- **The setup state is derived too.** `connect::Standing` is one of:
  - `Needs(step)`: the first step not yet met;
  - `Verified { at, observed }`;
  - `Failing { since, class }`.

  The agent never has to remember how far it got. A restart, a second device or a second conversation all read the same standing. This is the "a card's state derived, never kept" rule from app-as-reading 2.
- **What an observation means is the item counts, not the words.** "14 events in the next 60 days" and "1,204 messages in the inbox" are numbers the box computes. That is why they are Clean (§10).

## 2. One algebra: template, manifest, connection (Edward Kmett)

**A template is not a manifest; a manifest is part of what a template produces.**

```
Template = Params -> Connection           (a smart constructor; refuses bad params by name)
Connection = { tables: Tables, steps: [Step], check: Check }
Tables   = Plugin(manifest fragment) | Mail(MailAccount) | Drive(DriveTable)
Step     = Secret { name, label, help, shape } | DeviceCode { account }
         | Grant { plugin, origin } | Terminal { command, why }
Check    = Synced { plugin } | MailSynced { account } | Listed { drive }
```

- **A manifest is only the authority.** It is the closed record `Host::load` and `Request::admit` enforce. A template is a family of manifest fragments indexed by its parameters, together with the person's steps and the proof.
- **A plugin bundle gains a fourth datum, its template.** The bundle becomes component, schema, views and template. A plugin the agent authors then needs no code for its second account.
- **`Tables` stays a sum.** Mail and Drive are not plugins yet (VISION §2 ports). The `Mail` and `Drive` arms shrink into `Plugin` as those ports become components, and no new arm is added beside them.
- **The connection is read back out of the instance; no new table is added.** Applying a template writes the tables that exist today: `[[mail.accounts]]`, `[[plugins]]` and its `[[plugins.credentials]]`, `[secretary] drive_allow`. The connection is recovered by `connect::of(instance)`. This is the onboarding precedent (`of ∘ file = Some`), so nothing has to be migrated.
- **Laws, as proptests in `app/core/tests/connect.rs`:**
  - `of(apply(t, p)) = Some((t, p))`.
  - `remove(add(f, c)) = f`, byte for byte on the instance text (the `add_provider` byte-preservation test, generalised).
  - `add` refuses a duplicate and leaves the file untouched.
  - `Settables::derive` is a homomorphism over connections: deriving from `a ∪ b` gives `derive(a) ∪ derive(b)`.
  - `standing` is a pure function of (instance, resolves, health), and meeting a step never moves it back to an earlier one.

## 3. The conversation (Q1)

**It is a conversation kind with its own record, not a second chat system and not a wider secretary.**

- **What it shares.** `secretary::store::Kind` gains `Setup`. It uses the same conversation store, redaction door, cards, Inbox, tap and dismissal.
- **What it does not share.**
  - Its own capability record, `Setup`: `Model`, `Conversations`, `Clock`, `Connections` (the readings in §1), `Templates`, `Checks`, and `Web` through `Act::Web` only.
  - Its own closed surface `SetupVerb`, snapshotted in `app/core/tests/setup-surface.json`.
  - Its own job, `Job::Setup`, and its own manual, `SETUP.md`.
  - It holds no `Mailbox`, `Corpus`, `Chain`, `Drive`, memory or digest. It never reads mail and never writes a todo. Its fence is a missing field (CLAUDE.md style rule 4).
- **One conversation per connection being set up**, titled for it ("Add a calendar", "Fix her@example.test").
- **Where conversations open.**
  - A composer at the top of the overview: "Ask to add or change anything."
  - "Change" on any line, which opens a conversation with that line's reference loaded.
  - A new `Reference::Connection { group, name }`, text form `connection:calendar/work`, with the usual round-trip proptest.
- **Verbs.** Each is Clean unless marked otherwise.
  - Reads:
    - `connections`: the overview's lines and standings.
    - `templates`: the catalog as data.
    - `connection_show`: Tainted, because it carries a plugin's `Said` words or a provider's error text.
  - Proposals:
    - `propose_connect { template, params }`
    - `propose_disconnect { connection }`
    - `propose_configure { change }`
    - `propose_secret { step }`, which files `Act::Secret`.
    - `propose_auth { account }`, which files `Act::Auth`.
    - `web_read { url }`: `Act::Web` behind a tap or an allow rule, Tainted.
    - `propose_plugin { plan }` (§5).
  - Running the check: `check { connection }`, which runs the template's check now. Its effect is `Effect::Syncs`, like `sync_mail`, so asking twice is asking once.

**A worked example: one calendar.**

1. Ada: "add my work calendar."
2. The agent reads `templates` and asks which service it is on.
3. It files `Connect { ics, label: "Work" }`. The card reads: "Add calendar 'Work'. The calendar plugin will read one iCal link. It may reach: `<the manifest's origins>`."
4. Ada taps. The box writes `PLUGIN_ICS_WORK` into the manifest and re-derives `Settables` live. It then files the template's next step: a Secret card with the template's help text ("in your calendar service, copy the secret address in iCal format"), its help link as a button, and a write-only field.
5. Ada pastes the link into that field and taps. The value goes through `settings::Writes` and nowhere else.
6. The box runs `Check::Synced` and appends a box-authored act turn: "Work: synced, 14 events in the next 60 days." The overview line turns healthy.

The same pattern covers the other paths:

- **Mail by device code.** `Connect` adds `[[mail.accounts]]`. The Auth card shows the URL and user code after the tap (the existing `Consent::given_by_tap` flow). The check is `sync_mail` plus a health success and an inbox count.
- **An app password.** A Secret card, then the same check.
- **A sign-in the box can only take at a terminal** (lieer, the Bridge, the Drive CLI). This is a `Step::Terminal`. The agent says the exact command and why, and the line reads "waiting on a terminal step". The phone is never told it did something it did not do.

## 4. The defaults as templates (Q2)

Templates are data in `suzatary_core::connect::catalog`, one closed `TemplateId` per default. Each declares parameters (parsed, never free text), the tables it writes, its secret names (always derived), its help links (https, data), the shape of each secret (§10), its check and its failure map (§9).

| Template | Params | Writes | Steps | Check |
|---|---|---|---|---|
| `ics` | label | one `url` credential `PLUGIN_ICS_<LABEL>` on the `ics` manifest | Secret (the link); Grant if the link's origin is outside the manifest | `Synced { ics }`: the fetch answered 200, the items admitted, a count of what lies ahead |
| `imap-device-code` | email, tenant | `[[mail.accounts]]` `MsImap` | DeviceCode (IMAP slot only; SMTP stays a terminal consent) | `MailSynced` plus an inbox count |
| `imap-app-password` (stage 7) | email, host, port | `[[mail.accounts]]` `Imap` via mbsync, `PassCmd` reading `MAIL_<SLUG>_PASSWORD`; the mbsyncrc section generated | Secret (the app password, help link to the provider's app-password page when the template knows it) | `MailSynced` |
| `bridge`, `lieer` | email | `[[mail.accounts]]` as today | Terminal (the Bridge login, `gmi init`) | `MailSynced` |
| `drive-cli` | (none) | `[secretary] drive_allow` folders | Terminal (the CLI login) | `Listed`: the root lists |
| `drive-webdav` (later) | url, user | a second Drive `Port`, proved by `conformance::drive` | Secret | `Listed` |

- **The agent needs no code because every value that grants anything comes from the template.**
  - The model picks a `TemplateId` and fills its `Params`.
  - Each parameter has a smart constructor: `EmailAddress`; a `Host` under the template's suffixes, as `WorkspaceUrl` does today; a `Label` of `[A-Za-z0-9 ]{1,32}`.
  - The model cannot type an origin, a secret name, a scope or a help URL.
- **A service that fits a template with new parameters needs no plugin.** For example, an ICS feed on `https://cal.example.test`, which the `ics` manifest does not allow:
  1. When the Secret tap for a `url` credential arrives, the box parses the link's origin and checks it against the manifest.
  2. If the origin is outside, the box stores nothing and files `Act::Grant { plugin: ics, origin }`, naming only the origin. The secret part of a link is its path, not its origin.
  3. Widening a grant is always a separate tap.

## 5. A service with no default (Q3)

- **When it is needed.** No template fits, for example a provider with only a bespoke JSON API.
- **What the agent proposes.** `propose_plugin { plan }` files `Act::AuthorPlugin { task }`. This is the coding-session act aimed at a second routine, `[coding.plugins]`.
- **The task.** Plain text, at most 4 KB, parsed once. It says:
  - which world it targets (`source`);
  - the API's origin or origins;
  - the credential names in `PLUGIN_<NAME>_`, by use (`url`, `bearer`, `header`);
  - the schema's registers;
  - the template, including steps, help links and check.
- **Where it runs.** In the cloud, under a fixed saved brief (`docs/PLUGIN-ROUTINE.md`, versioned). The box is small and builds nothing. The session has:
  - no connectors (`mcp_connections` is `[]`), no secret, and no route to the tailnet;
  - the task as untrusted data;
  - a repository holding only plugins (open question 1).

  It opens one PR with:
  - the source;
  - the component it built for `wasm32-unknown-unknown`;
  - the `.schema`, the template and the manifest fragment;
  - a report naming the session URL;
  - the fence tests (`app/plugin/tests/fence.rs` run against the new component by name).
- **Reproducible, or refused.** The tree half of the gate (Codespace) rebuilds the component from source with the app's pinned toolchain (a flake function the app exports, `lib.buildPlugin`). It fails unless the bytes match the committed `.wasm`. The owner approves source whose binary is provably that source's.
- **Admission is the owner's tap.**
  1. Once the PR is merged on the first-parent chain, the box files `Act::Admit { plugin, rev, component_sha256, schema_sha256 }`.
  2. The card shows the world, every origin, every credential name and its use, `reads`, both digests and the PR link.
  3. The tap performs `Edits::add_plugin` (the `[[plugins]]` table, `from = "user"`, pinned).
  4. `Host::load` then runs the admission laws: world and version, the schema's eight laws, and `plugin::Law` on the first trial.
  5. A refusal names the law and files nothing.
  6. The template then proceeds exactly as a default's (§3).
- **What the plugin may never do.** Each item is a fact of the host, not a rule the plugin follows.
  - Import beyond its world: no sockets, filesystem, environment or randomness (`LoadError::ForeignImports`).
  - Fetch outside the manifest's origins (`Request::admit`), or reach the box or the tailnet (`Ureq::public_only`, no redirects).
  - Hold or echo a secret (`Refusal::Echoed`).
  - Write anything but its own cache store, or touch a chain or the instance file.
  - Act on anything: a source cannot propose, and an actor only proposes through the tap.
- **A new origin is a new tap.** An authored plugin that later needs another origin is a new PR and a new Admit or Grant tap. A changed component under the same pin is refused by its sha256.

## 6. Every step a tap (Q4)

| Act (new unless marked) | Filed by | The tap performs |
|---|---|---|
| `Connect { template, params }` | agent; the secretary's `propose_act` since 2026-10-17 (audit S1), in a round of the owner's words alone | `Edits::add_connection`: toml_edit, temp file and rename; `Settables` swapped live; act turn names the tables |
| `Secret` (exists) | agent or box | `settings::Writes` with the value from the tap body, never stored; the origin check for a `url` |
| `Auth` (exists) | agent or box | the device-code flow, `Consent::given_by_tap` |
| `Grant { plugin, origin }` | box | `Edits::add_origin` |
| `Disconnect { connection }` | agent or box (§9) | `Edits::remove_connection`; the connection's secrets cleared through `Writes`; its cache store deleted (cache tier); mail on disk per open question 3 |
| `Configure(Order \| Reserve \| Provider \| RemoveProvider)` | agent | `Edits::{set_order, set_reserve, add_provider, remove_provider}`, swapped live as the routes do today |
| `AuthorPlugin { task }` | agent | fires the plugin routine (`commands::coding::Coder`) |
| `Admit { plugin, rev, digests }` | box, after the merge | `Edits::add_plugin`, then `Host::load` |

- **The tap stays the one gate.** Every tap is a device-signed request (`Trust::admits`) through `Tap::from_gate` with the CSRF triple. Its provenance goes to `requests/` (who asked, what it wrote; never a value). Loopback mints no `Tap`, so neither agent can tap. A card from a Tainted round is still a card: the tap is the review.
- **Instance writes need no hand edit.**
  - The performer edits the instance file in the user repository (it already is the data tree's), and `safe-push.sh` commits it within five minutes. Stage 4 checks the push's allowlist names it.
  - `deploy.sh` runs the new binary's `verify` against the live instance, so a deploy that cannot read a connection refuses before it moves.
  - The sync and plugin timers read the instance per run, so a new connection is live on the next tick with no restart.
- **The vocabulary grows.** `act_vocabulary::SIGNATURES` gains each new constructor. A MINOR version bump, since the stored-literal vocabulary extends.

## 7. Changing what exists

The same agent, the same cards. Nothing is edited in place on the page.

- **A new link or rotated app password.** "Change" on the line, then a Secret card for the same name.
- **A lapsed grant.** An Auth card.
- **Removing an account.** A `Disconnect` card that lists what goes (the tables, the secrets, the cache) and what stays (mail on disk, per open question 3).
- **Model order, reserve, providers.** A `Configure` card that shows from and to. "Secretary: `claude-cli/<a>` → `anthropic/<b>`" reads as data the box parsed through `models::ModelEntry`.
- **A model that is down.** If `Job::Setup`'s own model is unreachable, the agent cannot help. The box's fix cards (§9) need no model, an empty order falls back to `Instance::fallback`, and the terminal's `secretspec set` remains the operator's path.

## 8. Settings: the overview (Q5)

`settings::overview(instance, resolves, healths, ledgers, open_rows) -> [Group]` is a pure reading in core. Groups appear in this fixed order: Mail, Calendar, Storage, Models, Notifications, Coding, This box, Devices. A group with nothing declared is not drawn. Each line is a label, one health sentence and, when one stands, its open fix card inline (the Inbox's own card, as account rows do today). A login that can only read sees the lines and no Ask or Change.

| Shown today | Becomes |
|---|---|
| Mail accounts: credential line, health, open decisions | Mail: "her@example.test: synced 4 min ago". A missing slot is its line's fix card. Change and remove through Ask. |
| Calendars and plugins: `PLUGIN_ICS_<NAME>`, Set box, "link missing", last run | Calendar: "Work: synced 12 min ago, 14 events ahead". Secret names are never shown; the Set boxes are deleted (stage 1). Other plugins are listed under their template's group. |
| Models: job order editors | Models: one line per job ("Secretary answers with …"). Change through Ask. Editors deleted in stage 5. |
| Reserve control | One Models line ("keeps 20% for you"). Control deleted in stage 5. |
| Backends: one card per preset with key fields and the setup form | Deleted. A backend in use appears as its job's line with key health; adding one is Ask. |
| Model providers: base URL, attested, key field | Models lines for declared providers only. A declared provider no job uses says so, and §9 offers its removal. Base URL inside the line's details. |
| Notifications, Coding sessions | Shown only when declared, with health lines; their secrets go through fix cards. |
| This deployment: version, build, chains, rosters | This box: version and build. Chains and rosters move to Storage. |
| (absent) | Storage: the user repository (last pushed, from the push's own record), the chains, Drive (what it may read), the vault's last snapshot once the box records one. |
| This device: key, ask trust, renew | Devices: kept as direct controls, because they act on this browser's own key, not on the deployment. |

**Routes.**
- `GET /api/settings` becomes `GET /api/overview` (`wire::Overview` replaces `wire::Settings`).
- `POST /api/settings/{secret,provider,models,reserve}` are deleted. Their writes are performers behind the tap (§6).

## 9. "Smart", concretely, each a mechanism

1. **A broken connection files its own fix, with no model.** This generalises `mail_health`. Each template maps a failure class to an act:

   | Failure | Fix card |
   |---|---|
   | ICS link answers 401, 403 or 404 | Secret: "the link was rotated or revoked; set the new one", with the help button |
   | `NeedsLogin` | Auth |
   | A declared secret does not resolve | Secret |
   | A model backend refuses its key | Secret for that key |

   Identity is (connection, since), so a failure files once however often it is seen. A failure lasting a day still proposes the todo it does today.
2. **Ask opens knowing the state.** The agent's first context is the overview's failing and pending lines and each one's next step.
3. **Verified, never assumed.** Every connect ends with its check. The result is a box-authored act turn of counts, and a pending step stays visible on the line.
4. **A pasted secret is caught before it is stored** (§10).
5. **A link from an unknown origin becomes a Grant card**, not a silent failure (§4).
6. **What is unused gets offered for removal.** A provider no job orders, a calendar with nothing in 90 days, an account failing for a week: each files a box-authored `Disconnect` or `RemoveProvider` card, which can be dismissed.
7. **What only the owner can do, the owner is told exactly.** A terminal step's command and reason, or the provider page a help link opens.

## 10. The security view

- **Where injection can come from.** Provider help pages read through `Act::Web` (Tainted), a plugin's `Said` words and providers' error text (Tainted), and text the owner pastes from a page. Mail is not among them: the `Setup` record cannot read it.
- **What an injection can reach.**
  - Every value that grants anything comes from template data or from a parameter a constructor parsed: origins, secret names, scopes, help URLs, tables.
  - Only template help links are drawn as buttons. Links in model prose go through `refs.ts` as text with their host shown.
  - At worst, a successful injection yields a valid but wrong card the owner declines, or a task the plugin routine treats as untrusted. The plugin that comes back is fenced by a manifest the owner reads at Admit.
- **Credentials.**
  - A value exists only in a Secret tap's body, passed to `Writes` and then to `secretspec set` on stdin.
  - It is never in a proposal, transcript, task, PR, manifest or log.
- **The pasted-secret door.** `redact` scrubs credential query parameters and JWTs, but not a secret iCal link's path segment.
  - Each `Step::Secret` declares a shape: an https URL under its plugin's origins, an app password's grouping, or a high-entropy path segment.
  - A person turn in any conversation that matches a declared shape is refused before the store: nothing is written and nothing reaches the model.
  - The composer says "that looks like the secret itself; use the card's secure field" and runs the same check in the tab.
  - Both sides read one shape definition through `wire`.
- **Taint.** `connections`, `templates` and `check` answer Clean: the box's own names and counts. `connection_show` and `web_read` are Tainted. The memory gate (`verbs.rs`'s `Gate`) never opens in a Setup conversation, because its record has no memory.
- **The authored-plugin trust boundary** is §5. The rule underneath: the box never builds, merges or runs code that was not merged, rebuilt to the same bytes and pinned by the owner's Admit tap.

## 11. The agent's model and prompt

- **The model.** `Job::Setup` (`Job::ALL` gains it). The order is named in `[models] setup = ["claude-cli/<a Sonnet-class id>"]`. It is a data decision like any job's, recorded on the project chain, and changed only by a `Configure` card. An empty order answers with `Instance::fallback`.
- **The prompt.** `SETUP.md`, through `Manuals`, so it can be read in a diff. Its rules:
  - one connection at a time;
  - never ask for a secret in the chat, always file the step's card;
  - say what each tap will do before filing it;
  - take every service, link and value from the `templates` verb, never from memory or a page;
  - end every connect with its check and quote its result;
  - when no template fits, propose a plugin with a plan and say it will come back as a PR to approve.
- **The catalog is not restated in the manual.** It is served by `templates`, so the manual cannot drift from the code.

## 12. Quality: how it is tested

- **Core laws** (§2), plus:
  - every `TemplateId` applies, and its `of` inverts;
  - every failure class maps to exactly one fix act;
  - fix identity: one act per (connection, since).
- **Node tests** over `web/test/gen.ts`'s seeded generator for the overview's pure modules:
  - every declared connection appears exactly once;
  - no group is drawn for an undeclared kind;
  - a line's words are a function of its standing alone.

  Rust writes vectors under `app/core/tests/web/` and the TypeScript must agree with them.
- **E2E scenes** (`web/e2e/overview.test.ts`, `setup.test.ts`, against `suzatary serve` fixtures with a scripted model):
  1. Only what is declared is drawn, grouped; a fixture with no notifications has no Notifications heading.
  2. A failing calendar shows its fix card inline. Tapping Set makes the line healthy, and the CDP network log shows the value only in the tap's request.
  3. Ask, then add a calendar: Connect card, tap, Secret card, tap, and the act turn with counts.
  4. A link from an unknown origin yields a Grant card and nothing stored.
  5. A pasted link in the composer is refused, and no request carries it to the conversation.
  6. Disconnect: the instance lacks the tables and the line is gone.
  7. Configure order: the card shows from and to, and after the tap the Models line moves.
  8. A read-only login sees lines and no controls.
  9. The model unreachable: the fix cards still appear.
- **Pictures.** `shots.test.ts` keeps pictures of the overview for the eye, in both themes.

## 13. Stages (Q6)

Each stage is one PR, a series with laws and tests, gated in full and riding a train.

| # | Item | What | When |
|---|---|---|---|
| 1 | `fixcards` | A declared secret that does not resolve files a box-authored Secret card, once per name and declaration, drawn on its row. The raw Set boxes are deleted. | 2.0 |
| 2 | `overview` | The `overview` reading, `wire::Overview`, the page rewritten by group with labels instead of names. Model editors kept behind "Change" until stage 5. Scenes 1, 2, 8. | 2.0 |
| 3 | `connectionhealth` | One health join per connection (plugin ledgers like mail's; a Drive probe), fix rules by failure class, stale-removal cards. Scene 9. | 2.0 |
| 4 | `templates` | `connect::{Template, Connection, Step, Check, Standing}`, the catalog, `Act::{Connect, Disconnect, Grant}`, `Edits::{add_connection, remove_connection, add_origin}` with the byte laws. The box files a template's steps. Scenes 4, 6. | 2.0 |
| 5 | `configureacts` | `Act::Configure`, deleting the four POST routes and the page's editors. Scene 7. | 2.0 |
| 6 | `setupagent` | `Kind::Setup`, `Job::Setup`, the `Setup` record and its snapshotted surface, `SETUP.md`, `Reference::Connection`, the pasted-secret door, Ask and Change. Scenes 3, 5. | 2.0 |
| 7 | `imapapppassword` | `Sync::Imap` via mbsync with `PassCmd`. The mbsyncrc generated from the instance, so the `Patterns` to `inbox_folders` pin becomes a law of the generator. Its template. | 2.0 if room, else next |
| 8 | `webdavdrive` | A second Drive `Port` proved by `conformance::drive`, and its template. | later |
| 9 | `authoredplugins` | The plugin routine and repository, `Act::{AuthorPlugin, Admit}`, `lib.buildPlugin`, the reproducibility gate, templates shipped in bundles. | later (PLUGINS stage 5's routine half) |
| 10 | `authcode` | Authorization-code OAuth redirected to the app's tailnet URL, for providers with no device-code grant. | 2.0 (the owner, 2026-10-17) |

Stage 4 BUILT (2026-10-09, the `setup-stage-4-templates-dlr17z` report): `suzatary_core::connect` (a template's table is a closed vocabulary, `Table`; parameters a sum by shape, `Params`; `Declared` the instance's tables, read back by `connections()` and written by `with`/`without`/`with_origin`), the catalog without the Databricks, Phala and OpenRouter presets, the three acts and their cards, the byte-preserving edits in `app/cli/src/instance/connections.rs`, the box's rule `secretary::connections`, and scenes 4 and 6 in `web/e2e/connections.test.ts`. `Grant` is an act and not a `Step`: a step is a function of the template, and which origin a link names is a function of the secret's value. A connection's `Group` and `Standing` are the overview's (stage 2's): its standing is the join of its steps' standings (`connect::Seen` lends a secret's state, the account's grant and whether the check once succeeded), the reader joins the check's results, and a terminal's step reads `ByHand` with its command until then. The overview's mail and notification lines join their connections' readings; storage and the Drive are not mapped until their checks are observed (stage 3). A plugin's manifest stays when its last link is removed, since one the person wrote alike cannot be told apart.

Stage 3 BUILT (2026-10-13, `connectionhealth`): a failure's class is `connect::Failure`, a closed sum parsed where the failure is observed, carried by every health register (`mail::health::Failed`), by `Standing::Failing` and by the line's words; `Failure::remedy` gives each class exactly one remedy. A plugin's links are joined as mail's are (`Ledger.links: Healths`, carried by `plugin migrate-ledgers`), the mail and model registers gain the class (`mail migrate-health`), the Drive is probed into `drive-health.json` and the data repository's pushes are read from its reflogs, so Storage and the Drive are mapped. The box's rule is `secretary::failing`: a fix is the remedy's step of the connection's own template (a link's secret, an account's device-code sign-in) or a model's key, keyed by (connection, since) through `Owed::since`, and a secret or sign-in another rule already asks for is that card. While no model answers, the models are the bootstrap's to ask. The stale offers are a provider's removal, stage 5's `Configure::remove` (its Configure card), and `Act::Disconnect` for a calendar quiet for 90 days and a mail account failing for a week; a removal card is drawn on the line it removes (`Need::Removal`).

Stage 10 BUILT (2026-10-17, `authcode`): "Sign in with Google", one consent for a Google account's mail and calendars (view, and edit of the calendars it owns); Drive is asked for by nothing until a feature reads it. `suzatary_core::google` is the sans-I/O flow over the authorization-code grant with PKCE: a `State` and a `Verifier` drawn per consent (256 bits each), the S256 `Challenge`, the `RedirectUri` derived from the app's URL (https, or http on loopback for a test), the `Scopes` asked (`openid email`, lieer's three Gmail scopes, `calendar.readonly` and `calendar.events.owned`, with `include_granted_scopes=true`), the `Callback` read once from the query, the exchange and refresh as `FormPost`s and their answers read into a `RefreshToken`, the parts Google says it granted and the verified account its ID token names (another account is refused). It reuses `mail::oauth`'s `FormPost`, `Answered`, `OauthError`, `Silent` and `NeedsLogin` (which gains `Google`); the sum that gains the arm is the template step, `connect::Step::{DeviceCode, AuthCode}` (and `GoogleClient`), with its act, `Act::GoogleSignIn` beside `Act::Auth`. The template is `google` (`TemplateId::Google`, writing `[[mail.accounts]]` with `sync = "google"`), its steps the box's Google client and the account's sign-in; the `lieer` template stays for an account signed in at a terminal. The client is asked for by `Act::GoogleClient`, a card whose words are the six steps (`google::CLIENT_STEPS`, also in the guide) with the exact redirect written into its step, and whose write-only field takes Google's downloaded client file, kept only when it is a web client listing that redirect. The consents are held in server memory alone (`serve::google::Consenting`, `secretary::google_sign_in::Open`), each bound to its card and the tapping actor; the card's button is the box's own `GET /api/oauth/google/consent/<card>`, which sends the browser on to Google, so the tab's fold draws it and no state is stored; `GET /api/oauth/google/callback` takes the state once, refuses another actor's, trades the code with the verifier and keeps the grant as `GOOGLE_GRANT_<ACCOUNT>` (`Declaration::GoogleGrant`, which, like `GoogleClient`, no Secret card asks for): a `KeptGrant`, the parts Google granted and the refresh token on one line, whose parts alone `Secrets::google_grant` reads, so a grant lacking a part is owed a sign-in card asking for that part alone (`connections::lacking`); a first sign-in and a revoked grant's fix ask for every part, since Google forgets a revoked grant whole. The mail sync mints an access token from the grant and hands lieer only that token and its expiry (`.credentials.gmailieer.json` with `refresh_token`, `client_id` and `client_secret` null, which google-auth loads as valid until the expiry; `gmi init --no-auth` where lieer has no repository yet); the account sends (`Sender::Google`, `smtp::GoogleApi`) and searches older mail (`older::GoogleTokens`) on tokens minted the same way and classes a missing client `Unresolved` and a refused grant `NeedsLogin`, whose remedy is the sign-in card again (`secretary::failing`). The calendars are `suzatary_io::google::Google::occurrences`, read at most 20 calendars of at most 250 events each in a week back and 90 days ahead, no answer past 1 MiB read, each occurrence held to the calendar's limits, learned into `cache/google/<account>/store` at the calendar plugin's schema by `suzatary plugin sync`, so `calendar` reads them as the ICS plugin's. `Act::CalendarEvent` writes one event into the account's own calendar on its tap alone (`propose_act` `calendar_event`). Tests: `app/cli/tests/google.rs` against `tests/common/fake_google.rs`, which the browser suite also runs (`suzatary-dev fake-google`); the scene is `web/e2e/google.test.ts`. Not built: a Google Drive `Port`; it will ask for its scopes as a part of its own, alone.

Stage 5 BUILT (2026-10-13): `Act::Configure { change }` over `settings::configure::Configure`, an opaque struct over a private closed sum of `Order { job, Move<Vec<Entry>> }`, `Reserve(Move<Reserve>)` and `Provider(Provided)`, built only by `Configure::{order, reserve, declare, remove}` or read back from what they stored. A `Provided` either declares a `ProviderSetup` (where the provider was absent or declared otherwise) or removes a declared provider, so `RemoveProvider` is the provider arm's move to nothing rather than an arm of its own. An `Entry` is an order's entry as the card holds it, which becomes a `ModelEntry` only through `ModelEntry::parse` at the tap. A `Move` stores what the card read and what it writes, both on the card. One rule (`Configure::write`, run by `Configure::perform` through `Edits::swap`) writes `to` while the file reads `from`, is done when it reads `to`, and otherwise writes nothing and says what it reads; the box then withdraws that card. `Edits::swap` reads, compares and writes the file under one `flock` on `instance.toml.lock` beside it, which every instance-file edit and `suzatary instance set` take. `Edits::{add_provider, set_order, set_reserve}` are gone. `Configuration`, a read-only supertrait of `Edits`, is held by the `Secretary` and `Server` records, so `propose_act` `configure` reads a card's From; it is refused in a round that read anything but the owner's words. After the tap the server reads the file again (`State::reconnected`). The routes, `settings::Witness`, the editors and the wire types named in §14 are gone; scene 7 is `web/e2e/configure.test.ts`. The Models group draws one line per job and a Reserve line; a provider's Declare card has no line until it is declared, so it stands in the Inbox only.

## 14. What is deleted (no backward compatibility)

- On the Settings page: `secretField` and every raw secret box, the Backends preset cards, the provider setup form, the job order editors and the reserve control.
- In the web state: `settingsState.decisionsFor` (generalised to connection references).
- Routes: `POST /api/settings/{secret,provider,models,reserve}` and their handlers in `app/cli/src/commands/serve/settings.rs` and `routes.rs`.
- Wire types: `wire::Settings`, `PluginRow` and `PluginLinkRow` (replaced by `wire::Overview`).
- E2E: `providersetup.test.ts` and `modelchoice.test.ts`, rewritten as card scenes.
- Docs: the CLAUDE.md lines that say links are "set on the Settings page by name" (they become "asked for by a card").

No stored format moves: the instance tables are the ones written today.

## 15. Open questions for the owner

1. **Where authored plugins live.** In the user repository, as asked, a cloud session would clone the owner's chains, inbox and conversations. Recommended: a private plugins-only repository that the user repository pins as a flake input, so one user-repository commit still names everything.
2. **Who merges an authored plugin's PR.** The box never merges, so either the owner from the card's GitHub link, or "Hand to the main session". Recommended: the owner's link, with the tree half's verdict shown on the card.
3. **Removing a mail account.** It clears its secrets and cache. Should its mail in the maildir and notmuch go too? Recommended: keep it, and say so on the card.
4. **No direct controls at all.** When the model is down, the box's fix cards and the terminal remain. Is that enough, or should "Change" also offer a template's form without the agent?
5. **The Setup job's model.** Which Sonnet-class id? And may it run on a non-Anthropic backend, given it reads provider pages but never mail?
6. **Authorization-code OAuth** redirected to the tailnet URL (stage 10): worth it, given some providers refuse a private redirect host?

---

### Critical Files for Implementation
- app/core/src/secretary/acts.rs
- app/core/src/settings.rs
- app/core/src/plugin/manifest.rs
- web/src/pages/settings.ts
- app/core/src/secretary/mail_health.rs

## 16. Stage 0: the bootstrap (the owner, 2026-10-07)

The setup agent needs a model, so connecting a model is the one step that is NOT agent-guided. It is a fixed template the box files itself, exactly like a §9 fix card, whenever no job's order resolves to a model that answers:

- **Models: none connected → Connect** is the only line the overview draws before a model works.
- `model-claude-subscription`: the box runs Claude's token sign-in (`claude setup-token`) in a pty, the card shows its URL, the owner signs in on the phone and pastes the code into the card's secure field, the box feeds it to the pty and stores the token through `settings::Writes` (`CLAUDE_CODE_OAUTH_TOKEN`). No value in a transcript.
- `model-api-key`: Anthropic or an OpenAI-compatible provider; the key through a Secret step, checked by one test call before it is kept.
- Once one model answers, the setup agent takes over (more providers, switching).
- A model that later stops answering (an expired token) re-files the same card, so the agent is never the thing that must fix itself.
- `suzatary init` offers the same step on a terminal.

First commit: a spike proving `claude setup-token` can be driven headless from a card (URL out, code in, token captured); if not, the fallback is the API-key card plus a one-line terminal command on the card.

## 17. The owner's answers (2026-10-07)

1. **Where things live is the user's choice, asked during setup.** Where the prodrome (data), the code and the connectors are stored is itself a connection the setup agent configures ("Storage" group): git with a remote is the default template; another forge, plain git with no forge, or one of the user's data storages (a drive) are templates to come. No extra repository is assumed: agent-written connectors live in the data repository by default (under `plugins/`), pinned by digest there. The plan's "plugins-only repository" recommendation is withdrawn.
2. **No GitHub or PRs assumed.** The default admission is a card on the box: the authored connector arrives as a proposal (source, the component, its digests, its manifest's grants, the reproducible-build verdict) and a **Confirm** tap admits it. The transport back from the build is the configured storage (by default a branch in the data repository, which the box fetches and rebuilds to the same bytes before filing the card). A forge PR is an optional workflow for users who have one.
3. **Removing a mail account** asks whether to delete the mail kept as cache (the maildir and its index). Todos, cards and conversations ALWAYS stay; a reference to a message that no longer exists reads as "this email is no longer stored" (with its stored subject and sender if known), never an error and never removed. A law: every reading tolerates a dangling mail reference.
4. **"Change" opens the connection's own cards; "Ask" opens the agent.** A template already declares its steps, so "Change" on a line lists that template's actions as ordinary cards (a calendar: set a new link, check now, remove; mail: sign in again, set a new app password, remove; models: reorder, connect another), with no model involved. They work when the model is down and cost nothing to keep in step, since the agent files the very same cards. "Ask" is for what no template names (a new service, "why is this not syncing", moving storage). No hand-written forms.
5. Sonnet 5.5 on Anthropic for the setup job, for now.
6. Authorization-code OAuth to the tailnet URL: yes, later (stage 10).
