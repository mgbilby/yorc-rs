# yorc-rs walkthrough

A notebook-like tour of `yorc-rs` in 10 cells. Every cell is a function in
[`examples/walkthrough.rs`](examples/walkthrough.rs) that returns its output as
a `String`; the code block of each cell below is that function's body, and the
expected output is exactly what the cell prints. You need only cargo: no
Jupyter, and no crates beyond yorc-rs's own optional features.

```sh
cargo run --example walkthrough --all-features              # every cell, in order (cell 9 only explains itself)
cargo run --example walkthrough --all-features -- 5         # one cell
cargo run --example walkthrough --all-features -- 9 --network   # cell 9: one request to ORCID
cargo test --test walkthrough --all-features                # checks every cell but 9 against this file
```

Each run prints `## Cell N: Title` before a cell's output. `tests/walkthrough.rs`
runs the same functions and compares their output with the "Expected output"
blocks here, and checks that each code block is its function's body; cell 9
(network) is never run by the tests.

## Inputs

Inputs are files of the repository, named by paths relative to the package
root (the example resolves them from `CARGO_MANIFEST_DIR`, so it runs from any
directory):

| input | path |
|---|---|
| a real public record (CC0, see NOTICE) | `tests/data/0000-0002-2771-9344.xml` |
| ORCID's read sample of a full record (cell 7) | `reference/orcid-model/record_3.0/samples/read_samples/record-3.0.xml` |
| ORCID's read sample of a full work | `reference/orcid-model/record_3.0/samples/read_samples/work-full-3.0.xml` |
| ORCID's write samples of a full work | `reference/orcid-model/record_3.0/samples/write_samples/work-full-3.0.xml` and `.json` |
| a small Public Data File (dump) fixture | `tests/data/dump/orcid_test_summaries.tar.gz` |

`tests/data/` and `reference/` are in the repository but not in the published
package, so run the walkthrough from a checkout. The dump fixture is the same
file, byte for byte, as yorc-duckdb's `test/data/dump/orcid_test_summaries.tar.gz`
and yorc-zig's `testdata/dump/orcid_test_summaries.tgz`. The example is declared with `required-features = ["full"]`, so
`--all-features` (or `--features full`) is needed.

The record is 0000-0002-2771-9344, a public record that ORCID staff use as an
example (its owner, Sofia Maria Garcia Hernandez, is a fictitious person): 12
works and four other names, but no fundings, affiliations or peer reviews. Cell 7 therefore
reads ORCID's sample record instead. Each "Expected output" block has an HTML
comment saying how its values follow from the input files.

## Setup shared by the cells

The imports, input paths and small helpers the cells use (from the top of
`examples/walkthrough.rs`):

```rust
use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use yorc::OrcidId;
use yorc::blocking::{Access, Client, Config};
use yorc::dump::{self, Limits, Reservoir, Slot};
use yorc::model::{Affiliation, AffiliationKind, FuzzyDate, Relationship, Root, WorkSummary};
use yorc::rows::{Cell, Section, Tables};

/// What every cell returns: its output, or the first error.
pub type CellResult = Result<String, Box<dyn Error>>;

/// The public record of 0000-0002-2771-9344 (CC0), a saved `/record` response.
pub const RECORD: &str = "tests/data/0000-0002-2771-9344.xml";
/// ORCID's read sample of a full record (fictitious iD 8888-8888-8888-8880),
/// for the affiliations the public record does not have.
pub const SAMPLE_RECORD: &str =
    "reference/orcid-model/record_3.0/samples/read_samples/record-3.0.xml";
/// ORCID's read sample of a full work.
pub const READ_WORK_FULL: &str =
    "reference/orcid-model/record_3.0/samples/read_samples/work-full-3.0.xml";
/// ORCID's write sample of a full work, as XML.
pub const WRITE_WORK_FULL_XML: &str =
    "reference/orcid-model/record_3.0/samples/write_samples/work-full-3.0.xml";
/// ORCID's write sample of a full work, as JSON.
pub const WRITE_WORK_FULL_JSON: &str =
    "reference/orcid-model/record_3.0/samples/write_samples/work-full-3.0.json";
/// The small Public Data File fixture (the same file, byte for byte, as
/// yorc-duckdb's `test/data/dump/orcid_test_summaries.tar.gz` and yorc-zig's
/// `testdata/dump/orcid_test_summaries.tgz`).
pub const DUMP: &str = "tests/data/dump/orcid_test_summaries.tar.gz";

/// The cells, in order: number and title.
pub const CELLS: &[(u32, &str)] = &[
    (0, "Setup"),
    (1, "Check and normalise ORCID iDs"),
    (2, "Read a saved ORCID record and summarise it"),
    (
        3,
        "List the record's works with year, type and DOI, newest first, top 10",
    ),
    (4, "Read ORCID JSON, and convert a document XML→JSON→XML"),
    (
        5,
        "Flatten the record into section tables and export one as CSV",
    ),
    (6, "Contributors and co-authors of the works"),
    (7, "An affiliation timeline"),
    (8, "Stream the Public Data File dump fixture"),
    (9, "Getting one public record from ORCID"),
];

/// A path relative to the package root.
pub fn path(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

/// The bytes of a file relative to the package root.
pub fn read(rel: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    std::fs::read(path(rel)).map_err(|e| format!("{rel}: {e}").into())
}

/// The text of a file relative to the package root.
pub fn read_text(rel: &str) -> Result<String, Box<dyn Error>> {
    std::fs::read_to_string(path(rel)).map_err(|e| format!("{rel}: {e}").into())
}

/// A fuzzy date as `YYYY-MM-DD`, `YYYY-MM` or `YYYY`; `-` when there is none.
pub fn fuzzy(date: Option<FuzzyDate>) -> String {
    match date.unwrap_or_default() {
        FuzzyDate {
            year: Some(y),
            month: Some(m),
            day: Some(d),
        } => format!("{y:04}-{m:02}-{d:02}"),
        FuzzyDate {
            year: Some(y),
            month: Some(m),
            ..
        } => format!("{y:04}-{m:02}"),
        FuzzyDate { year: Some(y), .. } => format!("{y:04}"),
        _ => "-".to_string(),
    }
}
```

