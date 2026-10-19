//! Schema walk (ARCHITECTURE.md, "Schema-derived, not hand-picked"): every
//! element and attribute declared by the v3.0 record XSDs and `common-3.0.xsd`
//! must have a model counterpart.
//!
//! "Has a counterpart" is checked through the writer: a fully populated model
//! (every `Option` set, every `Vec` with one entry, every root kind) is written,
//! and the qualified element names and attribute names it emits are compared
//! with the names the XSDs declare (parsed here with quick-xml).
//!
//! * declared but never emitted → must be listed in `tests/xsd_uncovered.txt`
//!   with a reason (the test prints the count);
//! * listed but emitted → the list is stale: fail;
//! * emitted but not declared → the writer invents names: fail.
#![cfg(feature = "xml")]

mod reference_writer;
mod support;

use std::collections::BTreeSet;
use std::fs;

use quick_xml::Reader;
use quick_xml::events::Event;

use support::*;
use yorc::model::*;
use yorc::xml;

// ------------------------------------------------------------- the XSDs

/// (`prefix:name` of every declared element, `@name` of every declared attribute).
fn declared() -> (BTreeSet<String>, BTreeSet<String>) {
    let mut files: Vec<_> = xml_files_with_ext(&record_xsd_dir(), "xsd");
    files.push(model_dir().join("common_3.0/common-3.0.xsd"));
    let mut elements = BTreeSet::new();
    let mut attributes = BTreeSet::new();
    for f in files {
        let text = fs::read_to_string(&f).unwrap();
        let mut reader = Reader::from_str(&text);
        let mut prefix = String::new();
        loop {
            let ev = reader
                .read_event()
                .unwrap_or_else(|e| panic!("{}: {e}", f.display()));
            let e = match ev {
                Event::Start(e) | Event::Empty(e) => e,
                Event::Eof => break,
                _ => continue,
            };
            let qname = e.name().into_inner().to_string();
            let attr = |n: &str| {
                e.attributes()
                    .filter_map(|a| a.ok())
                    .find(|a| a.key.0 == n)
                    .map(|a| a.value.to_string())
            };
            match qname.as_str() {
                "xs:schema" => {
                    let tns = attr("targetNamespace").expect("targetNamespace");
                    prefix = tns.rsplit('/').next().unwrap().to_string();
                }
                "xs:element" => {
                    if let Some(n) = attr("name") {
                        elements.insert(format!("{prefix}:{n}"));
                    }
                }
                "xs:attribute" => {
                    if let Some(n) = attr("name") {
                        attributes.insert(format!("@{n}"));
                    }
                }
                _ => {}
            }
        }
    }
    (elements, attributes)
}

fn xml_files_with_ext(dir: &std::path::Path, ext: &str) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == ext))
        .collect();
    v.sort();
    v
}

// ------------------------------------------------ a fully populated model

fn s(v: &str) -> Option<String> {
    Some(v.to_string())
}

fn ident() -> OrcidIdentifier {
    OrcidIdentifier {
        uri: s("https://orcid.org/0000-0002-1825-0097"),
        path: s("0000-0002-1825-0097"),
        host: s("orcid.org"),
    }
}

fn source() -> Source {
    Source {
        source_orcid: Some(ident()),
        source_client_id: Some(ident()),
        source_name: s("n"),
        assertion_origin_orcid: Some(ident()),
        assertion_origin_client_id: Some(ident()),
        assertion_origin_name: s("n"),
    }
}

fn meta() -> ItemMeta {
    ItemMeta {
        put_code: Some(1),
        visibility: Some(Visibility::Public),
        display_index: Some(1),
        display_index_text: None,
        path: s("/0000-0002-1825-0097/x/1"),
        created_date: s("2001-12-31T12:00:00"),
        last_modified_date: s("2001-12-31T12:00:00"),
        source: Some(source()),
    }
}

fn date() -> Option<FuzzyDate> {
    Some(FuzzyDate {
        year: Some(2001),
        month: Some(2),
        day: Some(3),
    })
}

