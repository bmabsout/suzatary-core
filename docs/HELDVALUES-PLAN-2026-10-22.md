# Held values (2026-10-22)

The owner needs the numbers that mail and documents carry: an SSN sent to
counsel, an ITIN on a filing, a passport number on a form. Those numbers are
part of what makes the box useful. They must still never reach a model, a log
or a public tree. A **held value** is the one concept for such a number: the
box holds it, a model reads a placeholder, and the owner's device reveals or
sends it on a tap.

## What a held value is

A held value has four parts:

- **Where the box holds it.** It is never pushed, never in a model's context,
  never in a log line and never in a public tree.
- **A handle**, which names it. The handle is not the value.
- **A kind**: SSN, ITIN, TIN, EIN, verification code, card, account, routing
  number or IBAN. Passport and file come with the stages below.
- **A hint**: the last four digits, which a person needs to tell two values
  apart. A code has no hint, since its last four digits are most of it.

The box offers three uses of a held value, each on the owner's tap:

1. **REVEAL.** The owner's trusted device asks for one value by its handle.
   The box answers that request once, and keeps the value in no store.
2. **SEND.** A drafted email names a held value by its handle, as it names an
   attachment. The send card shows the recipients and says that a held value
   is included. Only the tap that sends puts the real value into the outgoing
   message.
3. **FIND.** A search says that a message names a held value of a kind. It
   never says the value.

## The finding that started this

The brief said that sent mail naming an identity number never reached the
notmuch corpus. Nothing in this repository drops, skips or leaves unindexed a
message because it names a number:

- the sync, the IMAP pulls, lieer and mbsync, and `notmuch new`;
- the older-mail search;
- the reader's `search`, `show` and `thread`, and the triage queue;
- the send paths, which write no copy (sent mail arrives through the next
  sync).

The places that look for a number act on a PDF (`file_read`), an upload
(`attachments`, `vault`), a Drive read, or a stored conversation line
(`Scrubbed`), never on a message. The real notmuch config and `mbsyncrc` are
in the data repository, so a `[new] ignore=` rule there is the one place left
to look, on the box.

The code had the opposite gap. The index is the box's own and may hold the
numbers. But `show`, `thread` and a search row gave the number to the
secretary and the triage agent.

## Stage 1 (this change): the handle, the placeholder for mail text, and reveal

**The detection** is `suzatary_core::redact`, unchanged in what it finds.
`redact::identity_numbers` returns each match's range and its `Kind`. The kind
comes from the closest label within 20 characters, or from the shape of the
number when there is no label (a written-out number in area 9xx is an ITIN,
any other is an SSN). `redact` is that function with each match replaced by
`[redacted]`, as before.

**The handle's spelling** is `held:<Message-ID in base64url, no padding>.<n>`:

- The Message-ID is the one notmuch indexes the message under.
- `n` is the value's place among the message's values (`held_value::Values`).
  The values are counted in one fixed order: every header value and text part
  of the message file as `mail-parser` decodes it, forwarded messages
  included; then the header values by name and the body as the notmuch reader
  shows them. Two spellings of one number (`000-12-3456`, `000 12 3456`) are
  one value.
- `Handle::print` and `Handle::parse` round-trip, and a proptest pins it.

**The placeholder** a model reads is
`[SSN ending 3456 · tap to show · held:…]`. It is
`[SSN ending 3456 · withheld]` where the box cannot read the text again:
Junk read from its server, a web page, a calendar, or a message that no
longer reads as holding the number. The Rust spells it (`Placeholder::print`),
and `web/src/view/heldvalue.ts` reads it. `web/test/heldvalue.test.ts` pins
both spellings character for character.

**Where it is applied.** There are two doors, and every model-facing reading
passes one of them:

1. `suzatary_io::mailbox::Notmuch` masks every text it answers: a row's sender
   and subject, a message's headers, body and links, and a thread. So the
   secretary, the triage agent and the app's mail pages get revealable
   placeholders.