## Cell 0: Setup

Goal: see which features the walkthrough uses and that every input file is where it should be.

```rust
let mut out = String::new();
writeln!(out, "yorc-rs {}", env!("CARGO_PKG_VERSION"))?;
let features = [
    ("id", cfg!(feature = "id")),
    ("model", cfg!(feature = "model")),
    ("xml", cfg!(feature = "xml")),
    ("hash", cfg!(feature = "hash")),
    ("rows", cfg!(feature = "rows")),
    ("dump", cfg!(feature = "dump")),
    ("json", cfg!(feature = "json")),
    ("blocking", cfg!(feature = "blocking")),
];
let on: Vec<&str> = features.iter().filter(|f| f.1).map(|f| f.0).collect();
writeln!(out, "features: {}", on.join(" "))?;
let inputs = [
    ("record", RECORD),
    ("sample record", SAMPLE_RECORD),
    ("read sample", READ_WORK_FULL),
    ("write sample (XML)", WRITE_WORK_FULL_XML),
    ("write sample (JSON)", WRITE_WORK_FULL_JSON),
    ("dump fixture", DUMP),
];
for (what, rel) in inputs {
    let found = if path(rel).is_file() {
        "found"
    } else {
        "MISSING"
    };
    writeln!(out, "{what}: {rel} ({found})")?;
}
Ok(out)
```

The features are the ones `--all-features` turns on (the example requires `full`). The test skips this cell when the dump fixture is missing.

Expected output:

<!-- derived: python3 -I: os.path.isfile on each path; features: the 8 base features of Cargo.toml (full = all of them). -->
```text
yorc-rs 0.1.0
features: id model xml hash rows dump json blocking
record: tests/data/0000-0002-2771-9344.xml (found)
sample record: reference/orcid-model/record_3.0/samples/read_samples/record-3.0.xml (found)
read sample: reference/orcid-model/record_3.0/samples/read_samples/work-full-3.0.xml (found)
write sample (XML): reference/orcid-model/record_3.0/samples/write_samples/work-full-3.0.xml (found)
write sample (JSON): reference/orcid-model/record_3.0/samples/write_samples/work-full-3.0.json (found)
dump fixture: tests/data/dump/orcid_test_summaries.tar.gz (found)
```

## Cell 1: Check and normalise ORCID iDs

Goal: check and normalise ORCID iDs in their valid, invalid, URL and spaced forms (`OrcidId`, feature `id`).

```rust
let mut out = String::new();
let inputs = [
    "0000-0002-1825-0097",                // valid
    "0000-0002-1825-0098",                // wrong check character
    "https://orcid.org/000000021694233x", // URL form, no hyphens, lowercase x
    " 0000 0002 2771 9344 ",              // spaced
];
for input in inputs {
    match OrcidId::parse(input) {
        Ok(id) => writeln!(out, "{input:?} -> {id} ({})", id.uri())?,
        Err(e) => writeln!(out, "{input:?} -> {e}")?,
    }
}
// ISO 7064 MOD 11-2 over the first 15 digits
let check = yorc::id::check_char(b"000000021825009").map(char::from);
writeln!(out, "check character of 0000-0002-1825-009?: {check:?}")?;
Ok(out)
```

