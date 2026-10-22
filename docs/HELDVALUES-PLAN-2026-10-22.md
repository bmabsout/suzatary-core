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
- **A kind**: SSN, ITIN, TIN or EIN today. Passport, code, account number and
  file come with the stages below.
- **A hint**: the last four digits, which a person needs to tell two values
  apart.

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

**What it does not cover yet:**

- **Attachments.** A text attachment's numbers still arrive as `[redacted]`
  (`file_read`), and a PDF naming one is still withheld. Both guards are kept.
- **Patterns.** The patterns detect SSNs and ITINs written out, and SSN, TIN,
  ITIN and EIN numbers of nine digits beside their label. They miss
  verification and two-factor codes, bank and card account numbers, routing
  numbers, passport numbers and A-numbers. This stage does not widen them. The
  link pass (`redact::unlink`), which removes a `code=`, `token=` or similar
  parameter and any JWT, runs only on stored conversation lines and does not
  reach a model's answer.

## Stage 2: SEND

A `propose_email` draft carries held values by handle in a field of their own,
beside its attachments, never in the body text. The card shows the
recipients, the subject, the body with each placeholder in place, and a line
"includes your SSN ending 3456". The tap that sends reads each value through
`Reveal` inside the postman (`commands::mail::deliver` stays the one file
that names the transport) and substitutes it into the outgoing MIME. The
stored proposal, the inbox and the transcript keep the handle. A value that
reads as `Gone` at the tap refuses the send by name.

## Stage 3: the attachments and the PDF hold fold in

A file the box holds back today becomes a held value of kind `file`, under
the same handle shape (`held:<store>.<id>`): an upload kept private
(`vault`), or a PDF that `FileRead` withholds for naming a number. Its hint is
the file's name and size. REVEAL opens it on the owner's device, and SEND
attaches it on the tap. `FileRead`'s `[redacted]` text becomes placeholders
whose handles name the file and the place in its text layer.

## Stage 4: FIND and the wider patterns

- A search row says `names a held SSN`, from the values the mask counted,
  never the value.
- New kinds (passport, account number, code) each arrive with their own
  pattern and its tests, once the owner chooses the patterns. A code is short
  and everywhere, so that choice is a policy, not a regex.
