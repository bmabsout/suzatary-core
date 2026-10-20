# The app as a person meets it — 2026-10-13

The pictures are in this document's PR, #165, in its first version.

The owner asked, before the v2.0.0 tag: "Is the app fast, are the components
user friendly, is there no extraneous information that a person isn't using
shown?" This file answers that question. It reports and fixes nothing: each
finding names a change, and the owner chooses which to take.

The previous reading of this kind is `docs/AUDIT-UX-2026-09-12.md`. Two of
its findings are still visible here (the list's chrome before the first todo,
and the todo page's provenance above the fold), and they are listed again
below with today's measurements.

## How it was measured

- **Build.** The fixture binary `suzatary-dev` (`scripts/app-build.sh
  --fixture`) and the production web bundle (`scripts/build-web.sh`, no
  `--dev`), at `4380011`. Merging origin/master afterwards brought in
  `CHANGELOG.md` and `app/Cargo.toml` only, so the pictures hold for the
  merged tree.
- **Servers.** The e2e harness's own fixture servers (`scripts/web-e2e.sh`):
  the viewer (`E2E_BASE`), the viewer with a held batch (`E2E_BASE_PROPOSED`)
  and the writer (`E2E_BASE_WRITE`), with the scripted model
  (`--model-fixture`) and the recorded transport. Fixture data only: the cast
  in `app/core/tests/web/cast.json`, every address under `example.*`.
- **Driver.** A throwaway scene file run through the harness over CDP and
  removed afterwards (not committed). It drove every destination and every
  card kind as a writer, the secretary chat (send, a slow answer, a card in
  the turn), the add sheet, Done from the list, the More sheet and Settings.
- **Pictures.** Kept in #165's first version, not in the repository. Light
  and dark at 390×844 and 1440×900; 360, 412 and 430 px wide for the main
  pages; `-full` pictures are the whole scroll height. In a
  `-full` picture the fixed tab bar and header are drawn where the viewport
  was, in the middle of the page: that is the capture, not the app.
- **Times.** Chromium's CPU throttled 4x (`Emulation.setCPUThrottlingRate`),
  the network not throttled, the box on loopback. A time is the page's own
  clock (`performance.now()`), polled once per animation frame:
  - *first paint* is `first-contentful-paint`;
  - *ready* is the first frame the page's own content is on screen (a row on
    Todos, the body on a todo, a card on the Inbox, the composer in a
    conversation, a line in Settings);
  - *settled* is the last DOM change before 700 ms of quiet;
  - *cold* is a fresh document with the HTTP cache and the service worker
    bypassed; *warm* is the median of three repeat visits.
  - A tap's time runs from just before the press to the first frame its
    answer is on screen.

  The box answers in microseconds on loopback, so these are the cost of the
  app on a slow phone and not of a network.

## The answer, in three sentences

**Fast: yes on the box's side, almost on the phone's.** Every page shows its
content within 0.4–1.5 s at 4x CPU (the Typst document 2.7 s cold), and a
navigation tap answers in 0.33–0.49 s, between Nielsen's 0.1 s ("instant") and
1 s ("flow kept") limits. Done takes 1.3 s to say so, and while it waits the
screen shows a red alarm of hashes.

**Friendly: the parts are good and the joins are not.** Four doors lead to the
secretary, and on a phone one of them covers another. The Inbox shows one card
per screen. The cards use four verbs for "yes" and two for "no". Fifteen different
sentences mean "loading".

**Extraneous: a great deal.** The default views show the store's machinery:
hashes, constructor literals, chain names, "the fold prices it", a rendered
cache's size, shell commands, the release plan. All of it is true and none of
it serves the task on the screen. Most of it should move behind a detail,
not be deleted, because the app's rule is to show its working.

## Measurements

### Pages (4x CPU, ms)

