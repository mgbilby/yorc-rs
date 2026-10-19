//! Reads every ORCID v3.0 read sample and the real-record fixture
//! (`tests/data/0000-0003-0100-6634.xml`, also yorc-duckdb's), and checks
//! model values and section rows against the yorc-duckdb extension's own
//! expectations (copied here from its test/sql/orcid_sections.test,
//! orcid_works.test, orcid_works_xml.test, orcid_api_messages.test,
//! orcid_locked_deprecated.test, orcid_activity_items.test, orcid_json.test,
//! orcid_write.test and test/cpp/core_test.cpp, so nothing outside this crate
//! is read; the two error documents of its test/data/errors are copied as
//! literals) and against values taken from the sample files with grep and
//! python3 -I (cited where they are used).
//!
//! Needs feature `rows`: `cargo test --all-features`.
#![cfg(feature = "rows")]

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use yorc::model::{AffiliationKind, Root};
use yorc::rows::{Cell, ColType, Section, Tables};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_samples_dir() -> PathBuf {
    crate_root().join("reference/orcid-model/record_3.0/samples/read_samples")
}

fn sample(name: &str) -> Vec<u8> {
    let path = read_samples_dir().join(name);
    fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[cfg(feature = "json")]
fn sample_text(dir: &str, name: &str) -> String {
    let path = crate_root()
        .join("reference/orcid-model/record_3.0/samples")
        .join(dir)
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn fixture() -> Vec<u8> {
    let path = crate_root().join("tests/data/0000-0003-0100-6634.xml");
    fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn tables(xml: &[u8]) -> Tables {
    Tables::from_xml(xml).expect("rows")
}

fn count(t: &Tables, s: Section) -> usize {
    t.get(s).len()
}

/// Cell as text: VARCHAR as is, integers and booleans formatted, NULL as
/// `<null>` (the convention of yorc-duckdb's core_test.cpp).
fn get(t: &Tables, s: Section, row: usize, col: &str) -> String {
    match t.cell(s, row, col) {
        None => format!("<no cell {}.{col}[{row}]>", s.name()),
        Some(Cell::Null) => "<null>".to_string(),
        Some(Cell::Varchar(v)) => v.clone(),
        Some(Cell::Integer(v)) => v.to_string(),
        Some(Cell::Boolean(v)) => v.to_string(),
        Some(other) => format!("{other:?}"),
    }
}

fn column(t: &Tables, s: Section, col: &str) -> Vec<String> {
    (0..count(t, s)).map(|r| get(t, s, r, col)).collect()
}

#[test]
fn every_read_sample_parses() {
    let mut n = 0;
    for entry in fs::read_dir(read_samples_dir()).expect("read_samples dir") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("xml") {
            continue;
        }
        let bytes = fs::read(&path).unwrap();
        let doc =
            yorc::xml::from_slice(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(
            !matches!(doc.root, Root::Unknown(_)),
            "{}: unknown root {}",
            path.display(),
            doc.root_name
        );
        let t = Tables::from_document(&doc, Some("h"));
        for &s in Section::ALL {
            for row in t.get(s) {
                assert_eq!(row.len(), s.columns().len(), "{}", s.name());
            }
        }
        n += 1;
    }
    assert_eq!(n, 53, "number of XML read samples");
}

#[test]
fn schema_matches_yorc_duckdb() {
    let widths: Vec<(&str, usize)> = Section::ALL
        .iter()
        .map(|s| (s.name(), s.columns().len()))
        .collect();
    assert_eq!(
        widths,
        [
            // orcid_sections.test / orcid_api_messages.test: DESCRIBE
            ("works", 32),
            ("fundings", 37),
            ("affiliations", 32),
            ("peer_reviews", 38),
            ("research_resources", 32),
            ("research_resource_items", 13),
            ("contributors", 12),
            ("external_ids", 13),
            ("person", 15),
            ("person_items", 24),
            ("history", 17),
            ("search_results", 13),
            ("errors", 10),
            ("research_resource_hosts", 12),
        ]
    );
    let total: usize = widths.iter().map(|(_, n)| n).sum();
    assert_eq!(total, 300);
    for &s in Section::ALL {
        assert_eq!(Section::from_name(s.name()), Some(s));
        let cols = s.columns();
        // search_results starts with num_found (REFERENCE.md)
        let first = if s == Section::SearchResults {
            "num_found"
        } else {
            "orcid"
        };
        assert_eq!(cols[0].name, first, "{}", s.name());
        assert!(s.column_index("source_hash").is_some(), "{}", s.name());
    }
    assert_eq!(Section::from_name("timeline"), None);
    assert_eq!(Section::from_name("summary"), None);

    fn names(s: Section) -> Vec<&'static str> {
        s.columns().iter().map(|c| c.name).collect()
    }
    // orcid_locked_deprecated.test: string_agg(column_name, ',')
    assert_eq!(
        names(Section::Errors).join(","),
        "orcid,response_code,developer_message,user_message,error_code,more_info,\
         container,position,source_hash,primary_orcid"
    );
    // REFERENCE.md, orcid_search_results
    assert_eq!(
        names(Section::SearchResults).join(","),
        "num_found,ordinal,orcid,orcid_uri,orcid_host,given_names,family_names,\
         credit_name,other_names,emails,institution_names,item,source_hash"
    );
    fn types(s: Section) -> Vec<ColType> {
        s.columns().iter().map(|c| c.ty).collect()
    }
    let (v, i, b) = (ColType::Varchar, ColType::Integer, ColType::Bigint);
    assert_eq!(types(Section::Errors), [v, i, v, v, i, v, v, i, v, v]);
    // orcid_sections.test: string_agg(column_name, ',') of the new section
    assert_eq!(
        names(Section::ResearchResourceHosts).join(","),
        "orcid,owner_put_code,ordinal,host_name,host_city,host_region,host_country,\
         host_disambiguated_id,host_disambiguation_source,container,item_ordinal,source_hash"
    );
    assert_eq!(
        types(Section::ResearchResourceHosts),
        [v, b, i, v, v, v, v, v, v, v, i, v]
    );
    // CROSSCHECK #10: the columns appended after the existing ones, in order
    // (orcid_sections.test checks the same tails)
    let tail = |s: Section, n: usize| -> String {
        let all = names(s);
        all[all.len() - n..].join(",")
    };
    let origin = "assertion_origin_orcid,assertion_origin_client_id";
    for s in [
        Section::Fundings,
        Section::Affiliations,
        Section::PeerReviews,
        Section::ResearchResources,
    ] {
        assert_eq!(tail(s, 2), origin, "{}", s.name());
    }
    assert_eq!(tail(Section::Contributors, 1), "contributor_orcid_host");
    assert_eq!(tail(Section::ExternalIds, 1), "normalized_error_code");
    assert_eq!(tail(Section::Person, 2), "orcid_uri,orcid_host");
    assert_eq!(
        tail(Section::PersonItems, 6),
        "assertion_origin_name,assertion_origin_orcid,assertion_origin_client_id,\
         normalized,normalized_error,normalized_error_code"
    );
    assert_eq!(
        tail(Section::History, 3),
        "assertion_origin_name,assertion_origin_orcid,assertion_origin_client_id"
    );
    // the earlier columns keep their positions (peer_reviews' 36th column)
    assert_eq!(
        Section::PeerReviews.column_index("subject_translated_name_language"),
        Some(35)
    );
    assert_eq!(Section::Fundings.column_index("source_hash"), Some(34));
    assert_eq!(Section::PersonItems.column_index("source_hash"), Some(17));
    assert_eq!(
        types(Section::SearchResults),
        [b, i, v, v, v, v, v, v, v, v, v, v, v]
    );
}

// ----------------------------------------------------------- record-3.0.xml

#[test]
fn record_sample_counts() {
    let t = tables(&sample("record-3.0.xml"));
    let counts: Vec<usize> = Section::ALL.iter().map(|s| count(&t, *s)).collect();
    // orcid_sections.test: 1 1 7 1 1 0 0 16 1 6 1; orcid_api_messages.test:
    // no errors in record-3.0.xml (and no search results); one host in the
    // research-resource-summary's proposal (record-3.0.xml:639-652)
    assert_eq!(counts, [1, 1, 7, 1, 1, 0, 0, 16, 1, 6, 1, 0, 0, 1]);
}

#[test]
fn record_sample_values() {
    let xml = sample("record-3.0.xml");
    let t = tables(&xml);

    let kinds: BTreeSet<String> = column(&t, Section::Affiliations, "affiliation_type")
        .into_iter()
        .collect();
    let want: BTreeSet<String> = [
        "distinction",
        "education",
        "employment",
        "invited-position",
        "membership",
        "qualification",
        "service",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(kinds, want);
    for o in column(&t, Section::Affiliations, "orcid") {
        assert_eq!(o, "8888-8888-8888-8880");
    }

    let p = Section::Person;
    assert_eq!(get(&t, p, 0, "given_names"), "give-names");
    assert_eq!(get(&t, p, 0, "family_name"), "family-name");
    assert_eq!(get(&t, p, 0, "credit_name"), "credit-name");
    assert_eq!(get(&t, p, 0, "biography"), "biography");
    assert_eq!(get(&t, p, 0, "name_visibility"), "public");

    let items: BTreeSet<String> = (0..count(&t, Section::PersonItems))
        .map(|r| {
            format!(
                "{}={}",
                get(&t, Section::PersonItems, r, "kind"),
                get(&t, Section::PersonItems, r, "value")
            )
        })
        .collect();
    let want: BTreeSet<String> = [
        "address=US",
        "email=user1@email.com",
        "external-identifier=value-1",
        "keyword=keyword1",
        "other-name=other-name-1",
        "researcher-url=url-name-1",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(items, want);
    let ext = (0..count(&t, Section::PersonItems))
        .find(|r| get(&t, Section::PersonItems, *r, "kind") == "external-identifier")
        .unwrap();
    assert_eq!(
        get(&t, Section::PersonItems, ext, "url"),
        "http://url.com/1"
    );
    assert_eq!(get(&t, Section::PersonItems, ext, "type"), "type-1");

    let h = Section::History;
    assert_eq!(get(&t, h, 0, "creation_method"), "API");
    assert_eq!(get(&t, h, 0, "claimed"), "true");
    assert_eq!(get(&t, h, 0, "visibility"), "private");

    // record-3.0.xml:20-24, common:orcid-identifier of the record
    assert_eq!(
        get(&t, p, 0, "orcid_uri"),
        "https://orcid.org/8888-8888-8888-8880"
    );
    assert_eq!(get(&t, p, 0, "orcid_host"), "orcid.org");
    // no assertion origin anywhere in record-3.0.xml (grep -c assertion-origin: 0)
    for s in [
        Section::Affiliations,
        Section::PersonItems,
        Section::History,
    ] {
        for c in ["assertion_origin_name", "assertion_origin_orcid"] {
            assert!(
                column(&t, s, c).iter().all(|v| v == "<null>"),
                "{}",
                s.name()
            );
        }
    }
    // the one host of the research-resource-summary (record-3.0.xml:639-652)
    let rh = Section::ResearchResourceHosts;
    let row: Vec<String> = Section::ResearchResourceHosts
        .columns()
        .iter()
        .filter(|c| c.name != "source_hash")
        .map(|c| get(&t, rh, 0, c.name))
        .collect();
    assert_eq!(
        row,
        [
            "8888-8888-8888-8880",
            "1234",
            "1",
            "XSEDE",
            "city",
            "region",
            "US",
            "XX",
            "grid",
            "proposal",
            "<null>",
        ]
    );

    // The research resource summary (timeline expectation in core_test.cpp:
    // modified 2017-09-08T13:31:19.987Z, created 2015-06-25).
    let r = Section::ResearchResources;
    assert_eq!(get(&t, r, 0, "last_modified"), "2017-09-08T13:31:19.987Z");
    assert_eq!(get(&t, r, 0, "created"), "2015-06-25T16:01:12.718Z");
    assert_eq!(get(&t, r, 0, "title"), "Giant Laser Award");
    assert_eq!(get(&t, r, 0, "orcid"), "8888-8888-8888-8880");
    assert_eq!(get(&t, r, 0, "item"), "research-resource-summary");

    // the employment's label in core_test.cpp's timeline check
    let emp = (0..count(&t, Section::Affiliations))
        .find(|r| get(&t, Section::Affiliations, *r, "affiliation_type") == "employment")
        .unwrap();
    assert_eq!(
        get(&t, Section::Affiliations, emp, "role_title"),
        "employment:role-title"
    );
    assert_eq!(get(&t, Section::Affiliations, emp, "end_year"), "2025");
    assert_eq!(get(&t, Section::Affiliations, emp, "end_month"), "<null>");

    // Same facts through the typed model.
    let doc = yorc::xml::from_slice(&xml).unwrap();
    assert_eq!(doc.orcid(), Some("8888-8888-8888-8880"));
    let record = doc.record().unwrap();
    let rr = &record
        .activities_summary
        .as_ref()
        .unwrap()
        .research_resources
        .as_ref()
        .unwrap()
        .groups[0]
        .summaries[0];
    assert_eq!(
        rr.meta.last_modified_date.as_deref(),
        Some("2017-09-08T13:31:19.987Z")
    );
    assert_eq!(rr.meta.put_code, Some(1234));
    let acts = doc.activities_summary().unwrap();
    for kind in AffiliationKind::ALL {
        let list = acts.affiliations(kind).unwrap();
        assert_eq!(list.groups.len(), 1, "{kind:?}");
        assert_eq!(list.groups[0].summaries[0].kind, kind);
    }
    assert_eq!(
        record.preferences.as_ref().unwrap().locale.as_deref(),
        Some("zh_CN")
    );
}

// --------------------------------------------------------- other samples

#[test]
fn work_full_sample() {
    let t = tables(&sample("work-full-3.0.xml"));
    let w = Section::Works;
    assert_eq!(count(&t, w), 1);
    assert_eq!(get(&t, w, 0, "item"), "work");
    assert_eq!(get(&t, w, 0, "put_code"), "123");
    assert_eq!(get(&t, w, 0, "orcid"), "<null>");
    assert_eq!(get(&t, w, 0, "group_index"), "<null>");
    assert_eq!(get(&t, w, 0, "translated_title"), "common:translated-title");
    assert_eq!(get(&t, w, 0, "translated_title_language"), "en");
    assert_eq!(get(&t, w, 0, "citation_type"), "formatted-unspecified");
    assert_eq!(get(&t, w, 0, "citation_value"), "work:citation");
    assert_eq!(get(&t, w, 0, "short_description"), "work:short-description");
    assert_eq!(get(&t, w, 0, "language_code"), "en");
    assert_eq!(get(&t, w, 0, "country"), "AF");
    assert_eq!(get(&t, w, 0, "doi"), "work:doi");
    assert_eq!(get(&t, w, 0, "pub_year"), "1948");
    assert_eq!(get(&t, w, 0, "pub_month"), "2");
    assert_eq!(get(&t, w, 0, "source_name"), "<null>");

    let c = Section::Contributors;
    assert_eq!(count(&t, c), 1);
    assert_eq!(get(&t, c, 0, "owner"), "work");
    assert_eq!(get(&t, c, 0, "owner_put_code"), "123");
    assert_eq!(get(&t, c, 0, "ordinal"), "1");
    assert_eq!(get(&t, c, 0, "contributor_orcid"), "8888-8888-8888-8880");
    // read_samples/work-full-3.0.xml:53 <common:host>orcid.org</common:host>
    assert_eq!(get(&t, c, 0, "contributor_orcid_host"), "orcid.org");
    assert_eq!(get(&t, c, 0, "credit_name"), "work:credit-name");
    assert_eq!(get(&t, c, 0, "email"), "work@contributor.email");
    assert_eq!(get(&t, c, 0, "sequence"), "first");
    assert_eq!(get(&t, c, 0, "role"), "author");

    let e = Section::ExternalIds;
    assert_eq!(count(&t, e), 2);
    assert_eq!(get(&t, e, 0, "type"), "agr");
    assert_eq!(get(&t, e, 0, "value"), "work:external-identifier-id");
    assert_eq!(get(&t, e, 0, "relationship"), "version-of");
    assert_eq!(get(&t, e, 1, "type"), "doi");
    assert_eq!(get(&t, e, 1, "value"), "work:doi");
    assert_eq!(get(&t, e, 1, "relationship"), "self");
    assert_eq!(get(&t, e, 1, "owner"), "work");
    assert_eq!(get(&t, e, 1, "container"), "external-ids");
}

#[test]
fn research_resource_sample() {
    let t = tables(&sample("research-resource-3.0.xml"));
    let r = Section::ResearchResources;
    assert_eq!(count(&t, r), 1);
    assert_eq!(get(&t, r, 0, "orcid"), "0000-0003-0902-4386");
    assert_eq!(get(&t, r, 0, "title"), "Giant Laser Award");
    assert_eq!(get(&t, r, 0, "translated_title_language"), "de");
    assert_eq!(get(&t, r, 0, "host_name"), "XSEDE"); // first host wins
    assert_eq!(get(&t, r, 0, "start_year"), "1999");
    assert_eq!(get(&t, r, 0, "end_year"), "2012");
    assert_eq!(get(&t, r, 0, "url"), "http://xsede.org/GiantLaserAward");

    let i = Section::ResearchResourceItems;
    assert_eq!(count(&t, i), 2);
    assert_eq!(get(&t, i, 0, "owner_put_code"), "1234");
    assert_eq!(get(&t, i, 0, "ordinal"), "1");
    assert_eq!(get(&t, i, 0, "resource_name"), "Giant Laser 1");
    assert_eq!(get(&t, i, 0, "resource_type"), "infrastructures");
    assert_eq!(get(&t, i, 0, "host_name"), "Lasers-R-US");
    assert_eq!(get(&t, i, 0, "url"), "http://blah.com");
    assert_eq!(get(&t, i, 1, "ordinal"), "2");
    assert_eq!(get(&t, i, 1, "resource_name"), "Moon Targets");
    assert_eq!(get(&t, i, 1, "host_name"), "Moon Holdings Ltd");
    assert_eq!(get(&t, i, 1, "url"), "http://blah2.com");

    assert_eq!(count(&t, Section::ExternalIds), 5);

    // Every host (CROSSCHECK #9): research-resource-3.0.xml has two proposal
    // hosts (lines 24-49) and one per resource item (lines 82-95, 114-127);
    // the first-host columns above keep XSEDE and Lasers-R-US.
    let h = Section::ResearchResourceHosts;
    let cols = [
        "orcid",
        "owner_put_code",
        "ordinal",
        "host_name",
        "host_disambiguation_source",
        "container",
        "item_ordinal",
    ];
    let rows: Vec<Vec<String>> = (0..count(&t, h))
        .map(|r| cols.iter().map(|c| get(&t, h, r, c)).collect())
        .collect();
    let id = "0000-0003-0902-4386";
    assert_eq!(
        rows,
        [
            [id, "1234", "1", "XSEDE", "grid", "proposal", "<null>"],
            [id, "1234", "2", "Lasers-R-Us", "lei", "proposal", "<null>"],
            [id, "1234", "1", "Lasers-R-US", "grid", "resource-item", "1"],
            [
                id,
                "1234",
                "1",
                "Moon Holdings Ltd",
                "lei",
                "resource-item",
                "2"
            ],
        ]
    );
    for row in 0..4 {
        assert_eq!(get(&t, h, row, "host_city"), "city");
        assert_eq!(get(&t, h, row, "host_region"), "region");
        assert_eq!(get(&t, h, row, "host_country"), "US");
        assert_eq!(get(&t, h, row, "host_disambiguated_id"), "XX");
    }
    // research_resources gains the assertion origin columns; the sample's
    // source has none
    assert_eq!(get(&t, r, 0, "assertion_origin_orcid"), "<null>");

    // The summaries of research-resources-3.0.xml: 1234 has two hosts, 1235
    // one (lines 41-68 and 116-130); the iD is the root path's
    // (/8888-8888-8888-8880/research-resources); trailing whitespace of
    // "XX\n\t..." is trimmed.
    let t = tables(&sample("research-resources-3.0.xml"));
    let rows: Vec<Vec<String>> = (0..count(&t, h))
        .map(|r| cols.iter().map(|c| get(&t, h, r, c)).collect())
        .collect();
    let id = "8888-8888-8888-8880";
    assert_eq!(
        rows,
        [
            [id, "1234", "1", "XSEDE", "grid", "proposal", "<null>"],
            [id, "1234", "2", "Lasers-R-Us", "lei", "proposal", "<null>"],
            [id, "1235", "1", "XSEDE", "grid", "proposal", "<null>"],
        ]
    );
    assert_eq!(get(&t, h, 0, "host_disambiguated_id"), "XX");
    assert_eq!(count(&t, Section::ResearchResourceItems), 0);
}

#[test]
fn peer_review_full_sample() {
    let t = tables(&sample("peer-review-full-3.0.xml"));
    let p = Section::PeerReviews;
    assert_eq!(count(&t, p), 1);
    assert_eq!(get(&t, p, 0, "put_code"), "12345");
    assert_eq!(get(&t, p, 0, "completion_year"), "1948");
    assert_eq!(get(&t, p, 0, "review_group_id"), "orcid-generated:12345");
    assert_eq!(
        get(&t, p, 0, "subject_translated_name"),
        "peer-review:subject-translated-name"
    );
    // read_samples/peer-review-full-3.0.xml:44 language-code="en"
    assert_eq!(get(&t, p, 0, "subject_translated_name_language"), "en");
    assert_eq!(get(&t, p, 0, "subject_id_type"), "doi");
    assert_eq!(
        get(&t, p, 0, "convening_org_disambiguation_source"),
        "FUNDREF"
    );

    let e = Section::ExternalIds;
    assert_eq!(count(&t, e), 2);
    assert_eq!(get(&t, e, 0, "container"), "review-identifiers");
    assert_eq!(get(&t, e, 0, "type"), "source-work-id");
    assert_eq!(get(&t, e, 1, "container"), "peer-review");
    assert_eq!(get(&t, e, 1, "type"), "doi");
}

#[test]
fn collections_and_single_documents() {
    assert_eq!(count(&tables(&sample("works-3.0.xml")), Section::Works), 3);

    let t = tables(&sample("employments-3.0.xml"));
    assert_eq!(count(&t, Section::Affiliations), 2);
    assert_eq!(get(&t, Section::Affiliations, 1, "group_index"), "2");

    assert_eq!(
        count(&tables(&sample("person-3.0.xml")), Section::PersonItems),
        6
    );
    assert_eq!(
        count(&tables(&sample("bulk-work-err.xml")), Section::Works),
        1
    );

    let t = tables(&sample("history-3.0.xml"));
    let h = Section::History;
    assert_eq!(count(&t, h), 1);
    assert_eq!(get(&t, h, 0, "creation_method"), "API");
    assert_eq!(get(&t, h, 0, "claimed"), "true");
    assert_eq!(get(&t, h, 0, "verified_email"), "true");
    assert_eq!(get(&t, h, 0, "visibility"), "private");

    let doc = yorc::xml::from_slice(&sample("bulk-work-err.xml")).unwrap();
    let Root::Bulk(bulk) = &doc.root else {
        panic!("not bulk");
    };
    assert_eq!(bulk.items.len(), 2);
    match &bulk.items[1] {
        yorc::model::BulkItem::Error(e) => {
            assert_eq!(e.response_code, Some(400));
            assert_eq!(e.error_code, Some(9037));
        }
        other => panic!("expected an error item, got {other:?}"),
    }
}

/// The literal of yorc-duckdb/test/sql/orcid_works_xml.test: entities,
/// numeric references, CDATA, self-closing elements, orcid-identifier fallback.
#[test]
fn works_xml_edge_literal() {
    let xml = r#"<?xml version="1.0"?>
<!-- comment -->
<record:record xmlns:record="r" xmlns:common="c" xmlns:activities="a" xmlns:work="w">
  <common:orcid-identifier><common:path>0000-0002-1825-0097</common:path></common:orcid-identifier>
  <activities:activities-summary>
    <activities:works>
      <activities:group>
        <work:work-summary put-code="7" visibility="public">
          <work:title><common:title>Fish &amp; Chips &#x2014; &#233;t&#xE9; &lt;3</common:title></work:title>
          <common:external-ids>
            <common:external-id>
              <common:external-id-type>doi</common:external-id-type>
              <common:external-id-value>10.1000/ABC</common:external-id-value>
              <common:external-id-relationship>part-of</common:external-id-relationship>
            </common:external-id>
            <common:external-id>
              <common:external-id-type>doi</common:external-id-type>
              <common:external-id-value>10.1000/XYZ</common:external-id-value>
              <common:external-id-relationship>self</common:external-id-relationship>
            </common:external-id>
          </common:external-ids>
          <common:url/>
          <work:type>book</work:type>
          <common:publication-date><common:year>1999</common:year><common:month>xx</common:month></common:publication-date>
          <work:journal-title><![CDATA[A <raw> & journal]]></work:journal-title>
        </work:work-summary>
      </activities:group>
    </activities:works>
  </activities:activities-summary>
</record:record>"#;
    let t = tables(xml.as_bytes());
    let w = Section::Works;
    assert_eq!(count(&t, w), 1);
    assert_eq!(get(&t, w, 0, "orcid"), "0000-0002-1825-0097");
    assert_eq!(get(&t, w, 0, "put_code"), "7");
    assert_eq!(get(&t, w, 0, "group_index"), "1");
    assert_eq!(
        get(&t, w, 0, "title"),
        "Fish & Chips \u{2014} \u{e9}t\u{e9} <3"
    );
    assert_eq!(get(&t, w, 0, "journal"), "A <raw> & journal");
    assert_eq!(get(&t, w, 0, "doi"), "10.1000/xyz");
    assert_eq!(get(&t, w, 0, "pub_year"), "1999");
    assert_eq!(get(&t, w, 0, "pub_month"), "<null>");
    assert_eq!(get(&t, w, 0, "url"), "<null>");

    // records without works yield no rows; malformed XML is an error
    let t = tables(br#"<record:record path="/0000-0002-1825-0097"/>"#);
    assert_eq!(count(&t, w), 0);
    assert!(Tables::from_xml(b"<a><b></a>").is_err());
}

// ------------------------------------------------- 0000-0003-0100-6634.xml

#[test]
fn fixture_work_summaries() {
    let xml = fixture();
    let doc = yorc::xml::from_slice(&xml).unwrap();
    assert_eq!(doc.orcid(), Some("0000-0003-0100-6634"));
    assert_eq!(
        doc.activities_summary().unwrap().work_summaries().count(),
        382
    );

    let t = Tables::from_document(
        &doc,
        Some("494662244addfd21664a3a56f222a88614b83f61084888ac6162eeb970324801"),
    );
    let w = Section::Works;
    // orcid_works.test
    assert_eq!(count(&t, w), 382);
    let distinct = |col: &str| -> usize {
        column(&t, w, col)
            .into_iter()
            .filter(|v| v != "<null>")
            .collect::<BTreeSet<_>>()
            .len()
    };
    let non_null = |col: &str| -> usize {
        column(&t, w, col)
            .into_iter()
            .filter(|v| v != "<null>")
            .count()
    };
    assert_eq!(distinct("group_index"), 155);
    assert_eq!(distinct("orcid"), 1);
    let put_code_sum: i64 = (0..382)
        .map(|r| t.cell(w, r, "put_code").and_then(|c| c.as_i64()).unwrap())
        .sum();
    assert_eq!(put_code_sum, 36_003_218_443);
    assert_eq!(non_null("subtitle"), 0);
    assert_eq!(non_null("journal"), 371);
    assert_eq!(non_null("pub_month"), 334);
    assert_eq!(non_null("pub_day"), 298);
    assert_eq!(non_null("doi"), 311);
    assert_eq!(distinct("doi"), 226);
    assert_eq!(non_null("url"), 84);
    let years: Vec<i64> = (0..382)
        .filter_map(|r| t.cell(w, r, "pub_year").and_then(|c| c.as_i64()))
        .collect();
    assert_eq!(years.iter().min(), Some(&2002));
    assert_eq!(years.iter().max(), Some(&2025));
    for r in 0..382 {
        assert_eq!(get(&t, w, r, "item"), "work-summary");
    }

    let row = (0..382)
        .find(|r| get(&t, w, *r, "put_code") == "182082411")
        .unwrap();
    let expect = [
        ("orcid", "0000-0003-0100-6634"),
        ("group_index", "1"),
        ("display_index", "1"),
        ("title", "mgbilby/oga_src: initial oga_src release"),
        ("subtitle", "<null>"),
        ("journal", "Zenodo"),
        ("type", "software"),
        ("pub_year", "2025"),
        ("pub_month", "4"),
        ("pub_day", "11"),
        ("doi", "10.5281/zenodo.15199545"),
        ("url", "<null>"),
        ("source_name", "DataCite"),
        ("visibility", "public"),
        ("created", "2025-04-11T18:39:27.813Z"),
        ("last_modified", "2025-04-11T18:39:27.813Z"),
        (
            "source_hash",
            "494662244addfd21664a3a56f222a88614b83f61084888ac6162eeb970324801",
        ),
        ("translated_title", "<null>"),
        ("citation_type", "<null>"),
        ("source_client_id", "0000-0001-8099-6984"),
        ("item", "work-summary"),
        ("path", "/0000-0003-0100-6634/work/182082411"),
    ];
    for (col, want) in expect {
        assert_eq!(get(&t, w, row, col), want, "{col}");
    }
    // core_test.cpp: the first row's title and source client
    assert_eq!(
        get(&t, w, 0, "title"),
        "mgbilby/oga_src: initial oga_src release"
    );
    assert_eq!(get(&t, w, 0, "source_client_id"), "0000-0001-8099-6984");

    let row = (0..382)
        .find(|r| get(&t, w, *r, "put_code") == "72106251")
        .unwrap();
    assert_eq!(get(&t, w, row, "group_index"), "155");
    assert_eq!(get(&t, w, row, "display_index"), "0");
    assert_eq!(get(&t, w, row, "type"), "dissertation-thesis");
    assert_eq!(get(&t, w, row, "pub_year"), "2002");
    assert_eq!(get(&t, w, row, "doi"), "10.5281/zenodo.3752256");

    let types = column(&t, w, "type");
    let n = |ty: &str| types.iter().filter(|v| v.as_str() == ty).count();
    assert_eq!((n("other"), n("book"), n("journal-article")), (94, 85, 48));
    let titles = column(&t, w, "title");
    assert_eq!(
        titles
            .iter()
            .filter(
                |v| v.as_str() == "BeDuhn\u{2019}s Greek Reconstruction of Marcion\u{2019}s Gospel"
            )
            .count(),
        3
    );
}

#[test]
fn fixture_source_hash_and_other_sections() {
    let xml = fixture();
    let t = tables(&xml);
    // orcid_works_xml.test: min(source_hash) over the file's rows
    assert_eq!(
        get(&t, Section::Works, 0, "source_hash"),
        "494662244addfd21664a3a56f222a88614b83f61084888ac6162eeb970324801"
    );
    // orcid_sections.test: person 1, history 1, external ids > 0. The other
    // counts are element counts in the file (grep):
    //   affiliations: 7 distinction + 9 education + 18 employment
    //     + 16 membership + 29 service summaries = 79
    //   fundings: 7 funding-summary; peer reviews: 2 peer-review-summary
    //   external_ids: 864 <common:external-id> (all under external-ids)
    //   person_items: 7 other-name + 20 researcher-url + 1 address
    //     + 20 keyword + 1 external-identifier (no emails) = 49
    let counts: Vec<usize> = Section::ALL.iter().map(|s| count(&t, *s)).collect();
    assert_eq!(counts, [382, 7, 79, 2, 0, 0, 0, 864, 1, 49, 1, 0, 0, 0]);
    for &s in Section::ALL {
        for r in 0..count(&t, s) {
            assert_eq!(
                get(&t, s, r, "orcid"),
                "0000-0003-0100-6634",
                "{}",
                s.name()
            );
        }
    }
}

// ------------------------------------------- search_results and errors

#[test]
fn search_sample() {
    // orcid_api_messages.test, orcid_recipes.test
    let t = tables(&sample("search-3.0.xml"));
    let s = Section::SearchResults;
    assert_eq!(count(&t, s), 1);
    let row: Vec<String> = [
        "num_found",
        "ordinal",
        "orcid",
        "orcid_uri",
        "orcid_host",
        "given_names",
        "item",
    ]
    .iter()
    .map(|c| get(&t, s, 0, c))
    .collect();
    assert_eq!(
        row,
        [
            "1",
            "1",
            "8888-8888-8888-8880",
            "https://orcid.org/8888-8888-8888-8880",
            "orcid.org",
            "<null>",
            "result",
        ]
    );
    assert_eq!(count(&t, Section::Errors), 0);
}

#[test]
fn expanded_search_literal() {
    // orcid_api_messages.test (no expanded-search sample is vendored)
    let xml = r#"<es:expanded-search num-found="7" xmlns:es="http://www.orcid.org/ns/expanded-search"><es:expanded-result><es:orcid-id>0000-0002-1825-0097</es:orcid-id><es:given-names>Given</es:given-names><es:family-names>Family</es:family-names><es:credit-name>G. Family</es:credit-name><es:other-name>Other A</es:other-name><es:other-name>Other B</es:other-name><es:email>a@example.org</es:email><es:institution-name>Inst A</es:institution-name><es:institution-name>Inst B</es:institution-name></es:expanded-result><es:expanded-result><es:orcid-id>0000-0003-0100-6634</es:orcid-id></es:expanded-result></es:expanded-search>"#;
    let t = tables(xml.as_bytes());
    let s = Section::SearchResults;
    let cols = [
        "num_found",
        "ordinal",
        "orcid",
        "orcid_uri",
        "given_names",
        "family_names",
        "credit_name",
        "other_names",
        "emails",
        "institution_names",
        "item",
    ];
    let rows: Vec<Vec<String>> = (0..count(&t, s))
        .map(|r| cols.iter().map(|c| get(&t, s, r, c)).collect())
        .collect();
    assert_eq!(
        rows,
        [
            vec![
                "7",
                "1",
                "0000-0002-1825-0097",
                "<null>",
                "Given",
                "Family",
                "G. Family",
                "Other A; Other B",
                "a@example.org",
                "Inst A; Inst B",
                "expanded-result",
            ],
            vec![
                "7",
                "2",
                "0000-0003-0100-6634",
                "<null>",
                "<null>",
                "<null>",
                "<null>",
                "<null>",
                "<null>",
                "<null>",
                "expanded-result",
            ],
        ]
    );
}

#[test]
fn error_sample() {
    // orcid_api_messages.test
    let t = tables(&sample("error-3.0.xml"));
    let e = Section::Errors;
    assert_eq!(count(&t, e), 1);
    let cols = [
        "orcid",
        "response_code",
        "developer_message",
        "user_message",
        "error_code",
        "more_info",
        "container",
        "position",
        "primary_orcid",
    ];
    let row: Vec<String> = cols.iter().map(|c| get(&t, e, 0, c)).collect();
    assert_eq!(
        row,
        [
            "<null>",
            "0",
            "error:developer-message",
            "error:user-message",
            "0",
            "http://tempuri.org",
            "<null>",
            "1",
            "<null>",
        ]
    );
    assert_eq!(get(&t, e, 0, "source_hash").len(), 64);
}

#[test]
fn bulk_error_sample() {
    // orcid_api_messages.test, orcid_recipes.test, orcid_locked_deprecated.test
    let t = tables(&sample("bulk-work-err.xml"));
    let e = Section::Errors;
    assert_eq!(count(&t, e), 1);
    let dev = get(&t, e, 0, "developer_message");
    assert!(dev.starts_with("Invalid work type, valid values are:"));
    let cols = [
        "response_code",
        "user_message",
        "error_code",
        "more_info",
        "container",
        "position",
        "orcid",
        "primary_orcid",
    ];
    let row: Vec<String> = cols.iter().map(|c| get(&t, e, 0, c)).collect();
    assert_eq!(
        row,
        [
            "400",
            "Please specify a work type.",
            "9037",
            "http://members.orcid.org/api/api-error-codes",
            "bulk",
            "2",
            "<null>",
            "<null>",
        ]
    );
    // the work of the same response is still a works row
    assert_eq!(count(&t, Section::Works), 1);
}

/// yorc-duckdb's test/data/errors/0000-0001-5272-5001_locked.xml (only the
/// `error` namespace declaration kept).
const LOCKED: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<error:error xmlns:error="http://www.orcid.org/ns/error">
    <error:response-code>409</error:response-code>
    <error:developer-message>409 Conflict: The ORCID record is locked and cannot be edited. ORCID https://orcid.org/0000-0001-5272-5001</error:developer-message>
    <error:user-message>The ORCID record is locked.</error:user-message>
    <error:error-code>9018</error:error-code>
    <error:more-info>https://members.orcid.org/api/resources/troubleshooting</error:more-info>
</error:error>
"#;

/// yorc-duckdb's test/data/errors/0000-0001-5433-3001_deprecated.xml (only
/// the `error` namespace declaration kept).
const DEPRECATED: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<error:error xmlns:error="http://www.orcid.org/ns/error">
    <error:response-code>301</error:response-code>
    <error:developer-message>301 Moved Permanently: This account is deprecated. Please refer to account: https://orcid.org/0009-0008-8963-1314. ORCID https://orcid.org/0000-0001-5433-3001</error:developer-message>
    <error:user-message>This account is deprecated. Please refer to account: https://orcid.org/0009-0008-8963-1314.</error:user-message>
    <error:error-code>9007</error:error-code>
    <error:more-info>https://members.orcid.org/api/resources/troubleshooting</error:more-info>
</error:error>
"#;

#[test]
fn locked_and_deprecated_errors() {
    // orcid_locked_deprecated.test, orcid_recipes.test
    let e = Section::Errors;
    let cols = [
        "orcid",
        "response_code",
        "error_code",
        "user_message",
        "container",
        "primary_orcid",
    ];
    let t = tables(LOCKED.as_bytes());
    assert_eq!(count(&t, e), 1);
    let row: Vec<String> = cols.iter().map(|c| get(&t, e, 0, c)).collect();
    assert_eq!(
        row,
        [
            "0000-0001-5272-5001",
            "409",
            "9018",
            "The ORCID record is locked.",
            "<null>",
            "<null>",
        ]
    );
    let t = tables(DEPRECATED.as_bytes());
    assert_eq!(count(&t, e), 1);
    let row: Vec<String> = cols.iter().map(|c| get(&t, e, 0, c)).collect();
    assert_eq!(
        row,
        [
            "0000-0001-5433-3001",
            "301",
            "9007",
            "This account is deprecated. Please refer to account: https://orcid.org/0009-0008-8963-1314.",
            "<null>",
            "0009-0008-8963-1314",
        ]
    );
    // neither document is a record: no person, history or works rows
    for t in [tables(LOCKED.as_bytes()), tables(DEPRECATED.as_bytes())] {
        for s in [Section::Person, Section::History, Section::Works] {
            assert_eq!(count(&t, s), 0, "{}", s.name());
        }
    }
    // a root path wins over the iD the message names
    let with_path = LOCKED.replace(
        "<error:error xmlns",
        r#"<error:error path="/0000-0002-1825-0097" xmlns"#,
    );
    let t = tables(with_path.as_bytes());
    assert_eq!(get(&t, e, 0, "orcid"), "0000-0002-1825-0097");
}

#[test]
fn api_message_items_only_at_the_document_element() {
    // orcid_activity_items.test: elements named like API messages inside a
    // work are not items
    let xml = r#"<work:work xmlns:work="w" xmlns:x="x" put-code="1" path="/0000-0002-1825-0097/work/1"><x:person><x:name><x:given-names>No</x:given-names></x:name></x:person><x:history><x:claimed>true</x:claimed></x:history><x:record><x:person/></x:record><x:error><x:response-code>400</x:response-code></x:error><x:notification/><x:record-summary/><x:search><x:result/></x:search><x:group-id-record put-code="2"/></work:work>"#;
    let t = tables(xml.as_bytes());
    let counts: Vec<usize> = [
        Section::Person,
        Section::History,
        Section::Errors,
        Section::SearchResults,
        Section::Works,
    ]
    .iter()
    .map(|s| count(&t, *s))
    .collect();
    assert_eq!(counts, [0, 0, 0, 0, 1]);
}

#[cfg(feature = "json")]
#[test]
fn api_messages_from_json() {
    // orcid_json.test
    let rows = |kind: &str, json: &str| -> Tables {
        let doc = yorc::json::from_str(kind, json).expect("json");
        Tables::from_document(&doc, None)
    };
    let t = rows(
        "error",
        r#"{"response-code": 0, "developer-message": "error:developer-message", "user-message": "error:user-message", "error-code": 0, "more-info": "http://tempuri.org"}"#,
    );
    let xml = tables(&sample("error-3.0.xml"));
    let e = Section::Errors;
    for c in Section::Errors.columns() {
        if c.name != "source_hash" {
            assert_eq!(get(&t, e, 0, c.name), get(&xml, e, 0, c.name), "{}", c.name);
        }
    }

    let t = rows(
        "search",
        r#"{"result": [{"orcid-identifier": {"uri": "https://orcid.org/8888-8888-8888-8880", "path": "8888-8888-8888-8880", "host": "orcid.org"}}], "num-found": 1}"#,
    );
    let s = Section::SearchResults;
    let row: Vec<String> = ["num_found", "ordinal", "orcid", "orcid_uri", "item"]
        .iter()
        .map(|c| get(&t, s, 0, c))
        .collect();
    assert_eq!(
        row,
        [
            "1",
            "1",
            "8888-8888-8888-8880",
            "https://orcid.org/8888-8888-8888-8880",
            "result",
        ]
    );

    let t = rows(
        "bulk",
        r#"{"bulk": [{"work": {"put-code": 156043, "title": {"title": {"value": "W"}}, "type": "journal-article"}}, {"error": {"response-code": 400, "developer-message": "Invalid work type", "user-message": "Please specify a work type.", "error-code": 9037, "more-info": "http://members.orcid.org/api/api-error-codes"}}]}"#,
    );
    let row: Vec<String> = ["container", "position", "response_code", "error_code"]
        .iter()
        .map(|c| get(&t, e, 0, c))
        .collect();
    assert_eq!(row, ["bulk", "2", "400", "9037"]);
}

// ------------------------------------- CROSSCHECK #9 and #10 in ORCID's JSON

/// read_samples/record-full-3.0.json carries the assertion origin in person
/// items and in activity summaries, a normalization error and a sandbox
/// orcid-identifier. Expected values from python3 -I over the file
/// (json.load; source/assertion-origin-orcid/path per item, by put-code).
#[cfg(feature = "json")]
#[test]
fn record_full_json_context_columns() {
    let text = sample_text("read_samples", "record-full-3.0.json");
    let doc = yorc::json::from_str("record", &text).expect("record-full-3.0.json");
    let t = Tables::from_document(&doc, None);
    let me = "0000-0002-7319-2192";
    let asserted = |s: Section| -> Vec<String> {
        (0..count(&t, s))
            .filter(|r| get(&t, s, *r, "assertion_origin_orcid") == me)
            .map(|r| {
                let kind = match s {
                    Section::PersonItems => format!("{}:", get(&t, s, r, "kind")),
                    _ => String::new(),
                };
                format!("{kind}{}", get(&t, s, r, "put_code"))
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    // funding-summary 5909 (of 5132, 5909)
    assert_eq!(asserted(Section::Fundings), ["5909"]);
    // one per affiliation type but services' 7093 and educations' 6455, 8718
    assert_eq!(
        asserted(Section::Affiliations),
        ["9264", "9265", "9266", "9267", "9268", "9269", "9270"]
    );
    // peer-review-summary 2389 (of 1961, 2389), research-resource-summary 1498,
    // work-summary 178602
    assert_eq!(asserted(Section::PeerReviews), ["2389"]);
    assert_eq!(asserted(Section::ResearchResources), ["1498"]);
    assert_eq!(asserted(Section::Works), ["178602"]);
    // person items: other-name 5813, address 3527, keyword 5097, external
    // identifier 3995
    assert_eq!(
        asserted(Section::PersonItems),
        [
            "address:3527",
            "external-identifier:3995",
            "keyword:5097",
            "other-name:5813"
        ]
    );
    // the name beside the iD; no assertion-origin-client-id in any sample
    // (grep -c assertion-origin-client-id: 0)
    for s in [
        Section::Fundings,
        Section::Affiliations,
        Section::PeerReviews,
        Section::ResearchResources,
        Section::PersonItems,
        Section::History,
    ] {
        for r in 0..count(&t, s) {
            let client = get(&t, s, r, "assertion_origin_client_id");
            assert_eq!(client, "<null>", "{}", s.name());
            let name = get(&t, s, r, "assertion_origin_name");
            let orcid = get(&t, s, r, "assertion_origin_orcid");
            if orcid == me {
                assert_eq!(name, "Three releasecandidate1", "{}", s.name());
            } else {
                let both = (name.as_str(), orcid.as_str());
                assert_eq!(both, ("<null>", "<null>"), "{}", s.name());
            }
        }
    }
    // history: no source in the sample
    assert_eq!(
        get(&t, Section::History, 0, "assertion_origin_name"),
        "<null>"
    );

    // works/group[0]: external-id[1] has external-id-normalized-error
    // {"error-code": "8001", ...}, at group level and in the work summary
    let e = Section::ExternalIds;
    let codes: Vec<(String, String)> = (0..count(&t, e))
        .filter(|r| get(&t, e, *r, "normalized_error_code") != "<null>")
        .map(|r| {
            let code = get(&t, e, r, "normalized_error_code");
            (code, get(&t, e, r, "normalized_error"))
        })
        .collect();
    let msg = "Cannot normalize identifier value doi:work:doi".to_string();
    assert_eq!(
        codes,
        [("8001".to_string(), msg.clone()), ("8001".to_string(), msg)]
    );
    // the person external identifier 3995 has no normalized form
    let pi = Section::PersonItems;
    let ext = (0..count(&t, pi))
        .find(|r| get(&t, pi, *r, "kind") == "external-identifier")
        .unwrap();
    for c in ["normalized", "normalized_error", "normalized_error_code"] {
        assert_eq!(get(&t, pi, ext, c), "<null>", "{c}");
    }

    // /orcid-identifier: {"uri": "https://sandbox.orcid.org/0000-0002-7319-2192",
    // "path": ..., "host": "sandbox.orcid.org"}
    assert_eq!(
        get(&t, Section::Person, 0, "orcid_uri"),
        "https://sandbox.orcid.org/0000-0002-7319-2192"
    );
    assert_eq!(
        get(&t, Section::Person, 0, "orcid_host"),
        "sandbox.orcid.org"
    );

    // research-resource-summary 1498: one host, NASA (proposal.hosts.organization[0])
    let h = Section::ResearchResourceHosts;
    assert_eq!(count(&t, h), 1);
    let row: Vec<String> = [
        "orcid",
        "owner_put_code",
        "ordinal",
        "host_name",
        "host_city",
        "host_region",
        "host_country",
        "host_disambiguated_id",
        "host_disambiguation_source",
        "container",
        "item_ordinal",
    ]
    .iter()
    .map(|c| get(&t, h, 0, c))
    .collect();
    assert_eq!(
        row,
        [
            me,
            "1498",
            "1",
            "NASA",
            "Washington",
            "District of Columbia",
            "US",
            "grid.238252.c",
            "GRID",
            "proposal",
            "<null>",
        ]
    );
}

/// write_samples/research-resource-3.0.json: two proposal hosts (ORCID,
/// NASA) and one host per each of the five resource items (python3 -I:
/// proposal.hosts.organization, resource-item[i].hosts.organization); the
/// document has no path, so orcid and owner_put_code are NULL.
#[cfg(feature = "json")]
#[test]
fn research_resource_json_hosts() {
    let doc = yorc::json::from_str(
        "research-resource",
        &sample_text("write_samples", "research-resource-3.0.json"),
    )
    .expect("research-resource-3.0.json");
    let t = Tables::from_document(&doc, None);
    let h = Section::ResearchResourceHosts;
    let cols = [
        "orcid",
        "owner_put_code",
        "container",
        "item_ordinal",
        "ordinal",
        "host_name",
        "host_city",
        "host_region",
        "host_disambiguated_id",
        "host_disambiguation_source",
    ];
    let rows: Vec<Vec<String>> = (0..count(&t, h))
        .map(|r| cols.iter().map(|c| get(&t, h, r, c)).collect())
        .collect();
    let n = "<null>";
    let (grid, ror) = (
        ["grid.455335.1", "GRID"],
        ["https://ror.org/027ka1x80", "ROR"],
    );
    let row = |container, item, ordinal, name, city, region, [id, source]: [&'static str; 2]| {
        [
            n, n, container, item, ordinal, name, city, region, id, source,
        ]
    };
    let moon = |item| {
        let name = "Moon Holdings Ltd";
        row(
            "resource-item",
            item,
            "1",
            name,
            "Bethesda",
            "Maryland",
            grid,
        )
    };
    assert_eq!(
        rows,
        [
            row("proposal", n, "1", "ORCID", "Bethesda", "Maryland", grid),
            row("proposal", n, "2", "NASA", "Washington", "D.C", ror),
            row(
                "resource-item",
                "1",
                "1",
                "Lasers-R-US",
                "Bethesda",
                "Maryland",
                grid
            ),
            moon("2"),
            moon("3"),
            moon("4"),
            moon("5"),
        ]
    );
    // the first-host column keeps ORCID (orcid_json.test)
    assert_eq!(get(&t, Section::ResearchResources, 0, "host_name"), "ORCID");
}