fn org() -> Organization {
    Organization {
        name: s("o"),
        address: Some(OrganizationAddress {
            city: s("c"),
            region: s("r"),
            country: s("US"),
        }),
        disambiguated_organization: Some(DisambiguatedOrganization {
            identifier: s("1"),
            source: s("ROR"),
        }),
    }
}

fn ext_id() -> ExternalId {
    ExternalId {
        meta: meta(),
        id_type: s("doi"),
        value: s("10.1/x"),
        normalized: Some(TransientString {
            value: s("10.1/x"),
            transient: Some(true),
        }),
        normalized_error: Some(TransientError {
            error_code: s("8001"),
            error_message: s("m"),
            transient: Some(true),
        }),
        url: s("https://doi.org/10.1/x"),
        relationship: Some(Relationship::SelfRef),
    }
}

fn title() -> Option<Title> {
    Some(Title {
        title: s("t"),
        subtitle: s("s"),
        translated_title: Some(TranslatedTitle {
            value: s("tt"),
            language_code: s("en"),
        }),
    })
}

fn contributor() -> Contributor {
    Contributor {
        contributor_orcid: Some(ident()),
        credit_name: s("c"),
        contributor_email: s("a@b.c"),
        attributes: Some(ContributorAttributes {
            sequence: Some(ContributorSequence::First),
            role: Some(ContributorRole::Author),
        }),
    }
}

fn work() -> Work {
    Work {
        meta: meta(),
        title: title(),
        journal_title: s("j"),
        short_description: s("d"),
        citation: Some(Citation {
            citation_type: Some(CitationType::Bibtex),
            citation_value: s("@x{}"),
        }),
        work_type: Some(WorkType::Book),
        publication_date: date(),
        external_ids: vec![ext_id()],
        url: s("https://x"),
        contributors: vec![contributor()],
        language_code: s("en"),
        country: Some(Country {
            value: s("US"),
            visibility: Some(Visibility::Public),
        }),
    }
}

fn work_summary() -> WorkSummary {
    WorkSummary {
        meta: meta(),
        title: title(),
        external_ids: vec![ext_id()],
        url: s("https://x"),
        work_type: Some(WorkType::Book),
        publication_date: date(),
        journal_title: s("j"),
    }
}

fn funding() -> Funding {
    Funding {
        meta: meta(),
        funding_type: Some(FundingType::Grant),
        organization_defined_type: s("t"),
        title: title(),
        short_description: s("d"),
        amount: Some(Amount {
            value: s("1"),
            currency_code: s("USD"),
        }),
        url: s("https://x"),
        start_date: date(),
        end_date: date(),
        external_ids: vec![ext_id()],
        contributors: vec![Contributor {
            attributes: Some(ContributorAttributes {
                sequence: None, // not in funding-3.0.xsd
                role: Some(ContributorRole::Lead),
            }),
            ..contributor()
        }],
        organization: Some(org()),
    }
}

fn funding_summary() -> FundingSummary {
    FundingSummary {
        meta: meta(),
        title: title(),
        external_ids: vec![ext_id()],
        url: s("https://x"),
        funding_type: Some(FundingType::Grant),
        start_date: date(),
        end_date: date(),
        organization: Some(org()),
    }
}

fn affiliation(kind: AffiliationKind) -> Affiliation {
    Affiliation {
        kind,
        meta: meta(),
        department_name: s("d"),
        role_title: s("r"),
        start_date: date(),
        end_date: date(),
        organization: Some(org()),
        url: s("https://x"),
        external_ids: vec![ext_id()],
    }
}

fn affiliations(kind: AffiliationKind) -> Option<Affiliations> {
    Some(Affiliations {
        kind,
        path: s("/p"),
        last_modified_date: s("2001-12-31T12:00:00"),
        groups: vec![AffiliationGroup {
            last_modified_date: s("2001-12-31T12:00:00"),
            external_ids: vec![ext_id()],
            summaries: vec![affiliation(kind)],
        }],
    })
}

