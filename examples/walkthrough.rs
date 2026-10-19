//! A notebook-like walkthrough of `yorc-rs`: ten cells (0 to 9),
//! each a function returning its output as a `String`. WALKTHROUGH.md shows
//! each cell's body with its expected output, and `tests/walkthrough.rs`
//! runs the same functions and checks their output against that file.
//!
//! ```sh
//! cargo run --example walkthrough --all-features             # cells 0 to 9 (9 only explains itself)
//! cargo run --example walkthrough --all-features -- 5        # one cell
//! cargo run --example walkthrough --all-features -- 9 --network    # the one cell that uses the network
//! ```
//!
//! Inputs are files of the repository, by path relative to the package root
//! (`CARGO_MANIFEST_DIR`), so the walkthrough runs from any directory.

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

/// Runs cell `n`; `network` lets cell 9 reach ORCID.
pub fn run(n: u32, network: bool) -> CellResult {
    match n {
        0 => cell_0(),
        1 => cell_1(),
        2 => cell_2(),
        3 => cell_3(),
        4 => cell_4(),
        5 => cell_5(),
        6 => cell_6(),
        7 => cell_7(),
        8 => cell_8(),
        9 => cell_9(network),
        _ => Err(format!("no cell {n}").into()),
    }
}

pub fn cell_0() -> CellResult {
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
}

pub fn cell_1() -> CellResult {
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
}

pub fn cell_2() -> CellResult {
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
}

pub fn cell_3() -> CellResult {
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
}

pub fn cell_4() -> CellResult {
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
}

pub fn cell_5() -> CellResult {
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
}

pub fn cell_6() -> CellResult {
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
}

pub fn cell_7() -> CellResult {
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
}

pub fn cell_8() -> CellResult {
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
}

pub fn cell_9(network: bool) -> CellResult {
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
}

/// Writes to stdout. A reader that closed the pipe early (`| head`) ends the
/// program quietly instead of a panic, as `print!` would give.
fn put(s: &str) {
    use std::io::Write as _;
    if let Err(e) = std::io::stdout().write_all(s.as_bytes()) {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        eprintln!("walkthrough: {e}");
        std::process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let network = args.iter().any(|a| a == "--network");
    let mut only = None;
    for a in args.iter().filter(|a| a.as_str() != "--network") {
        match a.parse::<u32>() {
            Ok(n) if CELLS.iter().any(|c| c.0 == n) && only.is_none() => only = Some(n),
            _ => {
                eprintln!(
                    "usage: walkthrough [CELL 0-9] [--network]  (--network only with cell 9)"
                );
                std::process::exit(2);
            }
        }
    }
    let mut failed = false;
    for &(n, title) in CELLS {
        if only.is_some_and(|o| o != n) {
            continue;
        }
        put(&format!("## Cell {n}: {title}\n"));
        // the network cell reaches ORCID only when asked for by number
        match run(n, network && only == Some(9)) {
            Ok(text) => put(&text),
            Err(e) => {
                put(&format!("error: {e}\n"));
                failed = true;
            }
        }
        put("\n");
    }
    if failed {
        std::process::exit(1);
    }
}
