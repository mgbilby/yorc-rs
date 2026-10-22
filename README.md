# yorc-rs: Cultivate your research identity and networks

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/yorc.svg)](https://crates.io/crates/yorc)
[![docs.rs](https://img.shields.io/docsrs/yorc)](https://docs.rs/yorc)
[![CI](https://github.com/mgbilby/yorc-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/mgbilby/yorc-rs/actions/workflows/ci.yml)
[![Rust 1.87+](https://img.shields.io/badge/rust-1.87%2B-orange.svg)](#minimum-supported-rust-version)
[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.23570275.svg)](https://doi.org/10.5281/zenodo.23570275)

The [ORCID](https://orcid.org/) message schema v3.0 for Rust: a typed model of
the whole record schema, ORCID XML and JSON read and write, iD checks, the
[ORCID Public Data File](https://info.orcid.org/documentation/integration-guide/orcid-public-data-file/)
streamed from its `.tar.gz` archives, and a blocking ORCID Public API client
for occasional single-record retrieval (one request every 45 s to ORCID; for
anything more, use the Public Data File and the `dump` reader).

- Schema-derived: every data-bearing element and attribute of the v3.0 record
  XSDs has a field, and a test walks the XSDs to keep it that way.
- Every ORCID sample document reads, writes and reads back to the same model,
  in XML and JSON, and the written XML validates against ORCID's XSDs.
- Lenient reads (unknown elements are skipped, unknown vocabulary values
  are kept as `Other(..)`), exact and deterministic writes, no panics on data.
- No DuckDB, no async runtime, no OpenSSL. Only the features you turn on pull
  in dependencies.
- General purpose: no policy. What you read, and why, is up to you.
- Byte-identical output with [yorc-zig](https://github.com/mgbilby/yorc-zig)
  and with [yorc-duckdb](https://github.com/mgbilby/yorc-duckdb)'s writers
  in a cross-check of ORCID's samples. Against ORCID's Java library it takes
  0.67 of the time on average (XML writes 0.11, XML reads 0.43); see
  [BENCHMARKS.md](BENCHMARKS.md), reproducible with the public harness
  [yorc-bench](https://github.com/mgbilby/yorc-bench).

## Scope

yorc-rs is an independent open-source project. It is not affiliated with,
endorsed by or sponsored by ORCID, Inc.; it reads and converts data in
ORCID's published formats. It does not provide access to the ORCID Sandbox
or the ORCID Member API: its optional client (feature `blocking`) reads only
ORCID's Public API (`pub.orcid.org`), at most one request every 45 s. Bulk
data should come from the
[ORCID Public Data File](https://info.orcid.org/documentation/integration-guide/orcid-public-data-file/),
read with the `dump` reader.

The crate is `yorc` (`use yorc::...`); the repository is `yorc-rs`.

Walkthrough: [WALKTHROUGH.md](WALKTHROUGH.md), the reading modules in ten
runnable cells with their expected output
(`cargo run --example walkthrough --all-features`).

## Features

| cargo feature | default | what it adds | dependencies |
|---|---|---|---|
| `id` | yes | `OrcidId`: parse and normalise iDs (URL forms, missing hyphens, lowercase `x`), ISO 7064 MOD 11-2 | none |
| `model` | yes | typed structs for the v3.0 record schema (`Record`, `Work`, `Funding`, `Affiliation`, `PeerReview`, `ResearchResource`, `Person`, ...) and for peer-review group ids (`GroupIdRecord`, `GroupIdRecords`, group-id-3.0) | none |
| `xml` | yes | ORCID XML → model (`xml::from_str`, `from_slice`) and model → XML (`xml::to_string`, `document_to_string`) | `quick-xml` |
| `json` | | ORCID's JSON form, read and write (`json::from_str`, `to_string`) | `serde_json` |
| `dump` | | Public Data File `.tar.gz` streaming without unpacking (`dump::open`, `dump::records`, with a per-member size limit: `dump::Limits`, 64 MiB by default), and a seeded, reproducible reservoir sampler (`dump::reservoir`) | `flate2`, `tar` |
| `hash` | | `hash::sha256_hex`, lowercase hex SHA-256 of a document | `sha2` |
| `rows` | | the model flattened into the section tables of the [yorc-duckdb](https://github.com/mgbilby/yorc-duckdb) extension, same names, columns and types (`rows::Tables`): the 11 record sections, `search_results` and `errors` for the API messages (search results, error responses, locked and deprecated records, bulk errors), and `research_resource_hosts` (every host of a research resource); opt-in, outside those 14: `timeline` (the dated events of a record) and `group_ids` (`rows::Extra`), and `rows::changes`, the field-level differences between two snapshots (yorc-duckdb's `orcid_changes`) | (`id`, `xml`, `hash`) |
| `blocking` | | an ORCID Public API client for occasional single-record retrieval (`pub.orcid.org` only: no Member API, no Sandbox): reads of a record by iD and search, anonymous or with an optional 2-legged `/read-public` token. Reads only. One request every 45 s to ORCID's hosts, process-wide, one at a time | `ureq` (rustls), `serde_json` |
| `full` | | all of the above | |

```toml
[dependencies]
yorc = "0.1"                                              # id, model, xml
yorc = { version = "0.1", features = ["json", "blocking"] }
```

## Quick start

The examples use `?` inside `fn main() -> Result<(), Box<dyn std::error::Error>>`;
each is compiled as a doctest. Runnable versions are in
[`examples/`](examples/) (see [Examples](#examples)).

Read an ORCID XML record (a saved `/record` response, a Public Data File
member, or any other v3.0 document):

```rust,no_run
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# #[cfg(feature = "xml")] {
let xml = std::fs::read_to_string("0000-0002-1825-0097.xml")?;
let doc = yorc::xml::from_str(&xml)?;
println!("iD: {:?}", doc.orcid());
if let Some(name) = doc.record().and_then(|r| r.person.as_ref()).and_then(|p| p.name.as_ref()) {
    println!("{:?} {:?}", name.given_names, name.family_name);
}
if let Some(summary) = doc.activities_summary() {
    for w in summary.work_summaries() {
        println!("{:?}", w.title.as_ref().and_then(|t| t.title.as_deref()));
    }
}
# }
# Ok(()) }
```

Read ORCID JSON (feature `json`); the kind is the XML root's local name
(`"record"`, `"work"`, `"employments"`, ...; see `json::KINDS`):

```rust,no_run
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# #[cfg(feature = "json")] {
let json = std::fs::read_to_string("0000-0002-1825-0097.json")?;
let doc = yorc::json::from_str("record", &json)?;
# let _ = doc;
# }
# Ok(()) }
```

Write any document back, as XML or JSON:

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# #[cfg(feature = "xml")] {
# let doc = yorc::xml::from_str(r#"<record:record path="/0000-0002-1825-0097" xmlns:record="http://www.orcid.org/ns/record"/>"#)?;
let xml = yorc::xml::to_string(&doc.root)?;   // ORCID's prefixes and namespaces, XSD order
# #[cfg(feature = "json")]
let json = yorc::json::to_string(&doc.root)?; // feature `json`; sorted keys, no nulls
# }
# Ok(()) }
```

Stream a Public Data File archive (feature `dump`), member by member, without
unpacking it:

```rust,no_run
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# #[cfg(all(feature = "dump", feature = "xml"))] {
use std::path::Path;
use yorc::dump;

let mut archive = dump::open(Path::new("ORCID_2025_10_summaries_1.tar.gz"))?;
for member in dump::records(&mut archive)? {
    let member = member?;           // path, stem (the iD) and the exact bytes
    let doc = member.parse()?;      // feature `xml`
    println!("{}: {:?}", member.stem, doc.orcid());
}
# }
# Ok(()) }
```

`dump::records` skips members that are not `.xml` files; to skip reading
some members as well (sampling, say), walk `archive.entries()` with
`dump::xml_entry_stem` and `dump::read_entry`.

A member is held in memory whole, so both refuse a member over 64 MiB (as
yorc-duckdb's `max_member_bytes`) with an `io::Error`, checked against the
tar header before anything is allocated and again while reading; the walk
goes on with the next member. A corrupt or crafted archive (a gzip bomb)
cannot make them allocate without bound. Set another limit with
`dump::records_with` and `dump::read_entry_with`:

```rust,no_run
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# #[cfg(feature = "dump")] {
use std::path::Path;
use yorc::dump::{self, Limits};

let mut archive = dump::open(Path::new("ORCID_2025_10_summaries_1.tar.gz"))?;
let limits = Limits::default().with_max_member_bytes(16 << 20);
for member in dump::records_with(&mut archive, limits)? {
    match member {
        Ok(m) => println!("{}: {} bytes", m.stem, m.bytes.len()),
        Err(e) => eprintln!("{e}"), // member '...' is ... bytes, over max_member_bytes (16777216)
    }
}
# }
# Ok(()) }
```

Flatten any document into yorc-duckdb's tables (feature `rows`). API
messages have rows too: a locked or deprecated record, served as an error
document, still names its iD:

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# #[cfg(feature = "rows")] {
use yorc::rows::{Section, Tables};

let xml = br#"<error:error xmlns:error="http://www.orcid.org/ns/error">
  <error:response-code>301</error:response-code>
  <error:developer-message>301 Moved Permanently: This account is deprecated. Please refer to account: https://orcid.org/0009-0008-8963-1314. ORCID https://orcid.org/0000-0001-5433-3001</error:developer-message>
  <error:error-code>9007</error:error-code>
</error:error>"#;
let t = Tables::from_xml(xml)?;
let cell = |name: &str| t.cell(Section::Errors, 0, name).and_then(|c| c.as_str());
assert_eq!(cell("orcid"), Some("0000-0001-5433-3001"));
assert_eq!(cell("primary_orcid"), Some("0009-0008-8963-1314"));
# }
# Ok(()) }
```

The tables follow yorc-duckdb's layout column for column: 14 sections, 300
columns. `research_resource_hosts` has one row per host organization of a
proposal or resource item (ORCID allows up to 10 each); the `host_*` columns
of `research_resources` and `research_resource_items` still hold the first
host (the first non-empty value per column), for compatibility. Columns
added later are appended after the existing ones (CHANGELOG.md lists them),
so reading cells by position keeps working; reading them by name
(`Tables::cell`, `Section::column_index`) is safest.

Two more of yorc-duckdb's sections are opt-in, so that the 14 above stay as
they are: `timeline` (one row per dated event: each item's creation and, if
later, its last modification) and `group_ids` (peer-review group ids), both
`rows::Extra`. `rows::changes(old, new)` lists what changed between two
snapshots of a record, field by field, as yorc-duckdb's `orcid_changes`:

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# #[cfg(feature = "rows")] {
use yorc::rows::{changes, timeline, ChangeKind};

let old = yorc::xml::from_str(r#"<keyword:keywords xmlns:keyword="http://www.orcid.org/ns/keyword"
    xmlns:common="http://www.orcid.org/ns/common">
  <keyword:keyword put-code="1"><common:created-date>2020-01-01T00:00:00.000Z</common:created-date>
    <keyword:content>vents</keyword:content></keyword:keyword>
</keyword:keywords>"#)?;
let new = yorc::xml::from_str(r#"<keyword:keywords xmlns:keyword="http://www.orcid.org/ns/keyword">
  <keyword:keyword put-code="1"><keyword:content>vents</keyword:content></keyword:keyword>
  <keyword:keyword put-code="2"><keyword:content>basalt</keyword:content></keyword:keyword>
</keyword:keywords>"#)?;
assert_eq!(timeline(&old, None).len(), 1); // the keyword's creation
let c = changes(Some(&old), Some(&new));
assert_eq!(c.len(), 2); // keyword 1 lost its created date, keyword 2 is new
assert_eq!((c[0].change, c[0].field), (ChangeKind::Modified, Some("created")));
assert_eq!(c[1].change, ChangeKind::Added);
assert_eq!(c[1].label.as_deref(), Some("basalt"));
# }
# Ok(()) }
```

Read from ORCID's Public API (feature `blocking`). The client is for
**occasional single-record retrieval**: every request to ORCID's hosts
(`orcid.org` and its subdomains), the token call and retries included, goes
through one process-wide limiter at **one request every 45 s**, strictly one
at a time. A faster limiter is clamped to that cap. For bulk data use the
[Public Data File](https://info.orcid.org/documentation/integration-guide/orcid-public-data-file/)
and the `dump` reader, which works on local files and is not throttled (nor
are Public Data File downloads from Figshare or ORCID's S3/CDN mirrors). There
are no crawling or parallel-fetching helpers. Requests never follow
redirects; GET is retried (at most 3 attempts, honouring `Retry-After`),
POST (the token call) never, and a 429 stops the client until its
`Retry-After` has passed.

The client reads only `pub.orcid.org` (`Access::Public`; `Access::Custom`
is for mock servers in tests and refuses ORCID's hosts). Every request URL
is checked against those hosts before it is sent. Reads need no token; the
optional token is a 2-legged `/read-public` token from
`orcid.org/oauth/token`, and any other token (another realm, another scope)
is refused before anything is sent. There is no sign-in (3-legged) OAuth,
no Member API, no Sandbox and no writes.

```rust,no_run
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# #[cfg(feature = "blocking")] {
use yorc::blocking::{Access, Client, Config};

let client = Client::new(Config::new(Access::Public));
let record = client.record("0000-0002-1825-0097", None)?;

// with an optional 2-legged /read-public token for your registered ORCID client
let token = client.read_public_token("APP-XXXXXXXXXXXXXXXX", "client-secret")?;
let works = client.works("https://orcid.org/0000-0002-1825-0097", Some(&token))?;
let (status, bytes) = client.record_raw("0000-0002-1825-0097", Some(&token))?; // the exact bytes

// search finds a record; it is paced like everything else and is not a bulk-export path
let page = client.search("family-name:Carberry AND given-names:Josiah", 0, 10, None)?;
println!("{:?} found", page.num_found);
# let _ = (record, works, status, bytes);
# }
# Ok(()) }
```

More: the crate documentation on [docs.rs](https://docs.rs/yorc), and
[ARCHITECTURE.md](ARCHITECTURE.md) for the design rules and the state of each
module.

## Examples

Small programs in [`examples/`](examples/), one per module:

| example | features | what it does |
|---|---|---|
| `orcid_id` | `id` | normalise and check iDs given on the command line |
| `read_record` | `xml` (default) | read a record file (`FILE.xml`) and summarise it |
| `json_convert` | `xml`, `json` | convert an ORCID XML file to JSON, or JSON back to XML |
| `section_rows` | `rows` | flatten a record file into the yorc-duckdb section tables |
| `dump_count` | `dump` | stream a Public Data File archive and count its records |
| `dump_sample` | `dump` | seeded, reproducible reservoir sample of iDs from an archive |
| `fetch_record` | `blocking` | read one record by iD from ORCID's Public API |
| `search` | `blocking` | one page of Public API search results |
| `walkthrough` | `full` | the ten cells of [WALKTHROUGH.md](WALKTHROUGH.md), all or one (`-- 5`); cell 9 reads ORCID only with `-- 9 --network` |

```sh
cargo run --example read_record -- tests/data/0000-0002-2771-9344.xml
cargo run --example dump_count --features dump -- ORCID_2025_10_summaries_1.tar.gz
cargo run --example fetch_record --features blocking -- 0000-0002-1825-0097
cargo run --example walkthrough --all-features -- 5
```

## Versioning

0.x releases follow Cargo's semver rules: a minor release (0.1 → 0.2) may
break, a patch release (0.1.0 → 0.1.1) does not.

- The model structs mirror ORCID's frozen v3.0 schema and are plain structs
  with public fields, so you can build them with `..Default::default()`.
  Adding a field to one (for example, if ORCID adds an element to v3.0) is
  therefore a minor (0.x) release, never a patch.
- Enums that can grow are `#[non_exhaustive]`: the vocabularies
  (`WorkType`, `Visibility`, ...; unknown values are `Other(..)`), `Root`,
  `BulkItem`, `Error`, `rows::Section`, `rows::Extra`, `rows::ChangeKind`,
  `rows::ColType`, `rows::Cell`, `blocking::ClientError` and
  `blocking::Access`. Match them with a `_ =>` arm. Enums fixed by the
  v3.0 schema or by an algorithm (`AffiliationKind`, `xml::Ns`,
  `dump::Slot`) are exhaustive.
- `rows::Section::ALL` is a slice, so a new section (as `search_results`,
  `errors` and `research_resource_hosts`) is not a type change; code that
  walks it sees the new tables. New columns are appended to a section,
  never inserted. `dump::DumpRecord` and `dump::Limits` are `#[non_exhaustive]`
  structs: build `Limits` from `Limits::default()`.

## Minimum supported Rust version

Rust **1.87**, edition 2024 (`rust-version` and `edition` in `Cargo.toml`): quick-xml 0.42 needs 1.86,
and the crate uses `u*::is_multiple_of`, stable since 1.87. Raising it is a
minor-version change.

## Compared with the `orcid` crate

[`orcid`](https://crates.io/crates/orcid) 0.2.3 reads the Public API's JSON
into a `serde_json::Value` with accessor methods, has blocking and async
clients and search (iDs only), and is read-only. `yorc-rs` has a typed
model of the whole v3.0 record schema (including history, distinctions,
invited positions, services, research resources, contributors and full
items, which `orcid` 0.2.3 does not read), reads and writes both XML and JSON,
streams the Public Data File, checks iDs strictly,
and has a rate-limited blocking Public API client with search and an
optional `/read-public` token; it has no async client yet. The two share no code. Details, field
counts and the shared-code measurement: [COMPARISON.md](COMPARISON.md).

## Development

```sh
cargo test --all-features          # every test and doctest; no network, no DuckDB
cargo test                         # default features
cargo test --no-default-features
```

The Rust blocks of this README are compiled as doctests.

The tests read ORCID's schemas and samples from `reference/orcid-model/` and
a real public record from `tests/data/`; neither is in the published package.
With `xmllint` installed, the round-trip tests also validate every written
document against the XSDs.

## Contributing

Bug reports and pull requests are welcome. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the layout, the checks to run and the
rules the code keeps; [CHANGELOG.md](CHANGELOG.md) lists the changes of each
release.

## Security

Documents and archives are treated as untrusted input. See
[SECURITY.md](SECURITY.md) for what that means and how to report a
vulnerability.

## License

MIT, see [LICENSE](LICENSE). The ORCID schema files and samples under
`reference/orcid-model/` (repository only) are © ORCID, Inc. under ORCID's
MIT-style licence, and the records `tests/data/0000-0003-0100-6634.xml` and
`tests/data/0000-0002-2771-9344.xml` are public ORCID record data under CC0 1.0;
see [NOTICE](NOTICE). This crate is not affiliated with, endorsed by or
sponsored by ORCID, Inc.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in yorc-rs by you shall be licensed as MIT, without
any additional terms or conditions.
