# The deployed server on a quiet box, 2026-09-10

The port's speed claim had no clean measurement: every number before this
was taken while agents built on the same two cores. This is the deployed
binary (`2047x6ra…-suzatary-0.14.0`, Prodrome 0.5.0) on the box at load
0.6, over TLS on the tailnet from the box itself (a new connection per
request, so the handshake is inside every number — `/api/tip` is the
floor), 20 requests per route, the live chain of 688 objects and 58 open
todos, the markup cache warm.

| route | median ms | p95 ms | body |
|---|---|---|---|
| `/api/tip` | 4.2 | 7.2 | 150 B |
| `/api/fold` | 15.6 | 20.4 | 740 KB |
| `/api/chain` | 17.9 | 24.3 | 1280 KB |
| `/api/todo/<one todo>` | 17.0 | 21.9 | 39 KB |
| `/api/mail/inbox` | 100.4 | 108.9 | 20 KB |
| `/api/suzatary/fold` | 5.3 | 7.2 | 20 KB |
| `/api/briefings/2026-09-09` | 4.6 | 5.9 | 5 KB |
| `/` | 5.3 | 8.3 | 3 KB |

For scale: at stage 2 (2026-09-07, loopback, a loaded box) the Python served
the fold in 39 ms and the chain in 78 ms, and the Rust in 33 and 40. The
mail inbox is the one route over 20 ms, and its cost is the notmuch
subprocess and reading the newest messages' files, not the fold.

How to take it again: the box quiet (`uptime`, no cargo or nix build
running), then for each route twenty `curl -sk -o /dev/null -w
'%{time_total}'` and the 10th and 19th of the sorted times.