fn peer_review_summary() -> PeerReviewSummary {
    PeerReviewSummary {
        meta: meta(),
        reviewer_role: Some(PeerReviewRole::Reviewer),
        external_ids: vec![ext_id()],
        review_url: s("https://x"),
        review_type: Some(PeerReviewType::Review),
        completion_date: date(),
        review_group_id: s("issn:1234-5678"),
        convening_organization: Some(org()),
    }
}

fn peer_review() -> PeerReview {
    PeerReview {
        meta: meta(),
        reviewer_role: Some(PeerReviewRole::Reviewer),
        review_identifiers: vec![ext_id()],
        review_url: s("https://x"),
        review_type: Some(PeerReviewType::Review),
        review_completion_date: date(),
        review_group_id: s("issn:1234-5678"),
        subject_external_identifier: Some(ext_id()),
        subject_container_name: s("c"),
        subject_type: s("journal-article"),
        subject_name: title(),
        subject_url: s("https://x"),
        convening_organization: Some(org()),
    }
}

fn proposal() -> Option<Proposal> {
    Some(Proposal {
        title: title(),
        hosts: vec![org()],
        external_ids: vec![ext_id()],
        start_date: date(),
        end_date: date(),
        url: s("https://x"),
    })
}

fn research_resource() -> ResearchResource {
    ResearchResource {
        meta: meta(),
        proposal: proposal(),
        resource_items: vec![ResourceItem {
            resource_name: s("n"),
            resource_type: Some(ResourceType::Equipment),
            hosts: vec![org()],
            external_ids: vec![ext_id()],
            url: s("https://x"),
        }],
    }
}

fn activities() -> ActivitiesSummary {
    ActivitiesSummary {
        path: s("/p"),
        last_modified_date: s("2001-12-31T12:00:00"),
        distinctions: affiliations(AffiliationKind::Distinction),
        educations: affiliations(AffiliationKind::Education),
        employments: affiliations(AffiliationKind::Employment),
        fundings: Some(Fundings {
            path: s("/p"),
            last_modified_date: s("2001-12-31T12:00:00"),
            groups: vec![FundingGroup {
                last_modified_date: s("2001-12-31T12:00:00"),
                external_ids: vec![ext_id()],
                funding_summaries: vec![funding_summary()],
            }],
        }),
        invited_positions: affiliations(AffiliationKind::InvitedPosition),
        memberships: affiliations(AffiliationKind::Membership),
        peer_reviews: Some(PeerReviews {
            path: s("/p"),
            last_modified_date: s("2001-12-31T12:00:00"),
            groups: vec![PeerReviewGroup {
                last_modified_date: s("2001-12-31T12:00:00"),
                external_ids: vec![ext_id()],
                duplicates: vec![PeerReviewDuplicates {
                    last_modified_date: s("2001-12-31T12:00:00"),
                    external_ids: vec![ext_id()],
                    summaries: vec![peer_review_summary()],
                }],
            }],
        }),
        qualifications: affiliations(AffiliationKind::Qualification),
        research_resources: Some(ResearchResources {
            path: s("/p"),
            last_modified_date: s("2001-12-31T12:00:00"),
            groups: vec![ResearchResourceGroup {
                last_modified_date: s("2001-12-31T12:00:00"),
                external_ids: vec![ext_id()],
                summaries: vec![ResearchResourceSummary {
                    meta: meta(),
                    proposal: proposal(),
                }],
            }],
        }),
        services: affiliations(AffiliationKind::Service),
        works: Some(Works {
            path: s("/p"),
            last_modified_date: s("2001-12-31T12:00:00"),
            groups: vec![WorkGroup {
                last_modified_date: s("2001-12-31T12:00:00"),
                external_ids: vec![ext_id()],
                work_summaries: vec![work_summary()],
            }],
        }),
    }
}

