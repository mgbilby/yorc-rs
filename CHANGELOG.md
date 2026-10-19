# Changelog

All notable changes to yorc-rs. Versions follow
[Semantic Versioning](https://semver.org/); the version is `package.version`
in `Cargo.toml`, and the release tag is `v<version>`.

## [Unreleased]

Additive (2026-10-18): nothing that existed changes, the 14 sections and their 300
columns included.

### Added
- Peer-review group ids (group-id-3.0): `model::GroupIdRecord`,
  `GroupIdRecords` and `GroupType`, `Root::GroupIdRecord` and
  `Root::GroupIdRecords`, as ORCID's `GroupIdRecord` / `GroupIdRecords`.
  XML read (the XSD's inline source fields read into `source`) and write
  (`common:source`, ORCID's `propOrder`), JSON read and write
  (`json::KINDS` gains `group-id-record` and `group-id-records`; the
  record's dates are XML-only, as in ORCID's JSON). The group-id documents
  are written outside the record emitter, so `xml::Ns` keeps its 28
  namespaces. `reference/orcid-model/group-id-3.0` is vendored for the
  tests.
- `rows::Extra` (`Timeline`, `GroupIds`), `rows::timeline`,
  `rows::group_ids`, `Tables::extra_from_xml`: yorc-duckdb's
  `orcid_timeline` and `orcid_group_ids` tables, same columns and order.
  Opt-in: `Section::ALL`, `Tables` and `section_rows` do not build them.
- `rows::changes(old, new)` with `Change`, `ChangeKind`, `CHANGE_COLUMNS`
  and `DIFF_SECTIONS`: yorc-duckdb's `orcid_changes`, the field-level
  differences between two snapshots, with its matching, order and labels.
  Values are text as ORCID writes them; a document's raw text that the
  model does not keep (`01` vs `1`, a `display-index` of `-1`) can differ
  from yorc-duckdb's.
- Tests: `tests/extra_sections.rs` (yorc-duckdb's pinned values from
  orcid_history_of_record.test and orcid_api_messages.test, and per-sample
  timeline counts); `tests/data/changes/` (yorc-duckdb's two snapshots) and
  `tests/data/group-id-records.xml`.

### Fixed
- XML: elements nested deeper than 256 levels are an `Error::Xml`. Before,
  a deeply nested document (a few bytes per level) overflowed the stack
  when its element tree was dropped, aborting the process.
- XML: an `&` in an attribute value looks at most 12 bytes ahead for its
  `;` (a reference name is at most 11 bytes), so a value of many `&` reads
  in linear time instead of quadratic. The decoded values do not change.
- JSON: `json::datetime_to_millis` returns `None` for a zone with a
  non-ASCII character where it panicked on a slice that was not at a char
  boundary (reachable from `json::to_string` of a document read from XML).

## [0.1.0] - 2026-10-17

First release. Rust 1.87 or later, edition 2024.

Developed as orcid-model (now yorc; crate `yorc`, repository `yorc-rs`),
renamed on 2026-10-18 to stay clear of ORCID's trademark.

Scope: yorc-rs is an independent open-source project, not affiliated with,
endorsed by or sponsored by ORCID, Inc. It does not provide access to the
ORCID Sandbox or the ORCID Member API: this release reads ORCID data and
converts it between XML, JSON and rows, and its optional client reads only
ORCID's Public API (`pub.orcid.org`), sending nothing to ORCID but Public
API reads (and the optional `/read-public` token call). Bulk data comes
from the Public Data File (`dump`).

### Stability
- The model structs mirror ORCID's frozen v3.0 schema and keep public
  fields, so they can be built with `..Default::default()`; they are not
  `#[non_exhaustive]`. Adding a field to one is a minor (0.x) release.
- Enums that may grow are `#[non_exhaustive]` (match them with `_ =>`): the
  eleven vocabulary enums (`WorkType`, `Visibility`, ...), `Root`,
  `BulkItem`, `Error`, `rows::Section`,
  `rows::ColType`, `rows::Cell`, `blocking::ClientError`,
  `blocking::Access`, and the structs `dump::DumpRecord` and
  `dump::Limits`.
  `AffiliationKind`, `xml::Ns` and `dump::Slot` are fixed
  by the schema or the algorithm and stay exhaustive. `rows::Section::ALL` is
  a slice, so a new section does not change its type.

### Model (`model`, default)
- Typed structs for every data-bearing element and attribute of the ORCID
  v3.0 record XSDs: works, fundings, the seven affiliation kinds (one
  `Affiliation` with an `AffiliationKind`), peer reviews, research resources
  and their items, contributors, person and its items (names, biography,
  other names, researcher URLs, emails, addresses, keywords, external
  identifiers), history, preferences, record, bulk, error, search and
  expanded search; summary and full forms are separate types.
- Closed vocabularies are enums with an `Other(String)` fallback, so new
  ORCID values never fail a parse.
- `Document` / `Root` for every root element ORCID's samples use, with
  `Document::orcid()` (the record's iD, as yorc-duckdb determines it).

### iDs (`id`, default)
- `OrcidId` (`parse`, `FromStr`, `Display`, `uri`), `normalize`,
  `is_valid`, `check_char`: ISO 7064 MOD 11-2, URL forms, missing hyphens,
  lowercase `x`; the same rules as yorc-duckdb's `orcid_normalize`.

### XML (`xml`, default)
- Read: `xml::from_str`, `from_slice` (quick-xml 0.42). Lenient: unknown
  elements and attributes are skipped, namespaces are matched by local name,
  entities and character references are decoded.
- Write: `xml::to_string` for every root kind, exact and deterministic
  (ORCID's prefixes, all namespaces on the root, XSD sequence order);
  content the XSDs forbid or XML 1.0 cannot carry is `Error::Unwritable`,
  never a panic.
- The writer works in one pass, straight from the model into one pre-sized
  `String` (no intermediate tree, no copies of model strings, no `format!`
  per element); escaping and the XML 1.0 character check are one scan that
  copies runs of plain text, eight bytes at a time. The output is
  byte-identical to the earlier tree writer, which `tests/reference_writer`
  keeps as the reference for the samples, the real record and every root
  kind.
- The reader keeps only the attributes it maps and no indentation text.

### JSON (`json`)
- ORCID's JSON form over the same model: `json::from_str(kind, ..)`,
  `to_string`, `to_value`; `{"value": ...}` wrappers,
  kebab-case keys, epoch-millisecond timestamps converted to and from
  `xs:dateTime`, deterministic writes (sorted keys, no nulls).
- `history/@visibility` is XML-only: the JSON writer leaves it out, as
  ORCID's own JSON does (ORCID's Java model refuses the unknown member, so
  `record-3.0` and `history-3.0` written as JSON did not read in Java;
  cross-implementation review #16). The JSON reader still accepts it. An XML ->
  JSON -> XML round trip loses it.

### Public Data File (`dump`)
- `dump::open` streams a `.tar.gz` archive member by member without
  unpacking it; `xml_entry_stem`, `read_entry`, `is_dump_path`.
- `dump::records` iterates over the `.xml` members as `DumpRecord`s (path,
  stem, exact bytes), with `DumpRecord::orcid` (feature `id`) and
  `DumpRecord::parse` (feature `xml`).
- A per-member size limit, as yorc-duckdb's `max_member_bytes`:
  `dump::Limits` (`#[non_exhaustive]`; `max_member_bytes`, default
  `Limits::DEFAULT_MAX_MEMBER_BYTES` = 64 MiB, `with_max_member_bytes`),
  `dump::records_with(archive, limits)` and
  `dump::read_entry_with(reader, size_hint, limits)`. `records` and
  `read_entry` apply the default. A member over the limit is an
  `io::Error` (`InvalidData`: `member '<path>' is <n> bytes, over
  max_member_bytes (<limit>)`), raised from the tar header before anything
  is allocated; the read itself is capped too, so a header that lies is
  caught as well. `records` then goes on with the next member. A corrupt
  or crafted archive (a gzip bomb) cannot allocate without bound.
- `dump::reservoir`: SplitMix64 and Algorithm R, specified exactly so a
  sample can be reproduced from the same archive, seed and size.

### Section rows (`rows`, `hash`)
- `rows::Tables`: the model flattened into the 11 persisted section tables
  of the yorc-duckdb extension (works, fundings, affiliations,
  peer_reviews, research_resources, research_resource_items, contributors,
  external_ids, person, person_items, history), with
  `hash::sha256_hex` as `source_hash`.
- The API-message sections `search_results` (search and expanded-search
  results: `num_found`, `ordinal`, iD, names, joined other names, emails
  and institutions) and `errors` (standalone `error` documents and the
  per-entry errors of a `bulk` response, with `container` and `position`;
  `orcid` falls back to the iD a locked or deprecated record's message
  names, and `primary_orcid` gives a deprecated record's primary account),
  with yorc-duckdb's names, columns, order and types. The `rows` feature
  now also turns on `id`.
- `research_resource_hosts` and the columns of the cross-check decisions
  below: 14 sections, 300 columns.

### Cross-check decisions (2026-10-18; cross-implementation review #9, #10)

- **#9, every research-resource host.** New section
  `rows::Section::ResearchResourceHosts` (`research_resource_hosts`, last in
  `Section::ALL`), one row per host organization of a proposal or a resource
  item: research-resource-3.0.xsd allows 1 to 10 hosts in each, so both go
  in one table, told apart by `container` (`proposal` / `resource-item`) and
  `item_ordinal` (the resource item's ordinal, as
  `research_resource_items.ordinal`; NULL for the proposal). The `host_*`
  columns of `research_resources` and `research_resource_items` keep the
  first host (first non-empty value per column), for compatibility.
- **#10, context gaps.** Columns read in some sections only are now read
  wherever the XSDs put the element: the assertion origin, a person external
  identifier's normalized form, the normalization error code, a
  contributor's `host` and the record's `orcid-identifier` URI and host.
  Every new column is appended after the existing ones, as
  `peer_reviews.subject_translated_name_language` was, so positions of the
  earlier columns do not move. Not added: the collections' and groups'
  `last-modified-date` (CROSSCHECK §1: `max(last_modified)` gives it).

Columns added (positions are 1-based; types are yorc-duckdb's):

| section | # | column | type | source |
|---|---:|---|---|---|
| `fundings` | 36 | `assertion_origin_orcid` | VARCHAR | `source/assertion-origin-orcid/path` |
| `fundings` | 37 | `assertion_origin_client_id` | VARCHAR | `source/assertion-origin-client-id/path` |
| `affiliations` | 31 | `assertion_origin_orcid` | VARCHAR | `source/assertion-origin-orcid/path` |
| `affiliations` | 32 | `assertion_origin_client_id` | VARCHAR | `source/assertion-origin-client-id/path` |
| `peer_reviews` | 37 | `assertion_origin_orcid` | VARCHAR | `source/assertion-origin-orcid/path` |
| `peer_reviews` | 38 | `assertion_origin_client_id` | VARCHAR | `source/assertion-origin-client-id/path` |
| `research_resources` | 31 | `assertion_origin_orcid` | VARCHAR | `source/assertion-origin-orcid/path` |
| `research_resources` | 32 | `assertion_origin_client_id` | VARCHAR | `source/assertion-origin-client-id/path` |
| `contributors` | 12 | `contributor_orcid_host` | VARCHAR | `contributor-orcid/host` |
| `external_ids` | 13 | `normalized_error_code` | VARCHAR | `external-id-normalized-error/error-code` |
| `person` | 14 | `orcid_uri` | VARCHAR | the record's `orcid-identifier/uri` (NULL outside a record) |
| `person` | 15 | `orcid_host` | VARCHAR | the record's `orcid-identifier/host` |
| `person_items` | 19 | `assertion_origin_name` | VARCHAR | `source/assertion-origin-name` |
| `person_items` | 20 | `assertion_origin_orcid` | VARCHAR | `source/assertion-origin-orcid/path` |
| `person_items` | 21 | `assertion_origin_client_id` | VARCHAR | `source/assertion-origin-client-id/path` |
| `person_items` | 22 | `normalized` | VARCHAR | `external-id-normalized` (external identifiers) |
| `person_items` | 23 | `normalized_error` | VARCHAR | `external-id-normalized-error/error-message` |
| `person_items` | 24 | `normalized_error_code` | VARCHAR | `external-id-normalized-error/error-code` |
| `history` | 15 | `assertion_origin_name` | VARCHAR | `source/assertion-origin-name` |
| `history` | 16 | `assertion_origin_orcid` | VARCHAR | `source/assertion-origin-orcid/path` |
| `history` | 17 | `assertion_origin_client_id` | VARCHAR | `source/assertion-origin-client-id/path` |
| `research_resource_hosts` (new) | 1 | `orcid` | VARCHAR | the record's iD |
| | 2 | `owner_put_code` | BIGINT | the research resource's `@put-code` |
| | 3 | `ordinal` | INTEGER | the host's 1-based position in `hosts` |
| | 4 | `host_name` | VARCHAR | `name` |
| | 5 | `host_city` | VARCHAR | `address/city` |
| | 6 | `host_region` | VARCHAR | `address/region` |
| | 7 | `host_country` | VARCHAR | `address/country` |
| | 8 | `host_disambiguated_id` | VARCHAR | `disambiguated-organization/disambiguated-organization-identifier` |
| | 9 | `host_disambiguation_source` | VARCHAR | `disambiguated-organization/disambiguation-source` |
| | 10 | `container` | VARCHAR | `proposal` or `resource-item` |
| | 11 | `item_ordinal` | INTEGER | the resource item's 1-based position; NULL for the proposal |
| | 12 | `source_hash` | VARCHAR | SHA-256 of the document |

yorc-duckdb also appends six columns to its `notifications` table (a
section this crate does not produce): `assertion_origin_name`,
`assertion_origin_orcid`, `assertion_origin_client_id`,
`item_external_id_urls`, `item_external_id_relationships` and
`item_external_id_normalized`. Width per section, before → after:
fundings 35 → 37, affiliations 30 → 32, peer_reviews 36 → 38,
research_resources 30 → 32, contributors 11 → 12, external_ids 12 → 13,
person 13 → 15, person_items 18 → 24, history 14 → 17, and
research_resource_hosts 12 (new); 13 sections and 267 columns → 14 and 300.
yorc-zig has the same layout (CROSSCHECK.md, "Pending: Zig", closed).

### ORCID Public API client (`blocking`)
For occasional single-record retrieval; bulk data comes from the Public
Data File (`dump`). No crawling, paging-through-everyone or parallel-fetching
helpers. The Public API only: no Member API, no Sandbox, no sign-in
(3-legged) OAuth, no writes.
- `Access::Public` (`pub.orcid.org`, and `orcid.org/oauth/token` for the
  optional token), fixed at construction, and `Access::Custom` (mock
  servers; it refuses ORCID's hosts).
- `blocking::Client` over ureq 3 (rustls, no async runtime): `record`,
  `record_raw` (the exact bytes, and the 301/409 error documents of
  deprecated and locked records), `person`, `works`, `work`, `works_bulk`
  (up to 100 put-codes) and `section`, each with an optional `&Token`.
- Enforcement before sending: every URL is checked against the access's
  hosts (`Access::permits`), the token call included; tokens carry a
  `Realm`, and an empty token, a token of another realm or one without the
  `/read-public` scope is `ClientError::Usage`.
- `Client::search` and `Client::expanded_search`: one page of a Solr query
  (`start`, `rows` up to `MAX_SEARCH_ROWS` = 1000) as `Search` /
  `ExpandedSearch`; paced like every request, not a bulk-export path.
- Pacing: one request every 45 s (`ORCID_INTERVAL`) to ORCID's hosts,
  through one process-wide limiter (`RateLimiter::orcid`), one request at a
  time; the token call and retries count. A faster limiter is clamped.
  Only `Custom` may go faster. Public Data File downloads and the `dump`
  reader are not throttled.
- Retries: GET on 5xx and transport errors, at most 3
  attempts to ORCID, `Retry-After` honoured; POST (the token call)
  never; a 429 is never retried and pauses the limiter for `Retry-After`. No
  redirects followed; ORCID's `error-3.0` bodies (XML, or JSON) parsed into
  `ClientError::Api`.
- Optional token: `Client::read_public_token` (2-legged
  `client_credentials`, scope `/read-public`), returning a `Token` whose
  `Debug` redacts the secrets; `Token::new` for a token obtained elsewhere.
  Reads need no token.

### Documentation
- Every public item has a doc comment (`#![warn(missing_docs)]`), and the
  main functions and types have examples, compiled as doctests; the README's
  Rust blocks are doctests too. docs.rs builds with all features and shows
  each item's feature (`--cfg docsrs`). `#![forbid(unsafe_code)]`.
- `examples/`: `orcid_id`, `read_record`, `json_convert`,
  `section_rows`, `dump_count`, `dump_sample`, `fetch_record`, `search`
  (both Public API only), `walkthrough`.

### Tests
- Every ORCID v3.0 sample and a real public record read, write and read back
  to the same model in XML and JSON; with xmllint installed, every written
  document validates against the XSDs. A test walks the XSDs and fails on any
  declared name the model does not carry. The section rows, the
  `search_results` and `errors` rows included, are checked against
  yorc-duckdb's own test expectations, copied into `tests/samples.rs`.
  The client runs against a local
  mock server. The schemas, samples and record are vendored in the
  repository (`reference/`, `tests/data/`), not in the package.

### Future work
- An async client (feature-gated, the same reads and limiter as `blocking`).
- Token refresh and revocation.
- serde (`Serialize`/`Deserialize`) derives for the model and `OrcidId`,
  behind a feature (deferred: the derived shape would not be ORCID's JSON,
  which `json` already reads and writes).

[0.1.0]: https://github.com/mgbilby/yorc-rs/releases/tag/v0.1.0