`OrcidId::parse` accepts the forms yorc-duckdb's `orcid_normalize` accepts: an `https://orcid.org/`, `http://orcid.org/` or `orcid.org/` prefix, missing hyphens, spaces anywhere, a lowercase `x`. The last character is the ISO 7064 MOD 11-2 check character of the first 15 digits.

Expected output:

<!-- derived: python3 -I: the MOD 11-2 rule of src/id.rs re-implemented over the four strings (0000-0002-1825-009 -> 7, 0000-0002-1694-233 -> X, 0000-0002-2771-934 -> 4; ...0098 fails the check); message text from InvalidOrcidId's Display. -->
```text
"0000-0002-1825-0097" -> 0000-0002-1825-0097 (https://orcid.org/0000-0002-1825-0097)
"0000-0002-1825-0098" -> invalid ORCID iD: "0000-0002-1825-0098"
"https://orcid.org/000000021694233x" -> 0000-0002-1694-233X (https://orcid.org/0000-0002-1694-233X)
" 0000 0002 2771 9344 " -> 0000-0002-2771-9344 (https://orcid.org/0000-0002-2771-9344)
check character of 0000-0002-1825-009?: Some('7')
```

## Cell 2: Read a saved ORCID record and summarise it

Goal: read a saved ORCID record and summarise it: the name, and the counts of works, fundings, affiliations and peer reviews (`xml::from_slice`).

```rust
let mut out = String::new();
let doc = yorc::xml::from_slice(&read(RECORD)?)?;
let record = doc.record().ok_or("not a record")?;
let name = record.person.as_ref().and_then(|p| p.name.as_ref());
let given = name.and_then(|n| n.given_names.as_deref()).unwrap_or("");
let family = name.and_then(|n| n.family_name.as_deref()).unwrap_or("");
let credit = name.and_then(|n| n.credit_name.as_deref()).unwrap_or("-");
writeln!(out, "iD: {}", doc.orcid().unwrap_or("-"))?;
writeln!(out, "name: {given} {family} (credit name: {credit})")?;

let summary = doc.activities_summary().ok_or("no activities summary")?;
let works = summary.work_summaries().count();
let fundings: usize = summary
    .fundings
    .iter()
    .flat_map(|f| &f.groups)
    .map(|g| g.funding_summaries.len())
    .sum();
let mut per_kind = Vec::new();
let mut affiliations = 0;
for kind in AffiliationKind::ALL {
    let n: usize = summary
        .affiliations(kind)
        .map_or(0, |a| a.groups.iter().map(|g| g.summaries.len()).sum());
    if n > 0 {
        per_kind.push(format!("{} {n}", kind.container_element()));
    }
    affiliations += n;
}
let peer_reviews: usize = summary
    .peer_reviews
    .iter()
    .flat_map(|p| &p.groups)
    .flat_map(|g| &g.duplicates)
    .map(|d| d.summaries.len())
    .sum();
writeln!(out, "works: {works}")?;
writeln!(out, "fundings: {fundings}")?;
let kinds = if per_kind.is_empty() {
    String::new()
} else {
    format!(" ({})", per_kind.join(", "))
};
writeln!(out, "affiliations: {affiliations}{kinds}")?;
writeln!(out, "peer reviews: {peer_reviews}")?;
Ok(out)
```