2. The verb dispatch of each surface (`secretary::run`, `agent::call`) passes
   every string of every answer, object keys included, through
   `held_value::withheld_value`. So a number that a verb read from anywhere
   else (Junk, the web, the calendar, a todo's text) reaches the model as a
   `withheld` placeholder. A placeholder reads as no number, so masking twice
   changes nothing.

The maildir and the notmuch index are untouched. The owner may search by the
number, and the box finds the message.

**Reveal.**

- The route is `POST /api/held/reveal` with `{"handle": "held:…"}`.
- It is behind the tap guard: a trusted device key of an actor that a tailnet
  login in `web.act_as` maps to, plus the CSRF triple. An untrusted device, a
  viewer and loopback are refused.
- The `Reveal` capability is held by the server's state and by no record, so
  no verb of the secretary or the agent can call it.
- The route reads the value again from the message file, through the same
  reader the mask used, and answers `Shown { value }` or `Gone`.
- The box keeps the request's record, as it does for every signed request
  (who asked, when, and the status, never the body). In the answer's place it
  keeps only `{"type":"NotKept"}` (`reply::Keeps::Instead`), so a replay of
  the request is answered with that.
- `held_value::Shown` and `wire::RevealPayload` print no value in a `{:?}`.
- The page draws a placeholder as a chip (`SSN ••••3456` and Show), in the
  chat's turns and answers and in a mail body. On a tap, the value replaces
  the dots in that one node and nowhere else: not in a page state, a cache, a
  replica or a stored line.

**What stage 1 did not cover:**

- **Attachments.** A text attachment's numbers still arrive as `[redacted]`
  (`file_read`), and a PDF naming one is still withheld. Both guards are kept.
- **Patterns.** The patterns detect SSNs and ITINs written out, and SSN, TIN,
  ITIN and EIN numbers of nine digits beside their label. They miss
  verification and two-factor codes, bank and card account numbers, routing
  numbers, passport numbers and A-numbers. This stage does not widen them. The
  link pass (`redact::unlink`), which removes a `code=`, `token=` or similar
  parameter and any JWT, runs only on stored conversation lines and does not
  reach a model's answer.

## Stage 2 (built): the wider patterns and SEND

**The patterns.** `redact::identity_numbers` also finds these kinds. Each
format is chosen so that dates, prices and phone numbers do not match:

- a verification or two-factor code: four to eight digits, or `ddd ddd`,
  after a label that names the code with its noun (verification code,
  security code, one-time password, and the words for a verification code in
  Portuguese, Spanish, French, German and Arabic) within 60 characters on its
  line, or within 20 characters of such a label or of verification, verify,
  passcode, OTP, 2FA or MFA; four or five digits right after `code` or
  `code:`; six to eight digits within 20 characters of code, codes or PIN,
  unless the word before makes it a postal, zip, promo, error or dialling code.
  A label claims only the first number beside it; a closer order, invoice,
  reference, flight, room, fee or total label claims the number instead; a
  year and a `yyyymmdd` date are not codes; and no label is read inside a
  placeholder;
- a card number anywhere: a stretch of digit groups of 13 to 19 digits with a
  Visa, Mir, Mastercard, American Express, Discover, UnionPay, RuPay, Maestro
  (16 to 19 digits only), JCB or Diners prefix at one of that issuer's lengths,
  and a valid Luhn check digit, never after a `+`;
- a bank account number: six to seventeen digits, in a row or in groups,
  within 20 characters of account, acct, a/c, checking or savings, unless an
  order, invoice, tracking, booking, telephone or help label is closer, and not
  grouped as a phone number (`ddd-ddd-dddd`) or a date;
- a routing number: nine digits within 20 characters of routing, ABA, RTN or
  transit, whose ABA checksum holds;
- an IBAN anywhere, in either case, whose mod-97 check holds.

A labelled code, account or routing number must stand alone: no `+` or
currency beside it, no digit beside it across a hyphen, dot, comma or slash,
and no run of three digits or more across one space ("code 482913 10 minutes"
stays a code). When matches overlap, a code wins (its last digits are most of
it), then a kind whose checksum held, then the closest label. A value shorter
than nine digits shows no digits in its placeholder. The same patterns redact a
document's text and a stored conversation line, through `redact`. Every model
client takes `for_model::ForModel`, whose one constructor masks, so every text
sent to a model passes the masker: the secretary's prompts, the compactor's,
and the triage agent's tool answers.

**SEND.** The plan put the handles in a field of the draft beside its
attachments. The built design keeps them in the body instead, so the stored
`Email` literal does not change:

- The secretary copies the placeholder it read into the body of a
  `propose_email` draft, where the number belongs. `propose_email` refuses a
  placeholder in the subject, a `withheld` one, a code (the owner types a code
  themselves), and one whose handle does not decode.
- `held_value::Written::parse` reads a body once into text and `Placeholder`s;
  `propose_email`, the card's reading and `fill` use that one parse.
- Each value must come from a message the conversation read
  (`secretary::read_mail::ReadMail`: the messages a `show`, a `search` or a
  reference answered), checked when it is proposed and again on the tap. The
  mail reader writes a placeholder in raw mail that names another message as
  withheld, so a sender cannot have another message's value sent.
- The card's sentence names each value, its message (sender, subject, date)
  and every To and Cc that receives it; its question names the value, and the
  page draws the placeholder as a chip.
- The tap's `mailed` step calls `held_value::fill` with the server's `Reveal`.
  `fill` reads each value again, checks that its kind and hint print the same
  placeholder the card showed, and returns a `Filled` body. `Post::send` takes
  that body, and `commands::mail::deliver` stays the one file that names the
  transport.
- A value that is gone, changed or unreadable refuses the send with a sentence
  that names the value by its kind and hint.
- The stored proposal, the inbox and the transcript keep the placeholder.
  `Filled` and `Shown` print no value in a `{:?}`.

**Forwarding a stored message (built).** `propose_forward` sends one stored
message whole, so a held value in it leaves as the message holds it:

- The proposal (`store::Forward`) records the message's notmuch id, its file's
  size and SHA-256, its attachments by their place in the file, and the held
  values in its text by kind and last four digits (`held_value::Values::held`).
  It holds none of the original's words. A file over 20 MB is refused after a
  `stat`, before it is read.
- The card shows the recipients, the original, its attachments and each held
  value in its text ("SSN ending 3456"). The question names them too. A value
  inside an attachment is not named.
- The tap reads the original again through `Reveal::original`, which the
  server's state holds and no record does. It reads the file once, and only
  while it is still the recorded size. The text and the attachments, or the
  file itself, are built from that one read and sent only while its digest
  holds. The `.eml` part goes as 7bit or 8bit (RFC 2046 §5.2.1).
- The draft it builds goes to the same `Post`, and nothing stores it. A file
  that changed since the proposal refuses the send and says to ask for a new
  forward. `propose_email` cannot attach a stored message.
- `app/cli/tests/forwarding.rs` pins the boundary: the model's answer, the
  card and every store hold the placeholder, in both of the number's
  spellings, only the send holds the number, and two attachments of one name
  are both sent.

## Stage 3: the attachments and the PDF hold fold in

A file the box holds back today becomes a held value of kind `file`, under
the same handle shape (`held:<store>.<id>`): an upload kept private
(`vault`), or a PDF that `FileRead` withholds for naming a number. Its hint is
the file's name and size. REVEAL opens it on the owner's device, and SEND
attaches it on the tap. `FileRead`'s `[redacted]` text becomes placeholders
whose handles name the file and the place in its text layer.

## Stage 4: FIND and passports

- A search row says `names a held SSN`, from the values the mask counted,
  never the value.
- Passport numbers and A-numbers arrive with their own patterns and tests.