| Page | Cold first paint | Cold ready | Cold settled | Warm first paint | Warm ready | Warm settled |
|---|---:|---:|---:|---:|---:|---:|
| Todos | 560 | 1492 | 1828 | 576 | 1156 | 1896 |
| A todo | 396 | 1113 | 1666 | 444 | 905 | 1494 |
| Inbox, a batch held | 344 | 1140 | 1115 | 340 | 897 | 877 |
| Inbox, empty | 356 | 411 | 1262 | 364 | 396 | 1286 |
| Secretary (topics) | 384 | 863 | 908 | 384 | 876 | 838 |
| A conversation | 356 | 714 | 1117 | 408 | 856 | 1299 |
| Memory | 448 | 1047 | 1023 | 400 | 912 | 901 |
| Settings | 344 | 1175 | 1098 | 344 | 1230 | 1145 |
| Todos, drawn by Typst (the document) | 724 | 2661 | 4088 | 316 | 1652 | 1899 |
| Mail | 560 | 576 | 1719 | 404 | 459 | 1446 |
| Chain | 368 | 435 | 1329 | 400 | 433 | 1618 |
| Usage | 308 | 360 | 1032 | 368 | 359 | 1112 |
| About | 344 | 360 | 1095 | 428 | 443 | 1208 |
| Briefings | 444 | 493 | 1203 | 452 | 455 | 1176 |
| Calendar | 392 | 459 | 1195 | 336 | 439 | 1160 |
| Project | 516 | 633 | 1542 | 580 | 603 | 1769 |

Where *settled* is well after *ready* (Todos +0.7 s, the empty Inbox +0.9 s,
Chain +1.2 s), the page keeps redrawing after the person has started reading.

### Taps (4x CPU, phone, as a writer)

| Tap | Answer on screen | What the person sees |
|---|---:|---|
| A row, to open it in place | 486 ms | The row opens with Open, Done, Ask, Add a subtodo |
| Open, to the todo's page | 464 ms | The page, its title the body |
| Tab: Inbox | 458 ms | The Inbox |
| Tab: Secretary | 338 ms | The topics |
| Tab: Todos | 476 ms | The list |
| Done, from the list | 1311 ms to the toast | A red banner of hashes and "1 not on the box", then "marked done · Undo" |
| The add sheet's handle | 330 ms | The sheet |
| Add todo, no category picked | refused | "Choose a category." below the button, under the tab bar |
| Dismiss on an Inbox card | 326 ms | The reason field |
| Send, in a conversation (six asks) | 646–1047 ms to the person's own turn; 769–1140 ms to the answer | "Queued" and Stop, then the answer |

The scripted model answers at once, so the send times are the app's own cost:
up to a second before the person sees their own words in the conversation.

### Widths

At 360, 412 and 430 px no page scrolls sideways. One element overflows: on
Todos at 360 px a category chip (`button.chipbtn`) ends at x = 385. The add
sheet's fifth chip, "other", is clipped at 390 px too (`add-sheet-typed.png`).

## Findings, ranked by impact

Each finding names its picture by file name; the pictures are in #165's first version.