fn name() -> Name {
    Name {
        visibility: Some(Visibility::Public),
        path: s("/p"),
        created_date: s("2001-12-31T12:00:00"),
        last_modified_date: s("2001-12-31T12:00:00"),
        given_names: s("g"),
        family_name: s("f"),
        credit_name: s("c"),
    }
}

fn biography() -> Biography {
    Biography {
        visibility: Some(Visibility::Public),
        path: s("/p"),
        created_date: s("2001-12-31T12:00:00"),
        last_modified_date: s("2001-12-31T12:00:00"),
        content: s("b"),
    }
}

fn list<T>(item: T) -> ItemList<T> {
    ItemList {
        path: s("/p"),
        last_modified_date: s("2001-12-31T12:00:00"),
        items: vec![item],
    }
}

fn person() -> Person {
    Person {
        path: s("/p"),
        last_modified_date: s("2001-12-31T12:00:00"),
        name: Some(name()),
        other_names: Some(list(OtherName {
            meta: meta(),
            content: s("o"),
        })),
        biography: Some(biography()),
        researcher_urls: Some(list(ResearcherUrl {
            meta: meta(),
            url_name: s("n"),
            url: s("https://x"),
        })),
        emails: Some(list(Email {
            meta: meta(),
            email: s("a@b.c"),
            verified: Some(true),
            primary: Some(true),
        })),
        addresses: Some(list(Address {
            meta: meta(),
            country: s("US"),
        })),
        keywords: Some(list(Keyword {
            meta: meta(),
            content: s("k"),
        })),
        external_identifiers: Some(list(ext_id())),
    }
}

fn record() -> Record {
    Record {
        path: s("/0000-0002-1825-0097"),
        orcid_identifier: Some(ident()),
        preferences: Some(Preferences { locale: s("en") }),
        history: Some(History {
            visibility: Some(Visibility::Private),
            creation_method: Some(CreationMethod::Api),
            completion_date: s("2001-12-31T12:00:00"),
            submission_date: s("2001-12-31T12:00:00"),
            last_modified_date: s("2001-12-31T12:00:00"),
            claimed: Some(true),
            source: Some(source()),
            deactivation_date: s("2001-12-31T12:00:00"),
            verified_email: Some(true),
            verified_primary_email: Some(true),
        }),
        person: Some(person()),
        activities_summary: Some(activities()),
    }
}

fn every_root() -> Vec<Root> {
    let mut roots = vec![
        Root::Record(record()),
        Root::Work(work()),
        Root::Funding(funding()),
        Root::PeerReview(peer_review()),
        Root::ResearchResource(research_resource()),
        Root::PersonalDetails(PersonalDetails {
            path: s("/p"),
            last_modified_date: s("2001-12-31T12:00:00"),
            name: Some(name()),
            other_names: Some(list(OtherName {
                meta: meta(),
                content: s("o"),
            })),
            biography: Some(biography()),
        }),
        Root::Name(name()),
        Root::Biography(biography()),
        Root::CreditName(s("c")),
        Root::Bulk(Bulk {
            items: vec![
                BulkItem::Work(work()),
                BulkItem::Error(OrcidError {
                    response_code: Some(409),
                    developer_message: s("d"),
                    user_message: s("u"),
                    error_code: Some(9021),
                    more_info: s("https://x"),
                }),
            ],
        }),
        Root::Search(Search {
            num_found: Some(1),
            results: vec![ident()],
        }),
        Root::ExpandedSearch(ExpandedSearch {
            num_found: Some(1),
            results: vec![ExpandedResult {
                orcid_id: s("0000-0002-1825-0097"),
                given_names: s("g"),
                family_names: s("f"),
                credit_name: s("c"),
                other_names: vec!["o".into()],
                emails: vec!["a@b.c".into()],
                institution_names: vec!["i".into()],
            }],
        }),
    ];
    for k in AffiliationKind::ALL {
        roots.push(Root::Affiliation(affiliation(k)));
    }
    roots
}