The record is the CC0 record 0000-0002-2771-9344 (see NOTICE). Fundings, affiliations and peer reviews are counted as items, not groups; the affiliation count covers all seven kinds and lists the non-empty ones in parentheses. This record has works only, so the other three counts are 0 (cell 7 turns to ORCID's sample record for affiliations).

Expected output:

<!-- derived: python3 -I ElementTree over tests/data/0000-0002-2771-9344.xml: record/@path; person/name given-names, family-name, credit-name; counts of activities-summary/works/group/work-summary (12); the fundings, the seven affiliation containers and peer-reviews are empty elements (0 each, so no kind is listed). grep -c agrees: 12 work:work-summary, no funding-summary, *-summary of an affiliation kind or peer-review-summary. -->
```text
iD: 0000-0002-2771-9344
name: Sofia Hernandez (credit name: Sofia Maria Garcia Hernandez)
works: 12
fundings: 0
affiliations: 0
peer reviews: 0
```

## Cell 3: List the record's works with year, type and DOI, newest first, top 10

Goal: list the record's works with year, type and DOI, newest first, top 10.

```rust
let mut out = String::new();
let doc = yorc::xml::from_slice(&read(RECORD)?)?;
let summary = doc.activities_summary().ok_or("no activities summary")?;
let mut works: Vec<&WorkSummary> = summary.work_summaries().collect();
// newest first: year, month, day descending (missing parts last), then put-code
works.sort_by_key(|w| {
    let d = w.publication_date.unwrap_or_default();
    (
        Reverse(d.year),
        Reverse(d.month),
        Reverse(d.day),
        w.meta.put_code,
    )
});
// the DOI as yorc-duckdb's `works.doi`: the first doi/self id, ASCII-lowercased
let doi = |w: &WorkSummary| {
    w.external_ids.iter().find_map(|x| {
        let doi_self = x.id_type.as_deref() == Some("doi")
            && x.relationship == Some(Relationship::SelfRef);
        x.value
            .as_deref()
            .filter(|_| doi_self)
            .map(str::to_ascii_lowercase)
    })
};
writeln!(
    out,
    "{} works; the newest 10 (year | type | DOI | title):",
    works.len()
)?;
for &w in works.iter().take(10) {
    let year = w.publication_date.and_then(|d| d.year);
    writeln!(
        out,
        "{} | {} | {} | {}",
        year.map_or("-".to_string(), |y| y.to_string()),
        w.work_type.as_ref().map_or("-", |t| t.as_str()),
        doi(w).unwrap_or_else(|| "-".to_string()),
        w.title
            .as_ref()
            .and_then(|t| t.title.as_deref())
            .unwrap_or("-"),
    )?;
}
Ok(out)
```

Sort order: publication year, month and day descending (a missing part sorts after the present ones), then put-code ascending, so ties are deterministic. The three newest works are one data set in work group 1, deposited three times through DataCite under two DOIs (each recorded as a version of the other). The DOI is yorc-duckdb's `works.doi` rule: the first external id of type `doi` and relationship `self`, ASCII-lowercased; `-` when there is none.

Expected output:

<!-- derived: python3 -I ElementTree: the 12 work-summary elements with publication-date, work:type, the first doi/self external id and the title, sorted by (year, month, day) descending then put-code; the first 10 printed. -->
```text
12 works; the newest 10 (year | type | DOI | title):
2026 | data-set | 10.25592/uhhfdm.19102 | Point cloud calorimeter shower datasets for multi-geometry pre-training: SimpleBox and LEMURS
2026 | data-set | 10.25592/uhhfdm.19103 | Point cloud calorimeter shower datasets for multi-geometry pre-training: SimpleBox and LEMURS
2026 | data-set | 10.25592/uhhfdm.19102 | Point cloud calorimeter shower datasets for multi-geometry pre-training: SimpleBox and LEMURS
2017 | other | 10.5281/zenodo.495149 | Introducing Orcid
2017 | other | 10.6084/m9.figshare.5479792.v1 | ORCID Public Data File 2017
2016 | other | 10.5281/zenodo.168190 | Introducing Orcid: What, Why And How
2016 | data-set | 10.6084/m9.figshare.4134027.v1 | ORCID Public Data File 2016
2016 | journal-article | 10.1002/chemv.201600091 | Your Lifelong Digital Name
2015 | data-set | 10.6084/m9.figshare.1582705 | ORCID Public Data File 2015
2014 | data-set | 10.14454/07243.2014.001 | ORCID Annual Public Data File, 2014
```

## Cell 4: Read ORCID JSON, and convert a document XML→JSON→XML

Goal: read ORCID JSON (`json::from_str`), and convert a document XML→JSON→XML (`json::to_string`, `xml::to_string`).

```rust
let mut out = String::new();
// ORCID JSON in: the kind is the XML root's local name
let doc = yorc::json::from_str("work", &read_text(WRITE_WORK_FULL_JSON)?)?;
let Root::Work(work) = &doc.root else {
    return Err("not a work".into());
};
let title = work.title.as_ref().and_then(|t| t.title.as_deref());
writeln!(
    out,
    "JSON work: {:?}, {}, published {}, {} external ids",
    title.unwrap_or("-"),
    work.work_type.as_ref().map_or("-", |t| t.as_str()),
    fuzzy(work.publication_date),
    work.external_ids.len(),
)?;
for c in &work.contributors {
    let id = c.contributor_orcid.as_ref().and_then(|o| o.path.as_deref());
    writeln!(
        out,
        "  contributor: {} ({})",
        c.credit_name.as_deref().unwrap_or("-"),
        id.unwrap_or("-")
    )?;
}

// XML -> JSON -> XML
let from_xml = yorc::xml::from_str(&read_text(WRITE_WORK_FULL_XML)?)?;
let kind = yorc::json::kind_of(&from_xml.root).ok_or("no JSON kind")?;
let json = yorc::json::to_string(&from_xml.root)?;
let from_json = yorc::json::from_str(kind, &json)?;
let xml_again = yorc::xml::to_string(&from_json.root)?;
let value = yorc::json::to_value(&from_xml.root)?;
let keys: Vec<&str> = value
    .as_object()
    .map(|o| o.keys().map(String::as_str).collect())
    .unwrap_or_default();
writeln!(out, "XML -> JSON: kind {kind:?}, keys: {}", keys.join(", "))?;
writeln!(
    out,
    "JSON -> XML: same model: {}, same XML: {}",
    from_json.root == from_xml.root,
    xml_again == yorc::xml::to_string(&from_xml.root)?
)?;
Ok(out)
```

The JSON kind is the XML root's local name (`json::KINDS`). JSON is written with sorted keys and no nulls; ORCID's `{"value": ...}` wrappers are kept.

Expected output:

<!-- derived: python3 -I json over write_samples/work-full-3.0.json (title, type, publication-date 2022-05-24, 2 external ids, 2 contributors); the JSON keys are the 11 child elements of write_samples/work-full-3.0.xml under their JSON names, sorted (json.rs w_work); 'same model' and 'same XML' follow from tests/json_roundtrip.rs (xml_samples_through_json) for a document without timestamps. -->
```text
JSON work: "Work Title", journal-article, published 2022-05-24, 2 external ids
  contributor: Jordan (0000-0002-5391-5437)
  contributor: Fran (0000-0002-4325-871X)
XML -> JSON: kind "work", keys: citation, contributors, country, external-ids, journal-title, language-code, publication-date, short-description, title, type, url
JSON -> XML: same model: true, same XML: true
```

## Cell 5: Flatten the record into section tables and export one as CSV

Goal: flatten the record into yorc-duckdb's section tables (`rows::Tables`, 14 sections) and export one as CSV.

```rust
let mut out = String::new();
// the rows yorc-duckdb's orcid_<section>_xml(xml) returns, source_hash included
let tables = Tables::from_xml(&read(RECORD)?)?;
for &section in Section::ALL {
    writeln!(
        out,
        "{:<24} {:>4} rows, {:>2} columns",
        section.name(),
        tables.get(section).len(),
        section.columns().len()
    )?;
}

// one section as CSV (RFC 4180 quoting; NULL is an empty field)
fn field(cell: &Cell) -> String {
    match cell {
        Cell::Null => String::new(),
        Cell::Varchar(s) if s.contains([',', '"', '\n', '\r']) => {
            format!("\"{}\"", s.replace('"', "\"\""))
        }
        Cell::Varchar(s) => s.clone(),
        Cell::Integer(i) => i.to_string(),
        Cell::Boolean(b) => b.to_string(),
        other => format!("{other:?}"),
    }
}
let section = Section::Works;
let mut csv = String::new();
let header: Vec<&str> = section.columns().iter().map(|c| c.name).collect();
writeln!(csv, "{}", header.join(","))?;
for row in tables.get(section) {
    let fields: Vec<String> = row.iter().map(field).collect();
    writeln!(csv, "{}", fields.join(","))?;
}
let file = std::env::temp_dir().join("yorc-rs-walkthrough-works.csv");
std::fs::write(file, &csv)?;
writeln!(
    out,
    "works.csv: a header and {} rows, written to the temp directory; the first lines:",
    tables.get(section).len()
)?;
for line in csv.lines().take(3) {
    writeln!(out, "{line}")?;
}
Ok(out)
```

`Tables::from_xml` fills `source_hash` with the SHA-256 of the file. The CSV quotes a field only when it holds a comma, a quote or a line end, and writes NULL as an empty field; the whole table goes to `yorc-rs-walkthrough-works.csv` in the system temp directory.

Expected output:

<!-- derived: python3 -I: section column counts from the const tables of src/rows.rs (sum 300); row counts from ElementTree: works 12, external_ids 26 (every common:external-id: 11 in the 10 work groups, 15 in the work summaries; grep -c '<common:external-id>' agrees), person 1, person_items 4 (the four other-name elements; researcher urls, emails, addresses, keywords and external identifiers are empty elements, and the biography is a column of person), history 1, the rest 0 (no fundings, affiliations, peer reviews or research resources, no contributors in summaries). CSV: the works columns of src/rows.rs; the first two work-summary elements (both in group 1); nothing needs quoting; source_hash = sha256sum of the record (59018b6d315be5...). -->
```text
works                      12 rows, 32 columns
fundings                    0 rows, 37 columns
affiliations                0 rows, 32 columns
peer_reviews                0 rows, 38 columns
research_resources          0 rows, 32 columns
research_resource_items     0 rows, 13 columns
contributors                0 rows, 12 columns
external_ids               26 rows, 13 columns
person                      1 rows, 15 columns
person_items                4 rows, 24 columns
history                     1 rows, 17 columns
search_results              0 rows, 13 columns
errors                      0 rows, 10 columns
research_resource_hosts     0 rows, 12 columns
works.csv: a header and 12 rows, written to the temp directory; the first lines:
orcid,put_code,group_index,display_index,title,subtitle,journal,type,pub_year,pub_month,pub_day,doi,url,source_name,visibility,created,last_modified,source_hash,translated_title,translated_title_language,short_description,citation_type,citation_value,language_code,country,path,source_client_id,source_orcid,assertion_origin_name,assertion_origin_orcid,assertion_origin_client_id,item
0000-0002-2771-9344,224052945,1,1,Point cloud calorimeter shower datasets for multi-geometry pre-training: SimpleBox and LEMURS,,Universität Hamburg,data-set,2026,8,17,10.25592/uhhfdm.19102,,DataCite,public,2026-08-17T12:42:27.716Z,2026-08-17T12:42:27.716Z,59018b6d315be538bb84a6105ff3ac1c4c274f3b467a455eb2576e03bbc2f86e,,,,,,,,/0000-0002-2771-9344/work/224052945,0000-0001-8099-6984,,,,,work-summary
0000-0002-2771-9344,224052944,1,1,Point cloud calorimeter shower datasets for multi-geometry pre-training: SimpleBox and LEMURS,,Universität Hamburg,data-set,2026,8,17,10.25592/uhhfdm.19103,,DataCite,public,2026-08-17T12:42:27.716Z,2026-08-17T12:42:27.716Z,59018b6d315be538bb84a6105ff3ac1c4c274f3b467a455eb2576e03bbc2f86e,,,,,,,,/0000-0002-2771-9344/work/224052944,0000-0001-8099-6984,,,,,work-summary
```

## Cell 6: Contributors and co-authors of the works

Goal: list the contributors of ORCID's two work-full samples and the co-authors with an iD across both.

```rust
let mut out = String::new();
let mut with_id: BTreeMap<String, String> = BTreeMap::new();
for (what, rel) in [
    ("read sample", READ_WORK_FULL),
    ("write sample", WRITE_WORK_FULL_XML),
] {
    let doc = yorc::xml::from_str(&read_text(rel)?)?;
    let Root::Work(work) = &doc.root else {
        return Err(format!("{rel}: not a work").into());
    };
    let title = work.title.as_ref().and_then(|t| t.title.as_deref());
    writeln!(
        out,
        "{what} {:?}: {} contributor(s) (name | iD | sequence | role)",
        title.unwrap_or("-"),
        work.contributors.len()
    )?;
    for (i, c) in work.contributors.iter().enumerate() {
        let id = c.contributor_orcid.as_ref().and_then(|o| o.path.as_deref());
        let name = c.credit_name.as_deref().unwrap_or("-");
        let attrs = c.attributes.as_ref();
        let sequence = attrs
            .and_then(|a| a.sequence.as_ref())
            .map_or("-", |s| s.as_str());
        let role = attrs
            .and_then(|a| a.role.as_ref())
            .map_or("-", |r| r.as_str());
        writeln!(
            out,
            "  {}. {name} | {} | {sequence} | {role}",
            i + 1,
            id.unwrap_or("-")
        )?;
        if let Some(id) = id {
            with_id
                .entry(id.to_string())
                .or_insert_with(|| name.to_string());
        }
    }
}
writeln!(
    out,
    "co-authors with an iD, across both works: {}",
    with_id.len()
)?;
for (id, name) in &with_id {
    writeln!(out, "  {id} {name}")?;
}
Ok(out)
```

The read sample's contributor is ORCID's placeholder iD 8888-8888-8888-8880, which is also the sample's source. `co-authors` here means every distinct contributor iD, sorted.

Expected output:

<!-- derived: python3 -I ElementTree over the two work-full-3.0.xml samples: work:contributors/work:contributor credit-name, contributor-orcid/path, contributor-sequence, contributor-role. -->
```text
read sample "common:title": 1 contributor(s) (name | iD | sequence | role)
  1. work:credit-name | 8888-8888-8888-8880 | first | author
write sample "Work Title": 1 contributor(s) (name | iD | sequence | role)
  1. Laure L. Haak | 0000-0001-5109-3700 | first | author
co-authors with an iD, across both works: 2
  0000-0001-5109-3700 Laure L. Haak
  8888-8888-8888-8880 work:credit-name
```

## Cell 7: An affiliation timeline

Goal: an affiliation timeline: employment and education, with organisation ids.

The record 0000-0002-2771-9344 has no affiliations, so this cell reads ORCID's sample record (`SAMPLE_RECORD`, `record-3.0.xml` from ORCID's read samples, fictitious iD 8888-8888-8888-8880), which has one summary of each affiliation kind. Its values are ORCID's placeholders (`education-org`, `education:role-title`, ...).