| Id | Impact | Page | Picture | What the person experiences | Who objects, and why | Change |
|---|---|---|---|---|---|---|
| X1 | high | Todos, after Done | `tap-done.png` | Tap Done and, for the second before the box takes it, a red box appears at the top: "Folded locally from the objects as of 2f09bafc200d…, 4405434d438f…, 533a6cd80bd7… — the same core, in this browser, so these numbers are this moment's; what may have moved is the chain. 1 object this browser wrote has not reached the box." with Retry, and a "1 not on the box" pill. It reads as an error, and nothing went wrong. | Norman (feedback must match the person's model: "I marked it done"); Nielsen heuristics 1 and 2 (status in the user's words); Krug (makes the person think). | While an act of this tab's own is in flight, mark only its row ("Done · sending", muted) and keep the toast. Show the banner only when the box refuses or after a delay (for example 5 s without an answer). Never show a hash in it; the Chain page keeps them. |
| X2 | high | Inbox | `390-light-inbox-write.png`, `390-light-inbox-write-full.png` | Nine cards, 267–471 px tall each, so a phone shows about one at a time. Five of the nine are the box's own setup cards ("Set the coding routine's token" and others), which also appear in Settings. The proposals that are actually the secretary's work for the owner (two emails, a changeset, an act) are below them. | Krug (the page's main thing must be obvious); Tufte (data-ink: four labelled blocks per card before its button); Wroblewski (one card per screen is a long scroll for a thumb). | Order proposals before setup. Fold the box's setup cards into one closed row, "Setup · 5 secrets to store", which opens in place and leads to Settings. Draw a card closed by default: its one-line why, its primary button and Dismiss; Because, Where to find it, From and To open on tap. |
| X3 | high | Todos, a todo, Secretary | `390-light-todo-writer.png`, `tap-row-opened.png`, `1440-light-todo-writer.png` | There are four doors to the secretary: the Secretary tab, the floating Ask button, a row's ASK and the todo page's "Ask the secretary". On the todo page the floating Ask covers "Ask the secretary". On the list it sits over the rows. On a wide screen the side panel already holds the todo as a chip and the page still shows its own button. | Rams (as little design as possible: one control per act); Nielsen 4 (consistency); Norman (two controls with one meaning make the person wonder which one differs). | Remove the floating Ask on the phone, where the tab is in the thumb's reach. On a wide screen, remove "Ask the secretary" from the page, since the panel is beside it. Keep a row's ASK, which carries the todo. |
| X4 | high | Every page, wide | `1440-light-todo-writer.png`, `1440-light-inbox-write.png`, `1440-light-conversation.png`, `1440-light-todos.png` | At 1440 px the todo's own page, the thing being read, is the narrowest column (about 400 px). The list takes about 500 px, and an empty secretary panel takes 350 px of nothing but "Whole conversation →". The Inbox and Settings are one 800 px column with that empty panel beside them. The header moves between pages: the brand starts at x = 64 on Todos and at x = 240 in a conversation. In an empty conversation the composer is at the top of the window. | Wroblewski (a wide layout should use the width for the content, not chrome); Tufte (empty panels are non-data ink); Nielsen 4 (the frame should not move between pages). This is the "desktop weaker than the phone" the owner noticed. | One page grid for every page, the header in the same place. Open the secretary panel only when its conversation has something in it, or when the person opens it. Give the reading pane the remaining width, with a measure of about 70 characters. Pin the composer to the bottom on every width. |
| X5 | high | Every page | `loading-todo.png`, `loading-inbox.png`, `loading-settings.png`, `loading-conversation.png` | While a page waits, the person sees one of fifteen sentences: "loading…", "Loading…", "Asking the box for the batches held for you…", "reading the corpus…", "reading the calendar…", "reading the briefing…", "Reading the memory…", "Loading conversations…", "loading Typst…", "Asking the server…", "fetching the chain and checking every object in this tab…", "Asking the box…", "Searching for “…”…", "The box took it; reading its record…" and "The box erased it; reading its record…". A conversation shows nothing, and so looks empty, with the composer at the top until the lines arrive. A todo's title is its id ("file-permit") and changes to its body when the page arrives. The Inbox says "Loading…" and then a different sentence 160 ms later. | Nielsen 1 and 4; the owner's own request for one loading state; Norman (a blank screen is indistinguishable from "there is nothing"). | One loading view in `web/src/view/frame.ts`: the page's frame with grey placeholder lines in the shape of its content, shown after 300 ms (shorter waits show nothing), with no words. Every page passes its shape and no sentence. A conversation draws the placeholder above the composer, at the bottom. A todo's title waits for its body and never shows the id. |
| X6 | high | Todos | `390-light-todos.png`, `390-light-typst.png` | The first todo starts at y = 421 of 844. Half of the first screen is the title, "1 needs attention · 2 wait on you", Group by with three chips, the search field and "Showing Open · all categories". The rows show a pie glyph and no number, so the sort key (the %) has to be decoded from a glyph. The Typst drawing of the same list (`#/typst`) prints the % under each glyph and fits all six todos on one screen. | Tufte (the number is the data; the glyph without it is decoration); Krug; Wroblewski (content first on a phone). The 2026-09-12 audit raised the chrome as finding 2. | Print the % beside the glyph on every row. Fold Group by and the search into one control in the title row (a magnifier and a sort menu). Drop "Showing …" while it says the default. |
| X7 | high | Add sheet | `add-sheet-typed.png`, `tap-add-after.png` | The sheet opens with no category chosen. "Add todo" refuses with "Choose a category.", printed under the button and half under the tab bar. The chips do not change. At 390 px the fifth chip, "other", is cut off at the edge. | Norman (constraints should prevent the error, not report it); Nielsen 5 (error prevention) and 9 (say where the error is). | Preselect a category: the list's open group, or the last one used. If none fits, mark the chip row as required in place, before the press. Let the chip row wrap. |
| X8 | high | Conversation | `chat-thinking-300ms.png`, `chat-thinking-2s.png` | With a model that takes 4 s, the screen at 0.3 s and at 2 s shows the person's turn marked "queued" and a "Queued" line with Stop. No time passes on screen and nothing says the box has started. `web/src/chat/answer.ts` has a "Thinking · N s" state, but this run never reached it before the answer. | Nielsen 1 (beyond 1 s, show progress); Norman (feedback). | Show "Thinking · N s" as soon as the box accepts the turn, not when the model's first event arrives. Keep Stop beside it. Drop the "queued" mark on the person's own bubble once the box has the turn. |
| X9 | medium | Inbox cards | `390-light-inbox-write.png` | Words the owner must decode: "filed by the box itself; no model asked"; "nothing is stored for the coding routine's token on this box"; "Because of proposal:setup/3"; "the coding routine's fire token reads it". The label column wraps "Because / of" and "Where to / find it" onto two lines. The secret's input has no label or placeholder: an empty box beside a pale Store. | Krug (self-evident words); Nielsen 2 (the user's language). | Write the card from the owner's side: "Store the coding routine's token. The routine's page has it under API trigger." Show a reference as its title ("the permit renewal"), not as `proposal:setup/3`. Widen the label column, or stack the label above its value. Give the field a placeholder ("Paste the token"). |
| X10 | medium | Inbox and chat cards | `390-light-inbox-write-full.png`, `chat-changeset-card-answered.png`, `chat-act-card-answered.png` | The "yes" button is Send, Run, Store or Confirm, and the "no" button is Dismiss on four kinds and Deny on the changeset. Every card also has Reply. | Nielsen 4 (consistency). The yes verbs are right to differ: Send says mail leaves, and an irreversible act should be named exactly. | Keep the yes verbs. Use one no verb (Dismiss) everywhere. |
| X11 | medium | A todo | `390-light-todo-writer-full.png`, `390-light-todo.png` | Under the title: "Rendered by typst.", an empty bordered box, then "CONFIRMED · Written by ada on 2026-09-23. A trusted actor stands behind this content, so the fold prices it." At the bottom, the history shows constructor literals (`datetime(2026, 9, 23, 15, 38, 56)`, `Decay(start=0.6, end=0.05, …)`, `('Missing it is not recoverable.',)`) and two hashes, open by default. | Tufte and Rams (it is not used in the task "what is this and what do I do"). The project's own rule pushes the other way: show the working. | Delete "Rendered by typst." and the empty box. Shorten provenance to "Confirmed · ada · 23 Sep" and put the rest in its tooltip. Close the history by default ("History (2 events)", open on tap), and draw its fields as sentences before the literals. |
| X12 | medium | Settings | `390-light-settings-full.png` | On a phone the page is 4,489 px tall. It shows a shell command for a terminal (`sh -c 'mkdir -p Mail/her-at-example-invalid && …'`), the data repository's absolute temp path, and three model lines that each end "Answered 15:38 on 2026-10-13". It also lists the release plan (`suzatarytwo`, `suzatarytwoone`, `suzatarytwotwo` with "In progress. No items yet.") and "Rendered text kept here: 0.0 MB of 20.0 MB (0 pieces)". The setup cards repeat whole. | Tufte; Rams; Krug (a settings page is scanned, not read). | One line per group, saying whether it is connected; open a group to see its lines. Show a terminal command behind "How to set this up at the box". Move the release plan to About. Move the cache size behind "This device". In Settings, refer to a setup card by one line ("Needs the routine's token: store it"), with the card kept in the Inbox. |
| X13 | medium | About | `390-light-about.png` | The theme switch (System, Light, Dark) is on About, below three paragraphs titled "One evaluator, on two machines" about prodrome and WebAssembly. | Norman (discoverability: nobody looks for the theme on About); Rams. | Move Theme to Settings → This device. About keeps the version and a one-line description, with the architecture behind a link. |
| X14 | medium | Conversation | `390-light-chat-email-card-full.png`, `chat-changeset-card-answered.png` | Each secretary answer carries five controls (Copy, Copy Typst, Show as text, Reply, Erase) and a "Checked proposals (1) · 0 s" trace line above it. Each person's turn carries Reply and Erase. In a short exchange there are more controls than sentences. | Tufte (non-data ink); Rams. | One "…" per turn (a long press on a phone) holding Copy, Copy Typst, Show as text and Erase. Keep Reply visible. Show the trace line only when a call failed or took more than a second. |
| X15 | medium | More sheet | `390-light-more.png` | The sheet opens on a switch between "life" and "suzatary", the chains' internal names. It lists Chain and Usage, which are pages about the system, beside Mail and Calendar. | Krug; Nielsen 2. | Name the chains as the owner speaks of them ("My todos", "This app's work"). Move Chain and Usage under About. |
| X16 | medium | Todos, a row open | `tap-row-opened.png` | Opening a row in place prints its body a second time, under the title it already shows. | Tufte (redundant ink). | Delete the repeat. Show the detail's first line instead, or nothing. |
| X17 | medium | Secretary (topics) | `390-light-secretary.png`, `1440-light-secretary.png` | Two text fields ("Search what was said" and "Name it, or leave it blank") stand above the first conversation. At 1440 the second field's placeholder is cut ("Name it, or leave it b…"). Times are written as `2026-10-13 15:38`. | Krug; Wroblewski (inputs at the top are out of the thumb's reach and push content down). | Make New topic a button that opens the conversation and asks for a name there. Put the search behind a magnifier. Use relative times ("15:38", "yesterday"). |
| X18 | low | Inbox, a held batch | `390-light-inbox-held.png`, `1440-light-inbox-held.png` | A batch's metadata line reads "life 2026-10-13 09:00:00 triage 2 changes c3d2202f9195". | Tufte. | Keep "2 changes" and the date. Move the chain, the actor and the hash into the diff. |
| X19 | low | A todo, the list | `390-light-todo.png` | The brand link is 73×13 px, and the history's hash links are 80×13 px. A secretary answer's Copy and Reply are text in 13 px type. These are under the 44 px guidance (Apple) and the 48 dp guidance (Material). | Wroblewski; Fitts. | Pad links that are controls to 44 px of height. The hash links go behind the history disclosure under X11. |
| X20 | low | Todos | `390-light-todos.png` | An unconfirmed todo shows its markup as written ("Email \*Harrow\* about the deed", tagged UNCONFIRMED), and the row truncates to "Email \*Harrow\* about t…". | Krug (asterisks look like a bug). The rule is deliberate (CLAUDE.md: an untrusted record is served as source until confirmed), and this is where the lenses and the security model disagree. | Keep the source. Let the UNCONFIRMED tag carry the explanation in its tooltip ("written by the triage agent; shown as written until you confirm it"), and give the row two lines before truncating. |
| X21 | low | Todos, drawn by Typst | `390-light-typst.png` | The slowest page: 2.7 s to content and 4.1 s to settled, cold at 4x; 1.7 s warm. It shows "loading Typst…" meanwhile. | Nielsen (past 1 s, keep the person oriented). | Fold into X5's placeholder. Compile the kit once per session and keep it, so a warm visit costs only the draw. |
| X22 | low | Inbox, dark | `390-dark-inbox-write.png` | A disabled Store reads as nearly the same tone as the card in dark mode, so the person cannot tell disabled from missing. | Nielsen 1. | Give disabled controls a visible outline in dark mode, or say why it is disabled ("Paste the token first"). |

