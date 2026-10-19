# `yorc-rs`: architecture

A Rust library for the ORCID message schema v3.0: iDs, a typed model of the
whole record schema, XML and JSON read and write,
the yorc-duckdb section rows, Public Data File streaming and a blocking
ORCID Public API client for occasional single-record retrieval. It is general purpose: no policy, no consent rules, no
jurisdiction logic. Callers decide what to read and why.

yorc-rs is an independent open-source project, not affiliated with,
endorsed by or sponsored by ORCID, Inc. It does not provide access to the
ORCID Sandbox or the ORCID Member API: its client reads only ORCID's Public
API (`pub.orcid.org`).

The package is `yorc-rs`; the import name is `yorc`. It was
developed inside an application and split out as a standalone crate on
2026-10-17; applications keep their own rules (which records they may read,
and why) on top of it.

See [COMPARISON.md](COMPARISON.md) for a feature, field and shared-code
comparison with `orcid` 0.2.3.

## Why a new crate

`orcid` 0.2.3 (crates.io; source: github.com/magnusmanske/orcid_rs, MIT) reads the
Public API's JSON and wraps it in a dynamic `serde_json::Value` with accessor methods
(`Author::works()`, `employment()`, …). It is read-only and has no typed model, no XML,
no write documents, bulk or Public Data File support. It also lacks history,
distinctions, invited positions, services, research resources, contributors and full
items. Its iD check accepts `X` in any position and drops other characters silently.
Reaching the scope below would mean replacing its core, so `yorc-rs` is a new
crate covering the whole ORCID message schema v3.0 (`reference/orcid-model`, ORCID
MIT-style licence), read and write, in XML and JSON.

## Layout

```
yorc-rs/
  Cargo.toml          [package] yorc-rs, its own empty [workspace]
  src/
    lib.rs, error.rs  Error, Result
    id.rs             OrcidId, iD normalisation (mirrors yorc-duckdb)
    hash.rs           SHA-256 hex                              (feature hash)
    model/            common, activities, person, record (+ Document/Root), group_id
    xml.rs, xml/      ORCID XML -> model; xml/write.rs: model -> XML (write/group_id.rs: group-id documents)
    json.rs           ORCID JSON <-> model                     (feature json)
    rows.rs, rows/    model -> the yorc-duckdb section tables (11 record + 2 API-message + hosts) (feature rows);
                      rows/extra.rs: the opt-in timeline and group_ids; rows/changes.rs: snapshot diff
    dump/             streaming .tar.gz; reservoir.rs: SplitMix64 + Algorithm R (feature dump)
    blocking.rs       ORCID Public API client (reads, search, /read-public token) (feature blocking)
  examples/           one small program per module (README.md, Examples)
  tests/              samples, write and JSON round trips, schema walk, mock-server client tests
  tests/data/         0000-0003-0100-6634.xml (a real public record, CC0); changes/ (two snapshots
                      of ORCID's record sample); group-id-records.xml
  reference/orcid-model/   ORCID's record_3.0, group-id-3.0 and common_3.0 XSDs and samples (see NOTICE)
```

The library has **no DuckDB dependency**. `rows` reproduces the extension's
tables without it.

## Modules