```rust
let mut out = String::new();
let doc = yorc::xml::from_slice(&read(SAMPLE_RECORD)?)?;
let summary = doc.activities_summary().ok_or("no activities summary")?;
let mut items: Vec<&Affiliation> = Vec::new();
for kind in [AffiliationKind::Education, AffiliationKind::Employment] {
    if let Some(list) = summary.affiliations(kind) {
        items.extend(list.groups.iter().flat_map(|g| &g.summaries));
    }
}
// oldest first: start year, month, day ascending, a missing part last
// (SQL's NULLS LAST), then kind, then put-code
items.sort_by_key(|a| {
    let d = a.start_date.unwrap_or_default();
    let last = |part: Option<i32>| (part.is_none(), part);
    (
        last(d.year),
        last(d.month),
        last(d.day),
        a.kind.as_str(),
        a.meta.put_code,
    )
});
writeln!(
    out,
    "{} affiliations, by start date (start to end | kind | organization | id | role):",
    items.len()
)?;
for a in &items {
    let org = a.organization.as_ref();
    let name = org.and_then(|o| o.name.as_deref()).unwrap_or("-");
    let id = org
        .and_then(|o| o.disambiguated_organization.as_ref())
        .map_or("-".to_string(), |d| {
            format!(
                "{} {}",
                d.source.as_deref().unwrap_or("?"),
                d.identifier.as_deref().unwrap_or("?")
            )
        });
    let role = a.role_title.as_deref().unwrap_or("-");
    writeln!(
        out,
        "{} to {} | {} | {name} | {id} | {role}",
        fuzzy(a.start_date),
        fuzzy(a.end_date),
        a.kind.as_str()
    )?;
}
Ok(out)
```

