//! Group-id records (model, XML and JSON), the `timeline` and `group_ids`
//! sections and the snapshot diff (`rows::changes`), against yorc-duckdb's
//! own expectations (copied from its test/sql/orcid_history_of_record.test
//! and orcid_api_messages.test) and against values taken from the sample
//! files with grep and python3 -I (a port of yorc-duckdb's extractor run over
//! the same files; cited where used).
//!
//! Needs features `rows` (and `json` for the JSON tests): `cargo test
//! --all-features`.
#![cfg(feature = "rows")]

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use yorc::model::{GroupIdRecord, GroupType, Root};
use yorc::rows::{CHANGE_COLUMNS, Cell, ChangeKind, Extra, Row, Section, changes};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> Vec<u8> {
    let path = crate_root().join(rel);
    fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn read_sample(name: &str) -> Vec<u8> {
    read(&format!(
        "reference/orcid-model/record_3.0/samples/read_samples/{name}"
    ))
}

const GROUP_ID_SAMPLE: &str = "reference/orcid-model/group-id-3.0/samples/group-id-3.0.xml";
const GROUP_ID_PAGE: &str = "tests/data/group-id-records.xml";
const FIXTURE: &str = "tests/data/0000-0003-0100-6634.xml";

/// Cell as text, NULL as `<null>` (the convention of tests/samples.rs).
fn text(c: &Cell) -> String {
    match c {
        Cell::Null => "<null>".to_string(),
        Cell::Varchar(v) => v.clone(),
        Cell::Integer(v) => v.to_string(),
        Cell::Boolean(v) => v.to_string(),
        other => format!("{other:?}"),
    }
}

fn get(extra: Extra, rows: &[Row], row: usize, col: &str) -> String {
    let i = extra
        .column_index(col)
        .unwrap_or_else(|| panic!("no column {col}"));
    text(&rows[row][i])
}

fn rows_of(extra: Extra, xml: &[u8]) -> Vec<Row> {
    yorc::rows::Tables::extra_from_xml(extra, xml).expect("rows")
}

// --------------------------------------------------------------- schema

#[test]
fn extra_schemas_match_yorc_duckdb() {
    // orcid_history_of_record.test: DESCRIBE orcid_timeline_xml has 13
    // columns; orcid_api_messages.test: orcid_group_ids_xml has 12.
    // Names and types: yorc-duckdb/src/core/sections.cpp (kColumns).
    let names = |e: Extra| e.columns().iter().map(|c| c.name).collect::<Vec<_>>();
    assert_eq!(
        names(Extra::Timeline),
        [
            "orcid",
            "event_at",
            "event",
            "kind",
            "put_code",
            "label",
            "organization",
            "visibility",
            "source_name",
            "created",
            "last_modified",
            "path",
            "source_hash"
        ]
    );
    assert_eq!(
        names(Extra::GroupIds),
        [
            "put_code",
            "name",
            "group_id",
            "description",
            "type",
            "source_name",
            "source_client_id",
            "source_orcid",
            "total",
            "page",
            "page_size",
            "source_hash"
        ]
    );
    let types = |e: Extra| {
        e.columns()
            .iter()
            .map(|c| format!("{:?}", c.ty))
            .collect::<Vec<_>>()
            .join(",")
    };
    assert_eq!(
        types(Extra::GroupIds),
        "Bigint,Varchar,Varchar,Varchar,Varchar,Varchar,Varchar,Varchar,Integer,Integer,Integer,Varchar"
    );
    assert_eq!(
        types(Extra::Timeline),
        "Varchar,Varchar,Varchar,Varchar,Bigint,Varchar,Varchar,Varchar,Varchar,Varchar,Varchar,Varchar,Varchar"
    );
    assert_eq!(Extra::ALL, &[Extra::Timeline, Extra::GroupIds]);
    assert_eq!(Extra::from_name("group_ids"), Some(Extra::GroupIds));
    assert_eq!(Extra::from_name("works"), None);
    // the fourteen sections are unchanged
    assert_eq!(Section::ALL.len(), 14);
    assert_eq!(
        Section::ALL
            .iter()
            .map(|s| s.columns().len())
            .sum::<usize>(),
        300
    );
    assert_eq!(
        CHANGE_COLUMNS.iter().map(|c| c.name).collect::<Vec<_>>(),
        [
            "orcid",
            "section",
            "kind",
            "put_code",
            "change",
            "field",
            "old_value",
            "new_value",
            "label"
        ]
    );
}

// ------------------------------------------------------------ group ids

#[test]
fn group_id_sample_reads_and_rows() {
    let bytes = read(GROUP_ID_SAMPLE);
    let doc = yorc::xml::from_slice(&bytes).unwrap();
    assert_eq!(doc.root_name, "group-id-record");
    let Root::GroupIdRecord(g) = &doc.root else {
        panic!("not a group-id record: {:?}", doc.root);
    };
    assert_eq!(g.name.as_deref(), Some("group-id:name"));
    assert_eq!(g.group_id.as_deref(), Some("issn:xxxxxxxx"));
    assert_eq!(g.description.as_deref(), Some("group-id:description"));
    assert_eq!(g.group_type, Some(GroupType::Publisher));
    assert_eq!((g.put_code, &g.source), (None, &None));

    // orcid_api_messages.test: SELECT put_code, name, group_id, description,
    // type, source_name, total FROM orcid_group_ids('.../group-id-3.0.xml')
    let rows = rows_of(Extra::GroupIds, &bytes);
    assert_eq!(rows.len(), 1);
    let line: Vec<String> = [
        "put_code",
        "name",
        "group_id",
        "description",
        "type",
        "source_name",
        "total",
    ]
    .iter()
    .map(|c| get(Extra::GroupIds, &rows, 0, c))
    .collect();
    assert_eq!(
        line,
        [
            "<null>",
            "group-id:name",
            "issn:xxxxxxxx",
            "group-id:description",
            "publisher",
            "<null>",
            "<null>"
        ]
    );
    assert_eq!(
        get(Extra::GroupIds, &rows, 0, "source_hash"),
        yorc::hash::sha256_hex(&bytes)
    );
    // no other section has rows
    let t = yorc::rows::Tables::from_document(&doc, None);
    assert!(Section::ALL.iter().all(|&s| t.get(s).is_empty()));
    assert!(yorc::rows::timeline(&doc, None).is_empty());

    // written back: the layout of the record writer
    let xml = yorc::xml::to_string(&doc.root).unwrap();
    assert_eq!(
        xml,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <group-id:group-id-record xmlns:group-id=\"http://www.orcid.org/ns/group-id\">\n\
         \x20 <group-id:name>group-id:name</group-id:name>\n\
         \x20 <group-id:group-id>issn:xxxxxxxx</group-id:group-id>\n\
         \x20 <group-id:description>group-id:description</group-id:description>\n\
         \x20 <group-id:type>publisher</group-id:type>\n\
         </group-id:group-id-record>\n"
    );
    assert_eq!(yorc::xml::from_str(&xml).unwrap().root, doc.root);
}

#[test]
fn group_id_records_page() {
    // orcid_api_messages.test: the group-id-records page (tests/data holds
    // the same document) gives these rows, ORDER BY put_code
    let bytes = read(GROUP_ID_PAGE);
    let rows = rows_of(Extra::GroupIds, &bytes);
    let cols = [
        "put_code",
        "name",
        "group_id",
        "type",
        "source_client_id",
        "source_name",
        "total",
        "page",
        "page_size",
    ];
    let lines: Vec<String> = (0..rows.len())
        .map(|r| {
            cols.iter()
                .map(|c| get(Extra::GroupIds, &rows, r, c))
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect();
    assert_eq!(
        lines,
        [
            "11|A|issn:0000-0001|journal|APP-1|Client|2|0|100",
            "12|B|issn:0000-0002|conference|<null>|<null>|2|0|100"
        ]
    );
    assert_eq!(get(Extra::GroupIds, &rows, 1, "description"), "Second");

    let doc = yorc::xml::from_slice(&bytes).unwrap();
    let Root::GroupIdRecords(p) = &doc.root else {
        panic!("not a group-id-records page");
    };
    assert_eq!(
        (p.total, p.page, p.page_size),
        (Some(2), Some(0), Some(100))
    );
    assert_eq!(p.records.len(), 2);
    // the inline source fields (group-id-3.0.xsd) read into `source`
    let s = p.records[0].source.as_ref().unwrap();
    assert_eq!(
        s.source_client_id.as_ref().and_then(|c| c.path.as_deref()),
        Some("APP-1")
    );
    assert_eq!(s.source_name.as_deref(), Some("Client"));
    assert_eq!(p.records[1].source, None);

    // written with common:source; reads back to the same model
    let xml = yorc::xml::to_string(&doc.root).unwrap();
    assert_eq!(
        xml,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <group-id:group-id-records xmlns:common=\"http://www.orcid.org/ns/common\" \
         xmlns:group-id=\"http://www.orcid.org/ns/group-id\">\n\
         \x20 <group-id:total>2</group-id:total>\n\
         \x20 <group-id:page>0</group-id:page>\n\
         \x20 <group-id:page-size>100</group-id:page-size>\n\
         \x20 <group-id:group-id-record put-code=\"11\">\n\
         \x20   <group-id:name>A</group-id:name>\n\
         \x20   <group-id:group-id>issn:0000-0001</group-id:group-id>\n\
         \x20   <group-id:description>First</group-id:description>\n\
         \x20   <group-id:type>journal</group-id:type>\n\
         \x20   <common:source>\n\
         \x20     <common:source-client-id>\n\
         \x20       <common:path>APP-1</common:path>\n\
         \x20     </common:source-client-id>\n\
         \x20     <common:source-name>Client</common:source-name>\n\
         \x20   </common:source>\n\
         \x20 </group-id:group-id-record>\n\
         \x20 <group-id:group-id-record put-code=\"12\">\n\
         \x20   <group-id:name>B</group-id:name>\n\
         \x20   <group-id:group-id>issn:0000-0002</group-id:group-id>\n\
         \x20   <group-id:description>Second</group-id:description>\n\
         \x20   <group-id:type>conference</group-id:type>\n\
         \x20 </group-id:group-id-record>\n\
         </group-id:group-id-records>\n"
    );
    assert_eq!(yorc::xml::from_str(&xml).unwrap().root, doc.root);
    // written rows equal the read ones (but for the source hash)
    let again = yorc::rows::group_ids(&yorc::xml::from_str(&xml).unwrap(), None);
    let n = Extra::GroupIds.columns().len() - 1;
    for (a, b) in rows.iter().zip(&again) {
        assert_eq!(a[..n], b[..n]);
    }
}

#[test]
fn group_id_dates_and_bad_text() {
    let g = GroupIdRecord {
        put_code: Some(7),
        created_date: Some("2020-01-01T00:00:00.000Z".to_string()),
        last_modified_date: Some("2021-01-01T00:00:00.000Z".to_string()),
        name: Some("R&D <x>".to_string()),
        group_type: Some(GroupType::PeerReviewService),
        ..GroupIdRecord::default()
    };
    let xml = yorc::xml::to_string(&Root::GroupIdRecord(g.clone())).unwrap();
    // ORCID's GroupIdRecord propOrder: last-modified-date before created-date
    assert_eq!(
        xml,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <group-id:group-id-record xmlns:common=\"http://www.orcid.org/ns/common\" \
         xmlns:group-id=\"http://www.orcid.org/ns/group-id\" put-code=\"7\">\n\
         \x20 <common:last-modified-date>2021-01-01T00:00:00.000Z</common:last-modified-date>\n\
         \x20 <common:created-date>2020-01-01T00:00:00.000Z</common:created-date>\n\
         \x20 <group-id:name>R&amp;D &lt;x&gt;</group-id:name>\n\
         \x20 <group-id:type>peer-review service</group-id:type>\n\
         </group-id:group-id-record>\n"
    );
    assert_eq!(
        yorc::xml::from_str(&xml).unwrap().root,
        Root::GroupIdRecord(g)
    );
    let empty = yorc::xml::to_string(&Root::GroupIdRecord(GroupIdRecord::default())).unwrap();
    assert_eq!(
        empty,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <group-id:group-id-record xmlns:group-id=\"http://www.orcid.org/ns/group-id\"/>\n"
    );
    let bad = GroupIdRecord {
        description: Some("bell\u{7}".to_string()),
        ..GroupIdRecord::default()
    };
    assert!(matches!(
        yorc::xml::to_string(&Root::GroupIdRecord(bad)),
        Err(yorc::Error::Unwritable { ref path, .. }) if path == "group-id-record/description"
    ));
}

#[cfg(feature = "json")]
#[test]
fn group_id_json() {
    // ORCID's write sample, read as ORCID's GroupIdRecord reads it
    let text = String::from_utf8(read(
        "reference/orcid-model/record_3.0/samples/write_samples/peer-review-group-id.json",
    ))
    .unwrap();
    let doc = yorc::json::from_str("group-id-record", &text).unwrap();
    let Root::GroupIdRecord(g) = &doc.root else {
        panic!("not a group-id record");
    };
    assert_eq!(g.name.as_deref(), Some("Group ID name"));
    assert_eq!(g.group_id.as_deref(), Some("ringgold:355321"));
    assert_eq!(
        g.description.as_deref(),
        Some("Description for a peer-review group id.")
    );
    assert_eq!(g.group_type, Some(GroupType::Magazine));
    assert_eq!(yorc::json::kind_of(&doc.root), Some("group-id-record"));
    assert!(yorc::json::KINDS.contains(&"group-id-records"));
    let json = yorc::json::to_string(&doc.root).unwrap();
    assert_eq!(
        json,
        "{\n  \"description\": \"Description for a peer-review group id.\",\n  \
         \"group-id\": \"ringgold:355321\",\n  \"name\": \"Group ID name\",\n  \
         \"type\": \"magazine\"\n}\n"
    );
    assert_eq!(
        yorc::json::from_str("group-id-record", &json).unwrap().root,
        doc.root
    );
    let rows = yorc::rows::group_ids(&doc, Some("h"));
    assert_eq!(get(Extra::GroupIds, &rows, 0, "type"), "magazine");
    assert_eq!(get(Extra::GroupIds, &rows, 0, "source_hash"), "h");

    // the page, through JSON and back; dates are XML-only (ORCID's
    // GroupIdRecord ignores them in JSON), the page's date is kept
    let page = yorc::xml::from_slice(&read(GROUP_ID_PAGE)).unwrap();
    let json = yorc::json::to_string(&page.root).unwrap();
    let back = yorc::json::from_str("group-id-records", &json).unwrap();
    assert_eq!(back.root, page.root);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["total"], serde_json::json!(2));
    assert_eq!(v["group-id-record"][0]["put-code"], serde_json::json!(11));
    assert_eq!(
        v["group-id-record"][0]["source"]["source-name"],
        serde_json::json!({"value": "Client"})
    );
    let dated = GroupIdRecord {
        created_date: Some("2020-01-01T00:00:00.000Z".to_string()),
        ..GroupIdRecord::default()
    };
    assert_eq!(
        yorc::json::to_string(&Root::GroupIdRecord(dated)).unwrap(),
        "{}\n"
    );
    // inline source keys, as the XSD's source-type has them
    let inline = yorc::json::from_str(
        "group-id-record",
        r#"{"source-name": {"value": "Client"}, "put-code": 3}"#,
    )
    .unwrap();
    let Root::GroupIdRecord(g) = &inline.root else {
        panic!()
    };
    assert_eq!(
        g.source.as_ref().and_then(|s| s.source_name.as_deref()),
        Some("Client")
    );
    assert_eq!(g.put_code, Some(3));
}

// ------------------------------------------------------------- timeline

#[test]
fn timeline_of_the_record_sample() {
    let rows = rows_of(Extra::Timeline, &read_sample("record-3.0.xml"));
    // orcid_history_of_record.test: count(*), created, modified, kinds
    let events: Vec<String> = (0..rows.len())
        .map(|r| get(Extra::Timeline, &rows, r, "event"))
        .collect();
    let kinds: BTreeSet<String> = (0..rows.len())
        .map(|r| get(Extra::Timeline, &rows, r, "kind"))
        .collect();
    assert_eq!(rows.len(), 20);
    assert_eq!(events.iter().filter(|e| *e == "created").count(), 18);
    assert_eq!(events.iter().filter(|e| *e == "modified").count(), 2);
    assert_eq!(kinds.len(), 19);
    // ... WHERE event_at > '2002' ORDER BY event_at
    let late: Vec<String> = (0..rows.len())
        .filter(|&r| {
            let at = get(Extra::Timeline, &rows, r, "event_at");
            at != "<null>" && at.as_str() > "2002"
        })
        .map(|r| {
            ["event_at", "event", "kind", "label"]
                .iter()
                .map(|c| get(Extra::Timeline, &rows, r, c))
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect();
    assert_eq!(
        late,
        [
            "2015-06-25T16:01:12.718Z|created|research-resource|Giant Laser Award",
            "2017-09-08T13:31:19.987Z|modified|research-resource|Giant Laser Award"
        ]
    );
    // ... WHERE kind IN ('employment', 'keyword', 'name') ORDER BY kind
    let mut picked: Vec<String> = (0..rows.len())
        .filter(|&r| {
            ["employment", "keyword", "name"]
                .contains(&get(Extra::Timeline, &rows, r, "kind").as_str())
        })
        .map(|r| {
            format!(
                "{}|{}",
                get(Extra::Timeline, &rows, r, "kind"),
                get(Extra::Timeline, &rows, r, "label")
            )
        })
        .collect();
    picked.sort();
    assert_eq!(
        picked,
        [
            "employment|employment:role-title",
            "keyword|keyword1",
            "name|give-names"
        ]
    );
    // document order (python3 -I, the extractor port over record-3.0.xml):
    // the name first (a modification date only), then the person items,
    // then the activities in activities-summary order
    assert_eq!(
        kinds_in_order(&rows),
        "name/modified other-name researcher-url email address keyword \
         external-identifier distinction education employment funding \
         invited-position membership peer-review qualification research-resource \
         research-resource/modified service work"
    );
    assert_eq!(
        get(Extra::Timeline, &rows, 2, "label"),
        "biography".to_string()
    );
    let r = 16; // research-resource, created
    assert_eq!(get(Extra::Timeline, &rows, r, "put_code"), "1234");
    assert_eq!(get(Extra::Timeline, &rows, r, "organization"), "XSEDE");
    assert_eq!(
        get(Extra::Timeline, &rows, r, "source_name"),
        "XSEDE ORCID integration"
    );
    assert_eq!(get(Extra::Timeline, &rows, r, "visibility"), "public");
    assert_eq!(
        get(Extra::Timeline, &rows, r, "path"),
        "/0000-0003-0902-4386/research-resource/1234"
    );
    assert_eq!(
        get(Extra::Timeline, &rows, r, "orcid"),
        "8888-8888-8888-8880"
    );
    let f = 11; // funding: title and organization
    assert_eq!(get(Extra::Timeline, &rows, f, "label"), "common:title");
    assert_eq!(
        get(Extra::Timeline, &rows, f, "organization"),
        "common:name"
    );
    let p = 14; // peer-review-summary: no subject name
    assert_eq!(get(Extra::Timeline, &rows, p, "label"), "<null>");
    assert_eq!(
        get(Extra::Timeline, &rows, p, "organization"),
        "common:name"
    );
}

/// The rows' kinds, with `/modified` on modification rows and the biography
/// left out (it sits between the other names and the researcher URLs).
fn kinds_in_order(rows: &[Row]) -> String {
    (0..rows.len())
        .filter(|&r| get(Extra::Timeline, rows, r, "kind") != "biography")
        .map(|r| {
            let kind = get(Extra::Timeline, rows, r, "kind");
            match get(Extra::Timeline, rows, r, "event").as_str() {
                "modified" => format!("{kind}/modified"),
                _ => kind,
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn timeline_rows_per_sample() {
    // python3 -I, the extractor port over every sample: (rows, created rows,
    // undated rows); every other XML sample has none
    let expected: &[(&str, &str, usize, usize, usize)] = &[
        ("read_samples", "activities-3.0.xml", 12, 11, 0),
        ("read_samples", "addresses-3.0.xml", 2, 2, 0),
        ("read_samples", "bulk-work-err.xml", 1, 1, 0),
        ("read_samples", "distinction-3.0.xml", 1, 1, 0),
        ("read_samples", "distinctions-3.0.xml", 2, 2, 0),
        ("read_samples", "education-3.0.xml", 1, 1, 0),
        ("read_samples", "education-full-3.0.xml", 1, 1, 0),
        ("read_samples", "educations-3.0.xml", 2, 2, 0),
        ("read_samples", "emails-3.0.xml", 2, 2, 0),
        ("read_samples", "employment-3.0.xml", 1, 1, 0),
        ("read_samples", "employment-full-3.0.xml", 1, 1, 0),
        ("read_samples", "employments-3.0.xml", 2, 2, 0),
        ("read_samples", "external-identifiers-3.0.xml", 2, 2, 0),
        ("read_samples", "funding-3.0.xml", 1, 1, 0),
        ("read_samples", "funding-full-3.0.xml", 1, 1, 0),
        ("read_samples", "fundings-3.0.xml", 6, 3, 0),
        ("read_samples", "invited-position-3.0.xml", 1, 1, 0),
        ("read_samples", "invited-positions-3.0.xml", 2, 2, 0),
        ("read_samples", "keywords-3.0.xml", 2, 2, 0),
        ("read_samples", "membership-3.0.xml", 1, 1, 0),
        ("read_samples", "memberships-3.0.xml", 2, 2, 0),
        ("read_samples", "other-names-3.0.xml", 2, 2, 0),
        ("read_samples", "peer-review-3.0.xml", 1, 1, 0),
        ("read_samples", "peer-review-full-3.0.xml", 1, 1, 0),
        ("read_samples", "person-3.0.xml", 8, 8, 0),
        ("read_samples", "personal-details-3.0.xml", 4, 3, 1),
        ("read_samples", "qualification-3.0.xml", 1, 1, 0),
        ("read_samples", "qualifications-3.0.xml", 2, 2, 0),
        ("read_samples", "record-3.0.xml", 20, 18, 0),
        (
            "read_samples",
            "record-no-city-country-for-professional-activities-3.0.xml",
            20,
            18,
            0,
        ),
        ("read_samples", "research-resource-3.0.xml", 2, 1, 0),
        ("read_samples", "research-resources-3.0.xml", 4, 2, 0),
        ("read_samples", "researcher-urls-3.0.xml", 1, 1, 0),
        ("read_samples", "service-3.0.xml", 1, 1, 0),
        ("read_samples", "services-3.0.xml", 2, 2, 0),
        ("read_samples", "work-3.0.xml", 1, 1, 0),
        ("read_samples", "work-full-3.0.xml", 1, 1, 0),
        ("read_samples", "work-review-3.0.xml", 1, 1, 0),
        ("read_samples", "works-3.0.xml", 5, 3, 0),
        ("write_samples", "bulk-work-3.0.xml", 2, 0, 2),
        ("write_samples", "distinction-3.0.xml", 1, 0, 1),
        ("write_samples", "education-3.0.xml", 1, 0, 1),
        ("write_samples", "employment-3.0.xml", 1, 0, 1),
        ("write_samples", "funding-3.0.xml", 1, 0, 1),
        ("write_samples", "invited-position-3.0.xml", 1, 0, 1),
        ("write_samples", "membership-3.0.xml", 1, 0, 1),
        ("write_samples", "peer-review-full-3.0.xml", 1, 0, 1),
        (
            "write_samples",
            "peer-review-full-award-subject-type-3.0.xml",
            1,
            0,
            1,
        ),
        ("write_samples", "peer-review-simple-3.0.xml", 1, 0, 1),
        ("write_samples", "qualification-3.0.xml", 1, 0, 1),
        ("write_samples", "research-resource-3.0.xml", 1, 0, 1),
        ("write_samples", "service-3.0.xml", 1, 0, 1),
        ("write_samples", "work-full-3.0.xml", 1, 0, 1),
        ("write_samples", "work-simple-3.0.xml", 1, 0, 1),
    ];
    let mut seen = 0;
    for set in ["read_samples", "write_samples"] {
        let dir = crate_root()
            .join("reference/orcid-model/record_3.0/samples")
            .join(set);
        let mut names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".xml"))
            .collect();
        names.sort();
        for name in names {
            let rows = rows_of(Extra::Timeline, &fs::read(dir.join(&name)).unwrap());
            let count = |ev: &str| {
                (0..rows.len())
                    .filter(|&r| get(Extra::Timeline, &rows, r, "event") == ev)
                    .count()
            };
            let got = (rows.len(), count("created"), count("<null>"));
            match expected.iter().find(|e| e.0 == set && e.1 == name) {
                Some(e) => {
                    assert_eq!(got, (e.2, e.3, e.4), "{set}/{name}");
                    seen += 1;
                }
                None => assert_eq!(got, (0, 0, 0), "{set}/{name}"),
            }
        }
    }
    assert_eq!(seen, expected.len());
}

#[test]
fn timeline_of_the_fixture() {
    let bytes = read(FIXTURE);
    let rows = rows_of(Extra::Timeline, &bytes);
    let count = |col: &str, v: &str| {
        (0..rows.len())
            .filter(|&r| get(Extra::Timeline, &rows, r, col) == v)
            .count()
    };
    // orcid_history_of_record.test: count(*) >= 382 WHERE kind = 'work';
    // exact counts from python3 -I (the extractor port over the fixture)
    assert_eq!(count("kind", "work"), 679);
    assert_eq!(rows.len(), 940);
    assert_eq!(count("event", "created"), 521);
    assert_eq!(count("event", "modified"), 419);
    assert_eq!(count("kind", "service"), 54);
    assert_eq!(count("kind", "keyword"), 39);
    assert_eq!(count("kind", "researcher-url"), 38);
    assert_eq!(count("kind", "employment"), 35);
    assert_eq!(count("kind", "name"), 2);
    assert_eq!(count("orcid", "0000-0003-0100-6634"), 940);
    let h = yorc::hash::sha256_hex(&bytes);
    assert_eq!(count("source_hash", &h), 940);
}

// -------------------------------------------------------------- changes

fn doc(rel: &str) -> yorc::model::Document {
    yorc::xml::from_slice(&read(rel)).unwrap()
}

fn change_lines(c: &[yorc::rows::Change]) -> Vec<String> {
    c.iter()
        .map(|c| {
            c.to_row()
                .iter()
                .skip(1)
                .map(|cell| match cell {
                    Cell::Null => "NULL".to_string(),
                    other => text(other),
                })
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect()
}

#[test]
fn changes_between_two_snapshots() {
    let old = doc("tests/data/changes/old.xml");
    let new = doc("tests/data/changes/new.xml");
    // orcid_history_of_record.test: SELECT section, kind, put_code, change,
    // field, old_value, new_value, label FROM orcid_changes(old, new)
    let c = changes(Some(&old), Some(&new));
    assert_eq!(
        change_lines(&c),
        [
            "person|NULL|NULL|modified|given_names|give-names|given-names|given-names",
            "person_items|keyword|2|added|NULL|NULL|NULL|keyword2",
            "works|NULL|0|modified|title|common:title|Revised title|Revised title",
            "fundings|NULL|0|removed|NULL|NULL|NULL|common:title",
            "affiliations|employment|0|modified|role_title|employment:role-title|\
             Senior employment:role-title|Senior employment:role-title",
        ]
    );
    assert!(
        c.iter()
            .all(|c| c.orcid.as_deref() == Some("8888-8888-8888-8880"))
    );
    assert_eq!(c[1].change, ChangeKind::Added);
    assert_eq!(c[1].section, Section::PersonItems);
    // identical snapshots: no changes
    assert!(changes(Some(&old), Some(&old)).is_empty());
    // reversing the direction swaps added/removed
    let mut swapped: Vec<String> = changes(Some(&new), Some(&old))
        .iter()
        .filter(|c| c.change != ChangeKind::Modified)
        .map(|c| format!("{} {}", c.change.as_str(), c.section.name()))
        .collect();
    swapped.sort_by(|a, b| a.split(' ').nth(1).cmp(&b.split(' ').nth(1)));
    assert_eq!(swapped, ["added fundings", "removed person_items"]);
    // no old snapshot: everything on the new side is added
    let all = changes(None, Some(&new));
    assert_eq!(all.len(), 19);
    assert!(all.iter().all(|c| c.change == ChangeKind::Added));
    assert!(changes(None, None).is_empty());
}

#[test]
fn changes_of_an_edited_work() {
    // the bench's changes_record pair: the fixture, and the fixture with
    // "Edited: " after the first <common:title> of its first work summary
    let bytes = read(FIXTURE);
    let at = find(&bytes, b"<work:work-summary", 0);
    let at = find(&bytes, b"<common:title>", at) + b"<common:title>".len();
    let mut edited = bytes[..at].to_vec();
    edited.extend_from_slice(b"Edited: ");
    edited.extend_from_slice(&bytes[at..]);
    let old = yorc::xml::from_slice(&bytes).unwrap();
    let new = yorc::xml::from_slice(&edited).unwrap();
    // python3 -I, the changes.cpp port over the same pair
    let c = changes(Some(&old), Some(&new));
    assert_eq!(
        change_lines(&c),
        [
            "works|NULL|182082411|modified|title|mgbilby/oga_src: initial oga_src release|\
          Edited: mgbilby/oga_src: initial oga_src release|\
          Edited: mgbilby/oga_src: initial oga_src release"
        ]
    );
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> usize {
    from + hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .expect("needle")
}