| module | contents |
|---|---|
| `id` | `OrcidId` newtype: parse/normalize (URL forms, missing hyphens, lowercase `x`), ISO 7064 MOD 11-2, `Display`, `FromStr`. Same rules as yorc-duckdb's `orcid_normalize`. |
| `model::common` | `Source`, `Organization`, `DisambiguatedOrganization`, `FuzzyDate` (year, optional month and day), `ExternalId` + `Relationship`, `Visibility`, `Url`, `Amount`, `Title`/`TranslatedTitle`, `PutCode`, `LastModified`. |
| `model::activities` | `Work`/`WorkSummary`, `Funding`/`FundingSummary`, `Affiliation` (one struct with an `AffiliationKind` for the seven kinds: distinction, education, employment, invited position, membership, qualification, service), `PeerReview`/`PeerReviewSummary`, `ResearchResource`/`ResearchResourceSummary` + `ResourceItem`, `Contributor`, groups, `ActivitiesSummary`. Closed vocabularies (work type, funding type, contributor role, citation type, peer-review role/type, resource type) are enums with an `Other(String)` fallback, so new ORCID values never fail a parse. |
| `model::person` | `Person`, `Name`, `Biography`, `OtherName`, `ResearcherUrl`, `Email`, `Address`, `Keyword`, `PersonExternalIdentifier`. |
| `model::record` | `Record`, `History`, `Preferences`, `Bulk` (items + per-item `Error`), `OrcidError`, `Search`/`ExpandedSearch` results, `Document`/`Root`. |
| `model::group_id` | `GroupIdRecord` (put-code, name, group-id, description, `GroupType`, source, dates) and `GroupIdRecords` (a page: total, page, page size, records), as ORCID's `GroupIdRecord`/`GroupIdRecords`. The XSD's inline source fields read into `source`. Written outside the record emitter (`xml/write/group_id.rs`), since `group-id` is not one of the record namespaces of `xml::Ns`. |
| `xml` | ORCID XML read/write with `quick-xml` (no serde). Namespaced element names come from the XSDs; writing emits the prefixes and namespaces ORCID uses. Reads are lenient about unknown elements; writes are exact. |
| `json` | ORCID's JSON form (`{"value": ...}` wrappers, kebab-case keys) over the same model. |
| `blocking` (feature `blocking`) | ORCID Public API client over `ureq` 3: `Access::Public` (`pub.orcid.org`, and `orcid.org/oauth/token` for the optional token), plus `Access::Custom` for mock servers (which refuses ORCID's hosts). Reads of a record by iD (`record`, `record_raw`, `person`, `works`, `work`, `works_bulk`, `section`), anonymous or with an optional `&Token`; `search` / `expanded_search`, one page at a time. `Client::read_public_token`: the 2-legged `/read-public` token, a `Token` with its `Realm`; only such tokens are sent. No Member API, no Sandbox, no sign-in (3-legged) OAuth, no writes. One process-wide limiter for ORCID's hosts (one request per 45 s, one at a time), retries of GET only (at most 3 attempts), no retry after a 429, no redirects followed, every URL checked against the access's hosts, `error-3.0` bodies parsed. |
| `dump` (feature `dump`) | Streaming ORCID Public Data File reader (summaries and activities tarballs) without extracting to disk; `dump::records` yields each `.xml` member (path, stem, bytes; `orcid()`, `parse()`); every member read is capped by `dump::Limits::max_member_bytes` (64 MiB by default, yorc-duckdb's `max_member_bytes`; `records_with`, `read_entry_with` for another limit); `dump::reservoir`: a seeded, reproducible reservoir sampler. |
| `rows` (feature `rows`) | Flatten the model into the exact rows and column names of `yorc-duckdb`'s eleven record sections (works, fundings, affiliations, …), of its `search_results` and `errors` sections (search and expanded-search results; standalone and bulk errors, with the locked/deprecated iD fallback and `primary_orcid`) and of `research_resource_hosts` (every host of a proposal or resource item). Opt-in, outside `Section::ALL` and `Tables` (which they leave unchanged): `rows::Extra::Timeline` / `rows::timeline` (yorc-duckdb's `orcid_timeline`) and `rows::Extra::GroupIds` / `rows::group_ids` (`orcid_group_ids`), built by their own walkers; `rows::changes` (`orcid_changes`), the field-level diff of two snapshots, over the rows of the eight record sections. |

## Network access

- **Occasional single-record retrieval.** `blocking` reads one record at a
  time and runs the optional `/read-public` token request. Every request to
  ORCID's hosts (`orcid.org` and its subdomains) goes through `RateLimiter::orcid`, one per process: one request
  every 45 s, one at a time, retries and token calls included; a faster
  limiter is clamped, only `Access::Custom` (mock servers) may go faster.
  There are no crawling, paging-through-everyone or parallel-fetching
  helpers. Bulk data comes from the Public Data File through `dump`, which
  reads local files and is not throttled (nor are dump downloads from
  Figshare or ORCID's S3/CDN mirrors, which this crate does not perform).
- **The Public API only, a closed host set.** `Access::Public` names its
  two base URLs (`https://pub.orcid.org/v3.0`, `https://orcid.org/oauth`);
  `Access::permits` checks every URL just before it is sent (the token call
  included), and a `Custom` access refuses every ORCID host. Tokens carry
  the `Realm` they were issued for; an empty token, a token of another
  realm or one without the `/read-public` scope is `ClientError::Usage`
  before anything is sent.
- **No Member API, no Sandbox, reads only.** Access to the ORCID Member API
  and the ORCID Sandbox, sign-in OAuth, and writes (payload validation and
  shaping, create, update and delete) are out of scope for this crate.
- **Search**: `Client::search` / `expanded_search` read one page of results
  (`rows` up to 1000), paced like every request; it finds records, it is not
  a bulk-export path.
- What a caller reads, and on what basis, is the caller's business. An
  application that may only read records whose owners consented wraps the
  client with that check; nothing of that lives here.
- Nothing else touches the network: `dump` reads local files.

## Design rules

- **Schema-derived, not hand-picked.** Every element and attribute in the v3.0 XSDs maps
  to a field. A test walks the XSDs and fails if a field has no model counterpart.
- **Round-trip.** Every file in `reference/orcid-model/record_3.0/samples` must read and
  write back to equivalent XML (canonicalised: whitespace, attribute order, prefixes).
  Every JSON sample (`record-full-3.0.json` and the write samples) round-trips through
  `json`, and JSON ↔ XML give the same model.
- **No panics on data.** Unknown enum values become `Other`, missing optional elements
  become `None`, and malformed documents return `yorc::Error` with the element path.
- **Bounded memory on archives.** A Public Data File member is read whole, so its
  size is capped (`dump::Limits`, 64 MiB by default, as yorc-duckdb's
  `max_member_bytes`): a gzip bomb is an `io::Error`, not an unbounded allocation.
- **Documents are written whole.** `to_string` writes a document as the
  model holds it (no validation): the round-trip and the Rust/Zig
  byte-identity checks use it.
- **Rows mirror yorc-duckdb column for column.** A column added to a section
  is appended after the existing ones (as yorc-duckdb does), so positions
  never move; `tests/samples.rs` pins the widths and tails.
- **Features keep it light.** Default: `id`, `model`, `xml`. Opt in to `json`,
  `blocking`, `dump`, `hash`, `rows`; `full` is all of them.
- **Semver.** Model structs are plain (public fields, `Default`), not
  `#[non_exhaustive]`: they mirror the frozen v3.0 schema, and adding a field
  is a minor (0.x) release. Enums that can grow are `#[non_exhaustive]`
  (README.md, *Versioning*).
- **Documented and exemplified.** `#![warn(missing_docs)]`; the main
  functions carry doctests; the README's Rust blocks are doctests
  (`lib.rs`, `ReadmeDoctests`); `examples/` has one program per module.
- **Self-contained tests.** Every test reads only files in this directory
  (`reference/`, `tests/`); nothing needs the yorc-duckdb extension or the
  network (the client tests use a local mock server).

## Status

| module | state |
|---|---|
| `id` | done: `OrcidId` (`parse`, `FromStr`, `Display`, `uri`), `normalize`, `is_valid`, `check_char`. No serde yet. |
| `hash` (feature) | done: `sha256_hex`. |
| `model` | done: every data-bearing element/attribute of the record XSDs (`common`, `activities`, `person`, `record`), `Document`/`Root` for every root element of the samples (plus summaries). Affiliations are one `Affiliation` struct with an `AffiliationKind`. `ItemMeta::display_index_text` keeps a non-integer `@display-index` (ORCID emits `-1`) so it survives a write. |
| `xml` | done. Read: `from_str`, `from_slice` (quick-xml 0.42 events into a small element tree, then onto the model). Write: `to_string(&Root)` for every root kind, exact and deterministic (ORCID prefixes, all namespaces declared on the root in `xml::Ns` order, XSD sequence order, two-space indent, LF, dates `{:04}`/`{:02}`), in one pass from the model into one `String` sized from the model's shape: start tags stay open until the first child (`<x/>` otherwise), the namespaces in use are a bit set whose declarations are inserted after the root's name at the end, escaping and the XML 1.0 check are one 8-bytes-at-a-time scan that copies plain runs, and the element path of an `Error::Unwritable` is built only on error. `tests/reference_writer` keeps the earlier tree-building writer; `writer_matches_reference_*` (in `write_roundtrip.rs` and `schema_coverage.rs`) require the same bytes or the same error on every sample, the real record, the same documents with escapes, multi-byte text and unwritable characters injected, and every root kind. Content the XSDs forbid (a funding `contributor-sequence`) or XML 1.0 cannot carry is `Error::Unwritable`, never a panic. |
| `rows` (feature) | done for the 11 persisted record sections, the `search_results` and `errors` API-message sections and `research_resource_hosts` (14 sections, 300 columns); opt-in (`rows::Extra`, 2026-10-18): `timeline` and `group_ids`, and `rows::changes` (yorc-duckdb's `orcid_changes`); `summary`, `summary_items` and `notifications` are not produced (their documents are not modelled). Items are found where the XSDs put them (the extension matches path suffixes anywhere, so documents outside the schema can differ). `tests/samples.rs` checks the rows against the extension's published expectations, copied into the test. |
| `dump` (feature) | done: streaming `.tar.gz` access, `dump::records` / `records_with` (`DumpRecord`: path, stem, bytes, `orcid()`, `parse()`), a per-member size limit (`dump::Limits`, 64 MiB by default: the tar header is checked before allocating, and the read is capped, so a lying header fails too) + `dump::reservoir`. |
| `json` (feature) | done: `from_str(kind, json)` / `to_string(&Root)` / `to_value` for every root kind, over `serde_json::Value`. ORCID's shapes: kebab-case keys, `{"value": ...}` wrappers, timestamps as epoch milliseconds (converted to and from `xs:dateTime` text), `summaries` wrappers in affiliation groups, `resource-item` arrays, `{"bulk": [...]}`, Java enum names for `creation-method`. Lenient reads (bare or wrapped text, nulls, unknown keys); deterministic writes (sorted keys, no nulls). `tests/json_roundtrip.rs`: all 27 record JSON samples round-trip and match the original (normalised); JSON → XML validates with xmllint and reads back to the same model; all 75 XML documents → JSON → model are equal. `peer-review-group-id.json` is a group-id-3.0 document and is skipped. |
| `blocking` (feature) | done: Public API reads, search and the optional `/read-public` token (see *Modules*). `src/blocking.rs` unit tests: the closed host set and forged URLs (`Access::permits`, as pure functions on `Public`: no Member API or Sandbox host is permitted; `Custom` refuses ORCID's hosts; the send path refusing a forged URL), the token checks (realm, `/read-public` scope only), the token answer, the 45 s clamp and 3-attempt cap of `Public`. `tests/blocking_client.rs` runs it against a local mock server with `Custom` clients: paths, methods, `Accept`, `Authorization`, `error-3.0` parsing, no retry of POST or after a 429 (the limiter waits for `Retry-After`), other tokens refused with nothing sent, plus read, search, `record_raw`, the `/read-public` token and a shared limiter. |
| async client | removed (it was a stub). Future work: see CHANGELOG.md. |
| round-trip test | done: `tests/write_roundtrip.rs`. All 21 write samples, all 53 read samples and the real fixture record read → write → read to an identical model, byte-identical rewrites, and canonically equal XML (namespace URI + local name, sorted attributes, trimmed text, no comments/`xsi:*`, data-less elements dropped). One documented exception: `peer-reviews-3.0.xml` uses the pre-3.0 `peer-review:summary` layout, which the XSD rejects (asserted with xmllint). With xmllint installed, all 75 written documents validate against the vendored XSDs, including 15 whose originals do not (untrimmed `xs:dateTime`, empty `journal-title`). |
| schema-walking test | done: `tests/schema_coverage.rs` parses every record XSD and `common-3.0.xsd` with quick-xml (195 element names, 10 attribute names), writes a fully populated model of every root kind, and requires every declared name to be written, or listed with a reason in `tests/xsd_uncovered.txt` (11 global `common` elements that no record schema uses), and nothing undeclared to be written. |

## History

1. `id`, `hash`, `dump` and `dump::reservoir` were written first, for a
   command-line loader.
2. Then `model` + `xml` (read and write) for every section, validated write
   documents (`validate::Payload`), `json`, `rows`, the blocking client.
   (Validation and payloads were removed again on 2026-10-18; see 7.)
3. 2026-10-17: split out as `yorc-rs`. The application's consent rule
   (a `Consent` type, refusing every other iD) stayed in the application;
   the client here became a plain Public API reader with OAuth helpers; the
   async `client` stub was removed.
4. 2026-10-18 (still 0.1.0, unreleased), from the three-way review
   (the cross-implementation review, #4 and #7): `dump::Limits` (the per-member
   size limit, `records_with`, `read_entry_with`), and the `search_results`
   and `errors` sections in `rows`.
5. 2026-10-18 (still 0.1.0, unreleased), CROSSCHECK
   #2, #9 and #10: payloads leave out what ORCID sets (`Payload::to_root`);
   `research_resource_hosts` keeps every host; the assertion origin,
   normalized ids, normalization error code, contributor host and the
   record's `orcid-identifier` URI and host are read in every section that
   has them (columns appended; table in CHANGELOG.md). yorc-zig followed
   the same day (CROSSCHECK.md, "Pending: Zig", now closed).
6. 2026-10-18 (still 0.1.0, unreleased): `blocking`
   gained the `Access` modes (replacing `Environment`), sandbox-only Member
   API writes, the host allowlist, token realms and the 45 s process-wide
   pacing of ORCID's hosts.
7. 2026-10-18: Member API write support removed, held back pending ORCID's
   review: `validate`, `xml::payload_to_string` / `json::payload_to_string`
   (`Payload::to_root`), `Error::Invalid`, `Violation` and `Rule`, the
   client's create, update and delete methods, the update scopes,
   `Capability::ReadWrite`, `Purpose::Write`, `Operation::Write`,
   `blocking::Format`, `ItemKind` and the `write_payload` and `post_work`
   examples. The code is kept on the `member-api-writes` branch.
8. 2026-10-19 (still 0.1.0, unreleased): the public crate does not provide
   access to the ORCID Sandbox or the ORCID Member API. `blocking` reads
   only ORCID's Public API: removed `Access::Member`, `Access::Sandbox`,
   `Capability`, `Operation`, `Access::allows`, `Access::check_scopes`,
   `Access::read_base`, `Access::member_api_base` / `public_api_base` (now
   `api_base`), `Realm::Sandbox`, `MEMBER_API`, the `SANDBOX_*` bases, the
   `/authenticate`, `openid` and `/read-limited` scopes, member-scoped
   reads, the 3-legged sign-in (`authorize_url`,
   `authorize_url_with_scope`, `parse_callback`, `Client::exchange_code`,
   `ClientError::StateMismatch`, `Token::orcid` / `Token::name`) and the
   public `token_from_response`. Kept: `Public` reads and search, the
   optional `/read-public` token, the 45 s limiter, retries, the closed
   host set. Applications that need more keep it themselves.
9. TODO: run the `rows` conformance test against the extension itself
   (today it checks the extension's published expectations).
