# The secretary reads the owner's documents, by permission — 2026-09-25

The owner, 2026-09-24: "The secretary should be able to access things like my
sensitive docs (it should ask for permission, but the proton docs and such
should be allowed)". Chain: `secretarydocs` (project chain `suzatary`).
Replaces CLAUDE.md's "Proton Drive = main-session only".

## 1. The shape: the web pattern, pointed at Drive

Listing names is a read; reading a file's text is asked for, like a web
request, unless the folder is allowed. Allowing a folder is the card's
second answer, as "Always allow" is for a host. Drive is reached only
through the official CLI (`~/bin/proton-drive`, already signed in for the
vault backup).

## 2. Rules, each a mechanism

1. **A capability that cannot write.** `Drive { list(path), read(path) }`.
   Its io interpreter builds argv from a closed enum of `list`, `info` and
   `download` only, so trash, move, upload and delete are unreachable from
   any verb. `DrivePath` is parsed: absolute, no `..`, bounded.
2. **Two verbs.** `drive_list(path)` answers names, kinds, sizes and dates,
   with no tap. `drive_read(path, why, sources)` runs at once under an
   allowed folder and otherwise stores `Act::DriveRead { path, folder }`.
   The card names the file and its folder; its answers are Read and Always
   allow this folder, which appends to `[secretary] drive_allow`.
3. **Text only, through the attachments reader.** A download lands in a
   scratch directory, the attachments series' text extractor reads it
   (text and PDF; anything else is named and refused), and the file is
   deleted in the same call.
4. **SSN and TIN never enter a transcript.** Extracted text passes through
   `redact` before it reaches the conversation or the model:
   `ddd-dd-dddd` and nine-digit runs labelled SSN, TIN or ITIN become
   `[redacted]`. The card says so. When a task needs the number, the
   secretary says which file holds it, and the owner copies it himself.
5. **Nothing else changes.** The triage agent gets nothing; no bundle gains
   a sending trait; transcripts stay git-ignored and box-local.

## 3. The series

1. io: `Drive` over the CLI (list, download), `DrivePath`, `redact` with
   tests of its patterns.
2. core: the verbs, `Act::DriveRead`, its card, the allow rule and its
   matcher, and the surface snapshot.
3. cli: `[secretary] drive_allow`, read live; the tap reads and appends the
   text as the verb's answer; "Always allow" edits the instance file through
   `settings::Edits`.
4. docs: SECRETARY.md (when to read, never quote an ID number back),
   CLAUDE.md's Drive rule replaced.

Depends on the attachments series (`showattachments`) for text extraction.