## Each lens

### Usability (Norman, Nielsen, Krug)

Each page's main action is usually present and usually visible. A writer's
todo page leads with Done and Cancel. The Inbox card's yes-button is filled.
The composer is where a thumb expects it. The failures are at the joins:

- **Feedback** is the weakest heuristic here. Done is followed by an alarm
  (X1). A sent message sits at "Queued" with no clock (X8). The add sheet
  refuses after the press and below the fold (X7).
- **Dead ends:** none hard. Every page has the tab bar, and the todo page has
  its back link. The soft one is the Inbox setup cards (X2): their act needs a
  secret from somewhere else, and "Where to get it" leaves the app.
- **Words to decode** are the most frequent problem (X1, X9, X11, X15, X18).
  The vocabulary is the store's (object, fold, chain, box, actor, proposal
  ids), not the owner's. Krug's test ("can you tell what this is without
  thinking?") fails on every Inbox card.
- **Consistency:** four doors to one secretary (X3), several no-verbs (X10)
  and fifteen loading sentences (X5).

### Nothing unused (Tufte, Rams)

The things on screen that the person does not use in the task, and what to
do with each:

| Where | Element | Verdict | Why |
|---|---|---|---|
| A todo | "Rendered by typst." | delete | says how, not what |
| A todo | Empty bordered box under the header | delete | carries nothing |
| A todo | Provenance sentence ("A trusted actor stands behind…") | move behind a tooltip; keep "Confirmed · ada · date" | evidence is kept, prose is not needed |
| A todo | History with literals and hashes | move behind a closed disclosure | the app's working, wanted rarely |
| A todo | "Why this number" | keep | it answers the question the number raises |
| A todo | 30-day chart | keep, below Why | the product's picture |
| Todos | "Showing Open · all categories" | delete while it says the default | repeats the default |
| Todos | Group by, three chips | move into a menu | used once a week, shown on every visit |
| Todos | Pie glyph without a number | keep, and add the % | the number is the sort key |
| Todos, a row open | Body repeated | delete | already on the row |
| Inbox | "A diff opens on its summary, and settling one is a second tap." | delete | instructions for an interface should be unnecessary |
| Inbox | "filed by the box itself; no model asked" | move into a tooltip on the Setup tag | provenance, not task |
| Inbox | `proposal:setup/3` references | replace with titles | ids are not words |
| Inbox | Batch hash and actor | move into the diff | evidence |
| Chat | Copy, Copy Typst, Show as text, Erase on every answer | move into one menu | used rarely |
| Chat | "Checked proposals (1) · 0 s" | show only on failure or slowness | trace |
| Settings | Shell commands | move behind a disclosure | a terminal task on a phone page |
| Settings | Absolute data path | move behind a disclosure | box fact |
| Settings | Three "Answered … " lines | keep one line per group | repetition |
| Settings | Release plan | move to About | not a setting |
| Settings | Rendered-cache size | move behind "This device" | not used |
| About | Architecture essay | move behind a link | not used after the first read |
| More | "life" / "suzatary" | rename | internal names |
| More | Chain, Usage | move under About | system pages |
| Wide | Empty secretary panel | hide until used | empty |
| Every page | Dashed rule under every title | keep | the design system's mark (typst-design) |

### Mobile and desktop (Wroblewski)

- **Phone widths.** No page scrolls sideways at 360, 412 or 430 px. The
  overflows are the category chip at 360 px and "other" in the add sheet
  (X7).
- **Thumb reach.** The tab bar, the composer and the add sheet's button are in
  the bottom third, where a thumb rests. These sit at the top of the screen,
  out of easy reach one-handed:
  - a writer's Done and Cancel on the todo page (y ≈ 298 of 844);
  - the list's Group by and search (y ≈ 150–270);
  - the Secretary page's two fields.

  The floating Ask is well placed for a thumb, but it duplicates the tab
  and covers content (X3).
- **Wide window.** The desktop is weaker than the phone, as the owner said
  (X4). The phone gives the page the whole width. The desktop divides 1440 px
  into three columns: the list, the page read at 400 px, and a secretary
  panel that is empty on most visits. The frame also moves between pages.
  The phone is the better design because it shows one thing at a time. The
  desktop should show two things at a time, list and page, and the panel
  only when the person has asked it something.

### Loading and feedback

The app is local-first, and it shows. Todos draws from the replica even with
`/fold` held (`loading-todos.png`), and that is the best loading state the
app has: none. Everywhere else the person sees a sentence, a different one
per page, and two pages show something misleading. A conversation looks
empty, and a todo's title is its id until the page arrives (X5). One state
should replace all fifteen:

1. under 300 ms, nothing (the old content stays, or the frame is empty);
2. after 300 ms, the page's frame with placeholder lines in its content's
   shape, with no words;
3. on failure, the one problem line the frame already has, with Retry.

For a tap, the same principle applies. The control that was pressed answers
at once on the row it changed ("Done · sending"), and anything louder waits
until something has actually gone wrong (X1, X8).

## Where two lenses disagree

1. **Tufte against the app's own rule of evidence.** Hashes, literals and
   provenance are the app's "show your working" (CLAUDE.md). Tufte would
   delete them from the default view. The recommendation moves them behind a
   disclosure and deletes none (X11, X18).
2. **Wroblewski against Rams on the floating Ask.** For thumb reach the button
   is well placed. For "as little design as possible" it is a second Secretary
   tab. The tab is also in the thumb zone, so Rams wins (X3).
3. **Norman against Tufte on Done's feedback.** Norman wants the state shown;
   Tufte wants fewer marks. A quiet mark on the one row satisfies both, where
   the red banner satisfies neither (X1).
4. **Nielsen's consistency against naming each act exactly.** Send, Run and
   Store should stay distinct, because Send is irreversible and says so. The
   no-verbs carry no such weight and should be one (X10).
5. **Krug against the trust model on unconfirmed text.** Showing source looks
   like a bug, but the rule stops an agent's markup from rendering until it is
   confirmed. The design rule stands; the explanation moves to the tag (X20).

## Not covered

- Real network latency. The box was on loopback, so the times are CPU only.
- A list of hundreds of todos. The fixture holds six open todos on the life
  chain.
- The Android portal's WebView and its system back gesture.
- Screen readers. Labels were read from the DOM for the controls inventory,
  but no assistive-technology pass was made.
