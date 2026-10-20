# A photo in the chat — 2026-09-22

The owner, 2026-09-22: "I have no way of sending a photo in the chat."
Chain: `chatphotos` (project chain `suzatary`).

## 1. What exists, and what is missing

A person's turn is `Turn::Person { text, references }`; the composer has no
file input, the say route takes JSON, the session is fed text on `claude
-p`. The CLI (2.1.280) accepts an `image` content block on `--input-format
stream-json` and the model reads it, so vendor, fence and session need no
change.

## 2. Rules, each a mechanism

1. **Stored once, by content, beside the conversations.**
   `secretary/attachments/<sha256>`, git-ignored and box-local like the
   transcripts; a turn names hash and media type, never bytes.
   `AttachmentId` is a smart constructor over 64 hex digits, so no path
   passes through it.
2. **Only the four image types the model accepts.** `MediaType` is a closed
   enum (`jpeg`, `png`, `webp`, `gif`) checked against the file's magic
   bytes, never the browser's header; nothing that renders as a document.
   5 MB per photo (the API's ceiling).
3. **The browser re-encodes before upload.** Canvas redraw to JPEG, longest
   edge 2000 px: EXIF (location included) is stripped on the phone, HEIC
   becomes JPEG, rule 2's size holds. The sizing is a pure, node-tested
   function.
4. **Uploading is a write, behind the say gate.** `POST
   /api/secretary/attachments` takes raw bytes under the act triple and a
   named peer in `web.act_as`; loopback cannot. It answers the hash and
   stores nothing else. `GET …/attachments/<hash>` is behind the read gate,
   served with its stored type and `nosniff`.
5. **A turn carries its photos; the model sees each once.** `Turn::Person`
   gains `attachments: Vec<Attachment>` (`#[serde(default)]`, so stored
   transcripts parse). A resumed session gets only the new turn; a fresh
   session gets the whole transcript, with photos from the last eight
   turns and a text placeholder for older ones.
6. **What crosses is what the transcript names.** The session uses
   `--input-format stream-json` only for a turn with a photo, reading bytes
   from rule 1's store by hash; an unknown hash is a refusal naming it,
   never a silent drop.

## 3. What this does not change

Sending stays human-only; the secretary's bundle gains no trait; the photo
goes to the vendor the conversation already uses. Triage never sees an
attachment. Photos are not in the vault (the phone has them).

## 4. The series

1. core: `Attachment`, `AttachmentId`, `MediaType` (magic-byte sniff), the
   `Turn::Person` field, a transcript-compatibility test.
2. io: the content-addressed store; stream-json input for turns with
   photos.
3. core + cli: upload and read routes, gated, with refusal tests.
4. wire + web: composer photo button, pure resize, transcript thumbnails.
5. e2e: attach, send, see it in the transcript; scripted model sees the
   hash.