Sort order: oldest first, by start year, month and day ascending with a missing part last (as SQL's `NULLS LAST`, so undated items come last), then kind, then put-code. `id` is the disambiguated organisation (`RINGGOLD`, `ROR`, ...), `-` when the item has none.

Expected output:

<!-- derived: python3 -I ElementTree over reference/orcid-model/record_3.0/samples/read_samples/record-3.0.xml: the one education-summary and the one employment-summary, with start-date, end-date, organization/name, disambiguated-organization (source, identifier) and role-title. Both start on 1948-02-02 and have put-code 0, so the kind decides the order; the employment's end date has a year only. -->
```text
2 affiliations, by start date (start to end | kind | organization | id | role):
1948-02-02 to 2019-01-01 | education | education-org | GRID common:disambiguated-organization-identifier-education | education:role-title
1948-02-02 to 2025 | employment | employment-org | GRID common:disambiguated-organization-identifier-employment | employment:role-title
```

## Cell 8: Stream the Public Data File dump fixture

Goal: stream the Public Data File dump fixture: count the records, list their iDs, and draw a reproducible seeded sample with the crate's reservoir sampler (`dump::records_with`, `Limits`, `Reservoir`).

```rust
let mut out = String::new();
// every .xml member, read whole (at most 4 MiB each here; 64 MiB by default)
let mut archive = dump::open(&path(DUMP)).map_err(|e| format!("{DUMP}: {e}"))?;
let limits = Limits::default().with_max_member_bytes(4 << 20);
let (mut members, mut records, mut named) = (0, 0, 0);
for member in dump::records_with(&mut archive, limits)? {
    let member = member?;
    let doc = member.parse()?;
    let file_id = member.orcid();
    members += 1;
    records += usize::from(doc.record().is_some());
    named += usize::from(file_id.is_some());
    writeln!(
        out,
        "{} | {} | file name: {} | document: {}",
        member.path.display(),
        doc.root_name,
        file_id.map_or("not an iD".to_string(), |id| id.to_string()),
        doc.orcid().unwrap_or("-")
    )?;
}
writeln!(
    out,
    "XML members: {members}; records: {records}; named by a valid iD: {named}"
)?;

// a seeded sample of the records named by an iD, decided from the names alone:
// no member is read (read a kept one with dump::read_entry if you need it)
for seed in [42u64, 7] {
    let mut archive = dump::open(&path(DUMP))?;
    let mut reservoir = Reservoir::new(1, seed);
    for entry in archive.entries()? {
        let entry = entry?;
        let entry_path = entry.path()?.into_owned();
        let stem = dump::xml_entry_stem(&entry_path);
        let Some(id) = stem.and_then(|s| OrcidId::parse(s).ok()) else {
            continue;
        };
        let slot = reservoir.offer();
        if slot != Slot::Skip {
            reservoir.put(slot, id);
        }
    }
    let seen = reservoir.seen();
    let sample: Vec<String> = reservoir
        .into_items()
        .iter()
        .map(|id| id.to_string())
        .collect();
    writeln!(
        out,
        "sample of 1, seed {seed}: {} (of {seen} eligible)",
        sample.join(", ")
    )?;
}
Ok(out)
```

The fixture is `tests/data/dump/orcid_test_summaries.tar.gz` (byte-identical to yorc-duckdb's `test/data/dump/orcid_test_summaries.tar.gz` and yorc-zig's `testdata/dump/orcid_test_summaries.tgz`). It holds three `.xml` members (two `record` documents), a `README.txt` (skipped) and a member with a GNU long name; the third member is ORCID's `works-3.0.xml` read sample, under a file name that is not an iD. The sample is over the records named by a valid iD, with capacity 1: the first is kept, the second replaces it if `SplitMix64(seed).below(2)` is 0. The sample is decided from the member names alone, so no member is read; `dump::read_entry` reads a kept one.

Expected output:

<!-- derived: python3 -I tarfile over the fixture: the .xml file members in archive order, each root element's local name and the first segment of its @path; stems checked with the MOD 11-2 rule; the draws from SplitMix64 as specified in src/dump/reservoir.rs (seed 42: below(2) = 1, so the second record is skipped; seed 7: below(2) = 0, it replaces the first), as in yorc-zig's tests/dump_test.zig. -->
```text
ORCID_test_summaries/634/0000-0003-0100-6634.xml | record | file name: 0000-0003-0100-6634 | document: 0000-0003-0100-6634
ORCID_test_summaries/880/8888-8888-8888-8880.xml | record | file name: 8888-8888-8888-8880 | document: 8888-8888-8888-8880
ORCID_test_summaries/a-directory-name-long-enough-to-need-gnu-long-names-or-pax-headers/and-a-file-name-too-0000-0000-0000-0000.xml | works | file name: not an iD | document: 8888-8888-8888-8880
XML members: 3; records: 2; named by a valid iD: 2
sample of 1, seed 42: 0000-0003-0100-6634 (of 2 eligible)
sample of 1, seed 7: 8888-8888-8888-8880 (of 2 eligible)
```

## Cell 9: Getting one public record from ORCID

Goal: get one public record from ORCID's Public API with the blocking client (`Access::Public`). Not run unless you pass `-- 9 --network`.

```rust
let mut out = String::new();
// Access::Public: pub.orcid.org, public reads only (no Member API, no Sandbox)
let client = Client::new(Config::new(Access::Public));
writeln!(
    out,
    "one request every {} s to ORCID, at most (the client enforces the cap)",
    client.min_interval().as_secs()
)?;
writeln!(
    out,
    "bulk data must come from the Public Data File (cell 8), never from crawling the API"
)?;
if !network {
    writeln!(out, "not run: pass `-- 9 --network` to read one record")?;
    return Ok(out);
}
let record = client.record("0000-0002-2771-9344", None)?;
let name = record.person.as_ref().and_then(|p| p.name.as_ref());
writeln!(
    out,
    "read 0000-0002-2771-9344: {} {}",
    name.and_then(|n| n.given_names.as_deref()).unwrap_or(""),
    name.and_then(|n| n.family_name.as_deref()).unwrap_or("")
)?;
let works = record
    .activities_summary
    .as_ref()
    .map_or(0, |a| a.work_summaries().count());
writeln!(out, "works now: {works}")?;
Ok(out)
```

The client paces every request to ORCID's hosts through one process-wide limiter: **one request every 45 s**, strictly one at a time (`ORCID_INTERVAL`; a faster limiter is clamped to it). It is for occasional single-record retrieval: **bulk data must come from the Public Data File** (cell 8 and the `dump` module), never from crawling the API. The client reads only ORCID's Public API (`pub.orcid.org`): it does not provide access to the ORCID Sandbox or the ORCID Member API, and it writes nothing.

Expected output (offline, and in a full run):

<!-- derived: The first line: ORCID_INTERVAL = 45 s (src/blocking.rs). The live lines depend on ORCID's data today. -->
```text
one request every 45 s to ORCID, at most (the client enforces the cap)
bulk data must come from the Public Data File (cell 8), never from crawling the API
not run: pass `-- 9 --network` to read one record
```

With `-- 9 --network`, the last line is replaced by two lines read from
`pub.orcid.org` (live data, so not exact; the saved copy has 12 works):

```text
read 0000-0002-2771-9344: Sofia Hernandez
works now: <the number of works the record has today>
```
