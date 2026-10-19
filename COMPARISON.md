# `yorc-rs` vs `orcid` (orcid_rs): features, field coverage, shared code

Compared: `orcid` 0.2.3 (crates.io, MIT; repository github.com/magnusmanske/orcid,
earlier measured at github.com/magnusmanske/orcid_rs @ `2b16b36`, 2026-05-20;
3,986 lines of Rust in the published package) against `yorc-rs` 0.1.0
(11,362 lines of Rust in `src/`, doc comments and doctests included).

**Status, stated plainly (2026-10-17):** done in yorc-rs: the typed model
of the whole v3.0 record schema, XML read and write, JSON read and write,
the yorc-duckdb section rows, Public Data File
streaming, and the blocking Public API client with search and an optional
`/read-public` token. Not done: an async client (a stub was removed; future
work). Out of scope: access to the ORCID Member API or the ORCID Sandbox,
and sign-in (3-legged) OAuth. The client reads any public record; it applies no consent or other
policy (callers that need one wrap it).

## Features

| feature | orcid 0.2.3 | yorc-rs 0.1.0 | planned |
|---|---|---|---|
| Data model | `serde_json::Value` + accessor methods | typed structs for every data-bearing XSD element (`model`); yorc-duckdb's 11 record tables plus `search_results`, `errors` and `research_resource_hosts`, 300 columns (`rows`) | |
| Formats read | Public API JSON | ORCID XML (`xml`) and JSON (`json`) | |
| Formats written | none | XML and JSON, schema-exact and deterministic (`xml::to_string`, `json::to_string`) | |
| Public API reads | `/record` (via `author`), search | `/record` (typed, or the exact bytes), `/person`, `/works`, `/work/{put-code}`, `/works/{put-codes}`, any section path, `/search`, `/expanded-search` (`blocking`) | |
| OAuth | none | optional client-credentials `/read-public` token; no sign-in (3-legged) OAuth | refresh, revocation |
| Member API | none | none: out of scope (no Member API or Sandbox access) | |
| Bulk | none | bulk work reads (`works_bulk`) | |
| Public Data File | none | streamed `.tar.gz`, `.xml` members with iD and parsed document (`dump::records`, a per-member size limit, 64 MiB by default), seeded reservoir sampling (`dump`) | |
| Search | `search`, `search_doi`, `SearchBuilder` (iDs only) | `Client::search` (iDs) and `Client::expanded_search` (names, emails, institutions), one page per call (`start`, `rows` ≤ 1000), typed results | |
| Rate limiting / retries | none | one process-wide limiter for ORCID's hosts (one request per 45 s, one at a time), `Retry-After`, at most 3 attempts, no retry of POST or after a 429, no redirects followed | |
| Async / blocking | both (reqwest; tokio "full" always on) | blocking (ureq 3, no async runtime) | async, feature-gated |
| Sandbox / production | `base_url` override | production Public API only: `Access::Public`, a closed host set (`Custom` for mock servers, refusing ORCID's hosts); no Sandbox | |
| iD validation | MOD 11-2, but accepts `X` anywhere, drops other chars | strict (position, characters, URL forms), yorc-duckdb's rules | |
| SQL / DuckDB | none | `rows` gives the yorc-duckdb extension's rows without DuckDB | checked against the extension itself |

## Field coverage by section

Counts are distinct data fields a caller can read. orcid 0.2.3 counted from its public
accessors; yorc-rs as the columns of its `rows` tables, which are yorc-duckdb's
(`yorc-duckdb/src/core/sections.cpp`, listed in `yorc-duckdb/docs/REFERENCE.md`).
The typed `model` carries more than the tables: every data-bearing element and
attribute of the record XSDs (checked by `tests/schema_coverage.rs`).

| section | orcid 0.2.3 | yorc-rs (`rows`) | not in orcid 0.2.3 |
|---|---|---|---|
| Person: name, biography | 5 (given, family, credit, biography, iD) | 13 | name/biography visibility, last-modified, path |
| Other names | 1 (content) | 18 (`person_items`) | put-code, display index, visibility, source, dates |
| Researcher URLs | 2 (name, url) | ↑ same table | same item metadata |
| Keywords | 1 (content) | ↑ | same |
| Person external ids | 2 (type, value) | ↑ | url, relationship, item metadata |
| Emails | **0** | ↑ (value, verified, primary, …) | whole section |
| Addresses | **0** | ↑ (country, …) | whole section |
| Works | 4 (title, type, pub date, external ids) | 32 + contributors 11 + external ids 12 | subtitle, translated title, journal, short description, citation, URL, DOI, language, country, contributors, put-code, visibility, group, source, dates |
| Fundings | 12 | 35 | contributors, put-code, visibility, group, source, dates, organization disambiguation |
| Education, employment | 6 each (`Role`) | 30 (`affiliations`) | URL, put-code, visibility, group, source, dates |
| Membership, qualification | 7 each | ↑ same table | put-code, visibility, group, source, dates |
| Distinctions, invited positions, services | **0** | ↑ same table | three whole sections |
| Peer reviews | 11 | 35 | subject container name, subtitle, translated subject name, convening-org disambiguation, item metadata |
| Research resources + items | **0** | 30 + 13 | whole section |
| History | **0** | 14 | whole section |
| Item metadata (every activity) | **0** | put-code, display index, visibility, path, source name/client/ORCID, assertion origin, created, last-modified | all |

## Shared code

Measured on 2026-10-17 over `src/` of both packages (before the documentation pass that added doc comments, the search requests and `dump::records`; comments are ignored by the measure) (the orcid 0.2.3 `.crate`
from crates.io), with a script that trims whitespace and ignores trivial lines
(blank lines, punctuation-only lines, comments, `use`, attributes, anything
under 12 characters), then compares distinct lines and distinct windows of 4
consecutive non-trivial lines:

| measure | yorc-rs | orcid 0.2.3 | identical |
|---|---|---|---|
| distinct non-trivial lines | 4,802 | 1,416 | **26** |
| distinct 4-line windows | 5,788 | 1,932 | **1** |

The identical lines are generic Rust idioms (`.into_iter()`,
`fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {`, `match self {`,
`impl Client {`) and field names both take from the ORCID schema
(`pub struct Work {`, `start_date: None,`, `subject_url: None,`). The one
shared window is an empty constructor listing ORCID's affiliation fields in
schema order: `start_date: None, end_date: None, organization: None,
external_ids: Vec::new()` (`Affiliation::new` here, `Role::new` in orcid
0.2.3; the lines between them, such as `url: None,`, are under 12 characters
and ignored).

No longer run is shared, so no code was copied in either direction. Both implement the
ISO 7064 MOD 11-2 check digit, which is a published algorithm. Their code differs, and
their behaviour differs (see *iD validation* above).