/// Qualified element names (`prefix:local`) and `@attribute` names in a
/// document written by this crate (no default namespace, no prefixed
/// attributes besides `xmlns:*`).
fn emitted(xml_text: &str, elements: &mut BTreeSet<String>, attributes: &mut BTreeSet<String>) {
    let mut reader = Reader::from_str(xml_text);
    loop {
        match reader.read_event().unwrap() {
            Event::Start(e) | Event::Empty(e) => {
                elements.insert(e.name().into_inner().to_string());
                for a in e.attributes() {
                    let a = a.unwrap();
                    let k: &str = a.key.0;
                    if !k.starts_with("xmlns") {
                        attributes.insert(format!("@{k}"));
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
}

fn allow_list() -> BTreeSet<String> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/xsd_uncovered.txt");
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .map(|l| l.split('#').next().unwrap().trim())
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

#[test]
fn every_xsd_element_and_attribute_has_a_model_counterpart() {
    let (decl_el, decl_at) = declared();
    let mut el = BTreeSet::new();
    let mut at = BTreeSet::new();
    for root in every_root() {
        let text = xml::to_string(&root).unwrap();
        // the kitchen-sink model must also survive a round trip
        let back = xml::from_str(&text).unwrap();
        assert_eq!(back.root, root, "kitchen-sink round trip:\n{text}");
        emitted(&text, &mut el, &mut at);
    }
    let declared: BTreeSet<String> = decl_el.union(&decl_at).cloned().collect();
    let emitted: BTreeSet<String> = el.union(&at).cloned().collect();
    let allowed = allow_list();

    let invented: Vec<_> = emitted.difference(&declared).collect();
    assert!(
        invented.is_empty(),
        "written but not declared by any XSD: {invented:?}"
    );

    let uncovered: BTreeSet<String> = declared.difference(&emitted).cloned().collect();
    let unlisted: Vec<_> = uncovered.difference(&allowed).collect();
    let stale: Vec<_> = allowed.difference(&uncovered).collect();
    assert!(
        unlisted.is_empty(),
        "declared by the XSDs but without a model counterpart (add to the model, or list in tests/xsd_uncovered.txt with a reason): {unlisted:?}"
    );
    assert!(
        stale.is_empty(),
        "listed in tests/xsd_uncovered.txt but covered (remove): {stale:?}"
    );
    eprintln!(
        "schema walk: {} declared names ({} elements, {} attributes), {} covered by the model, {} listed as uncovered",
        declared.len(),
        decl_el.len(),
        decl_at.len(),
        emitted.len(),
        allowed.len()
    );
}

#[test]
fn funding_contributor_sequence_is_unwritable() {
    let mut f = funding();
    f.contributors = vec![contributor()];
    match xml::to_string(&Root::Funding(f)) {
        Err(yorc::Error::Unwritable { path, .. }) => {
            assert_eq!(
                path,
                "funding/contributors/contributor/contributor-attributes/contributor-sequence"
            )
        }
        other => panic!("expected Unwritable, got {other:?}"),
    }
}

#[test]
fn namespaces_match_the_xsds() {
    // Every Ns the writer knows is the targetNamespace of a vendored XSD.
    let mut tns = BTreeSet::new();
    for f in xml_files_with_ext(&record_xsd_dir(), "xsd") {
        let t = fs::read_to_string(&f).unwrap();
        let i = t.find("targetNamespace=\"").unwrap() + "targetNamespace=\"".len();
        tns.insert(t[i..i + t[i..].find('"').unwrap()].to_string());
    }
    tns.insert("http://www.orcid.org/ns/common".to_string());
    let ours: BTreeSet<String> = xml::Ns::ALL.iter().map(|n| n.uri()).collect();
    assert_eq!(ours, tns);
}

// ------------------------------------------- the 0.1.0 writer as reference

/// Every root kind, fully populated, then with edge values (empty strings,
/// empty and default containers, negative and extreme integers, a
/// non-integer display index) and the two errors: the streaming writer must
/// give exactly the bytes, or the error, of the 0.1.0 tree writer.
#[test]
fn writer_matches_reference_on_every_root() {
    let mut roots = every_root();
    let a = activities();
    let p = person();
    let r = record();
    roots.extend([
        Root::ActivitiesSummary(a.clone()),
        Root::Works(a.works.clone().unwrap()),
        Root::WorkSummary(work_summary()),
        Root::Fundings(a.fundings.clone().unwrap()),
        Root::FundingSummary(funding_summary()),
        Root::Affiliations(a.educations.clone().unwrap()),
        Root::PeerReviews(a.peer_reviews.clone().unwrap()),
        Root::PeerReviewSummary(peer_review_summary()),
        Root::ResearchResources(a.research_resources.clone().unwrap()),
        Root::ResearchResourceSummary(ResearchResourceSummary {
            meta: meta(),
            proposal: proposal(),
        }),
        Root::Person(p.clone()),
        Root::OtherNames(p.other_names.clone().unwrap()),
        Root::OtherName(p.other_names.clone().unwrap().items[0].clone()),
        Root::ResearcherUrls(p.researcher_urls.clone().unwrap()),
        Root::ResearcherUrl(p.researcher_urls.clone().unwrap().items[0].clone()),
        Root::Emails(p.emails.clone().unwrap()),
        Root::Email(p.emails.clone().unwrap().items[0].clone()),
        Root::Addresses(p.addresses.clone().unwrap()),
        Root::Address(p.addresses.clone().unwrap().items[0].clone()),
        Root::Keywords(p.keywords.clone().unwrap()),
        Root::Keyword(p.keywords.clone().unwrap().items[0].clone()),
        Root::ExternalIdentifiers(p.external_identifiers.clone().unwrap()),
        Root::ExternalIdentifier(ext_id()),
        Root::History(r.history.clone().unwrap()),
        Root::Preferences(r.preferences.clone().unwrap()),
        Root::Error(OrcidError {
            response_code: Some(-1),
            ..OrcidError::default()
        }),
        Root::CreditName(None),
        Root::CreditName(Some(String::new())),
        Root::Record(Record::default()),
        Root::ActivitiesSummary(ActivitiesSummary::default()),
        Root::Person(Person::default()),
        Root::Search(Search {
            num_found: Some(i64::MAX),
            results: vec![OrcidIdentifier::default()],
        }),
        Root::ExpandedSearch(ExpandedSearch::default()),
        Root::Bulk(Bulk::default()),
        Root::Unknown("something-else".to_string()),
    ]);
    for k in AffiliationKind::ALL {
        roots.push(Root::AffiliationSummary(affiliation(k)));
        roots.push(Root::Affiliations(Affiliations::new(k)));
    }
    let mut w = work();
    w.meta.put_code = Some(i64::MIN);
    w.meta.display_index = Some(-7);
    w.publication_date = Some(FuzzyDate {
        year: Some(-5),
        month: Some(123),
        day: Some(0),
    });
    w.title = Some(Title {
        title: Some(String::new()),
        subtitle: None,
        translated_title: Some(TranslatedTitle::default()),
    });
    w.external_ids = vec![ExternalId::default()];
    w.contributors = vec![
        Contributor::default(),
        Contributor {
            attributes: Some(ContributorAttributes::default()),
            ..Contributor::default()
        },
    ];
    w.country = Some(Country::default());
    w.citation = Some(Citation::default());
    roots.push(Root::Work(w));
    let mut m = meta();
    m.display_index_text = Some("-1".to_string());
    roots.push(Root::Keyword(Keyword {
        meta: m,
        content: None,
    }));
    let mut f = funding();
    f.contributors = vec![contributor()];
    roots.push(Root::Funding(f));
    let mut k = p.keywords.clone().unwrap().items[0].clone();
    k.meta.source.as_mut().unwrap().source_name = s("tab\tok, bell\u{7} not");
    roots.push(Root::Keyword(k));

    for (i, root) in roots.iter().enumerate() {
        assert_eq!(
            xml::to_string(root),
            reference_writer::to_string(root),
            "root {i}"
        );
    }
}
