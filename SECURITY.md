# Security policy

## Reporting a vulnerability

Please report security problems privately, through GitHub's
[private vulnerability reporting](https://github.com/mgbilby/yorc-rs/security/advisories/new)
for this repository, not in a public issue. Include what you ran and, if you
can share it, the input that triggers the problem.

Only the latest release is supported with fixes.

## What yorc-rs protects against

**Documents are untrusted input.** Reading any XML or JSON document or
Public Data File archive, however malformed or hostile, should end in a
value or an error, never a panic:

- Only XML's predefined entities and character references are expanded.
  DTD-declared and external entities are never resolved, and reading never
  touches the network or other files.
- Every archive member read by `dump` is capped (`dump::Limits`, 64 MiB by
  default); the size in the tar header is checked before allocating, and the
  read itself is capped, so a lying header fails too.
- Unknown elements are skipped and unknown vocabulary values kept as
  `Other(..)`.

**The client** (feature `blocking`) reads only, and only ORCID's Public API
(`pub.orcid.org`): it does not provide access to the ORCID Sandbox or the
ORCID Member API. Every URL it sends is checked against its hosts
(`pub.orcid.org`, and `orcid.org/oauth/token` for the optional
`/read-public` token), it follows no redirects, and it keeps to one request
every 45 s to ORCID's hosts. A `Token`'s `Debug` output redacts it.

A panic, a hang, unbounded memory use on a capped path, or a request to a
host outside those hosts is a vulnerability; please report it.
