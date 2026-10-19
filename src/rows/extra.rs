//! yorc-duckdb's `timeline` and `group_ids` sections ([`Extra`]).

use super::{ColType, Column, Row, Section, Tables, col, integer, varchar};
use crate::model::*;

/// Two more of yorc-duckdb's sections, kept apart from [`Section`] and
/// [`Tables`] so that those stay exactly as they are (the fourteen sections
/// and their 300 columns): [`Extra::Timeline`] (`orcid_timeline`) and
/// [`Extra::GroupIds`] (`orcid_group_ids`). They are opt-in: nothing builds
/// them unless asked ([`Extra::rows`], [`timeline`], [`group_ids`]).
///
/// `timeline`: one row per dated event of the document's items (every
/// activity, person item, name and biography): a `created` row (`event_at` =
/// `created`) and, when the last modification differs, a `modified` row
/// (`event_at` = `last_modified`); an item with only a modification date
/// gives just that row, an undated item one row with NULL `event_at` and
/// `event`. Items come in document (XSD) order: the person's name, other
/// names, biography, researcher URLs, emails, addresses, keywords and
/// external identifiers, then the activities in `activities-summary` order
/// (distinctions, educations, employments, fundings, invited positions,
/// memberships, peer reviews, qualifications, research resources, services,
/// works). `kind` is the item element without `-summary`; `label` the
/// item's title (works: else its `country`), role title (affiliations),
/// subject name (full peer reviews), proposal title (research resources),
/// value (person items: content, URL name, email, country, external-id
/// value) or given names (name); `organization` the organization (fundings,
/// affiliations), convening organization (peer reviews) or first named
/// proposal host (research resources). `orcid` as in the other sections.
/// The items yorc-duckdb finds by element suffix anywhere but the schema
/// does not put there (the employments of a `record-summary`, for one) give
/// no rows here, as for the other sections.
///
/// `group_ids`: one row per `group-id-record` (a document of its own, or
/// each record of a `group-id-records` page, whose `total`, `page` and
/// `page_size` repeat on every row; NULL for a single record).
///
/// ```
/// use yorc::rows::{ColType, Extra};
///
/// assert_eq!(Extra::from_name("timeline"), Some(Extra::Timeline));
/// assert_eq!(Extra::Timeline.columns().len(), 13);
/// assert_eq!(Extra::GroupIds.columns()[0].name, "put_code");
/// assert_eq!(Extra::GroupIds.columns()[0].ty, ColType::Bigint);
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Extra {
    /// `timeline` (yorc-duckdb's `orcid_timeline`)
    Timeline,
    /// `group_ids` (yorc-duckdb's `orcid_group_ids`)
    GroupIds,
}

impl Extra {
    /// Every extra section, in yorc-duckdb's order.
    pub const ALL: &'static [Extra] = &[Extra::Timeline, Extra::GroupIds];

    /// Table name (`timeline`, `group_ids`).
    pub fn name(self) -> &'static str {
        match self {
            Extra::Timeline => "timeline",
            Extra::GroupIds => "group_ids",
        }
    }

    /// The extra section whose table name is `name`.
    pub fn from_name(name: &str) -> Option<Extra> {
        Extra::ALL.iter().copied().find(|s| s.name() == name)
    }

    /// Columns in table order (yorc-duckdb's).
    pub fn columns(self) -> &'static [Column] {
        match self {
            Extra::Timeline => TIMELINE,
            Extra::GroupIds => GROUP_IDS,
        }
    }

    /// Position of a column, by name.
    pub fn column_index(self, name: &str) -> Option<usize> {
        self.columns().iter().position(|c| c.name == name)
    }

    /// The section's rows for one document; `source_hash` fills the
    /// `source_hash` column.
    pub fn rows(self, doc: &Document, source_hash: Option<&str>) -> Vec<Row> {
        match self {
            Extra::Timeline => timeline(doc, source_hash),
            Extra::GroupIds => group_ids(doc, source_hash),
        }
    }
}

impl Tables {
    /// Parses `xml` and returns the rows of one extra section, with
    /// `source_hash` = SHA-256 of `xml`: what `orcid_<section>_xml(xml)`
    /// returns.
    ///
    /// ```
    /// use yorc::rows::{Extra, Tables};
    ///
    /// let xml = br#"<group-id:group-id-record xmlns:group-id="http://www.orcid.org/ns/group-id">
    ///   <group-id:name>Journal of Vents</group-id:name>
    ///   <group-id:group-id>issn:0000-0001</group-id:group-id>
    /// </group-id:group-id-record>"#;
    /// let rows = Tables::extra_from_xml(Extra::GroupIds, xml)?;
    /// assert_eq!(rows.len(), 1);
    /// assert_eq!(rows[0][2].as_str(), Some("issn:0000-0001"));
    /// # Ok::<(), yorc::Error>(())
    /// ```
    pub fn extra_from_xml(extra: Extra, xml: &[u8]) -> crate::Result<Vec<Row>> {
        let doc = crate::xml::from_slice(xml)?;
        let hash = crate::hash::sha256_hex(xml);
        Ok(extra.rows(&doc, Some(&hash)))
    }
}

/// The `timeline` rows of a document (see [`Extra`]).
///
/// ```
/// use yorc::rows::{timeline, Extra};
///
/// let doc = yorc::xml::from_str(r#"<work:work xmlns:work="http://www.orcid.org/ns/work"
///     xmlns:common="http://www.orcid.org/ns/common" put-code="7" path="/0000-0002-1825-0097/work/7">
///   <common:created-date>2020-01-01T00:00:00.000Z</common:created-date>
///   <common:last-modified-date>2021-06-01T00:00:00.000Z</common:last-modified-date>
///   <work:title><common:title>Vents</common:title></work:title>
/// </work:work>"#)?;
/// let rows = timeline(&doc, None);
/// assert_eq!(rows.len(), 2); // created, then modified
/// let event = Extra::Timeline.column_index("event").unwrap();
/// assert_eq!(rows[0][event].as_str(), Some("created"));
/// assert_eq!(rows[1][event].as_str(), Some("modified"));
/// assert_eq!(rows[1][1].as_str(), Some("2021-06-01T00:00:00.000Z")); // event_at
/// # Ok::<(), yorc::Error>(())
/// ```
pub fn timeline(doc: &Document, source_hash: Option<&str>) -> Vec<Row> {
    let mut t = Timeline {
        doc_orcid: doc.orcid(),
        hash: source_hash,
        rows: Vec::new(),
    };
    t.document(doc);
    t.rows
}

/// The `group_ids` rows of a document: none unless it is a
/// `group-id-record` or a `group-id-records` page.
///
/// ```
/// let doc = yorc::xml::from_str(r#"<g:group-id-records xmlns:g="http://www.orcid.org/ns/group-id">
///   <g:total>1</g:total><g:page>0</g:page><g:page-size>100</g:page-size>
///   <g:group-id-record put-code="11"><g:name>A</g:name></g:group-id-record>
/// </g:group-id-records>"#)?;
/// let rows = yorc::rows::group_ids(&doc, None);
/// assert_eq!(rows.len(), 1);
/// assert_eq!(rows[0][0].as_i64(), Some(11)); // put_code
/// assert_eq!(rows[0][8].as_i64(), Some(1)); // total
/// # Ok::<(), yorc::Error>(())
/// ```
pub fn group_ids(doc: &Document, source_hash: Option<&str>) -> Vec<Row> {
    match &doc.root {
        Root::GroupIdRecord(r) => vec![group_id_row(r, None, source_hash)],
        Root::GroupIdRecords(p) => p
            .records
            .iter()
            .map(|r| group_id_row(r, Some(p), source_hash))
            .collect(),
        _ => Vec::new(),
    }
}

fn id_path(id: Option<&OrcidIdentifier>) -> Option<&str> {
    id.and_then(|i| i.path.as_deref())
}

fn group_id_row(r: &GroupIdRecord, page: Option<&GroupIdRecords>, hash: Option<&str>) -> Row {
    let src = r.source.as_ref();
    vec![
        integer(r.put_code),
        varchar(r.name.as_deref()),
        varchar(r.group_id.as_deref()),
        varchar(r.description.as_deref()),
        varchar(r.group_type.as_ref().map(|t| t.as_str())),
        varchar(src.and_then(|s| s.source_name.as_deref())),
        varchar(id_path(src.and_then(|s| s.source_client_id.as_ref()))),
        varchar(id_path(src.and_then(|s| s.source_orcid.as_ref()))),
        integer(page.and_then(|p| p.total)),
        integer(page.and_then(|p| p.page)),
        integer(page.and_then(|p| p.page_size)),
        varchar(hash),
    ]
}

// ---------------------------------------------------------------- timeline

/// One dated item, before it becomes its events.
struct Item<'a> {
    kind: &'static str,
    put_code: Option<i64>,
    label: Option<&'a str>,
    organization: Option<&'a str>,
    visibility: Option<&'a Visibility>,
    source_name: Option<&'a str>,
    created: Option<&'a str>,
    last_modified: Option<&'a str>,
    path: Option<&'a str>,
}

impl<'a> Item<'a> {
    fn of(
        kind: &'static str,
        m: &'a ItemMeta,
        label: Option<&'a str>,
        organization: Option<&'a str>,
    ) -> Item<'a> {
        Item {
            kind,
            put_code: m.put_code,
            label,
            organization,
            visibility: m.visibility.as_ref(),
            source_name: m.source.as_ref().and_then(|s| s.source_name.as_deref()),
            created: m.created_date.as_deref(),
            last_modified: m.last_modified_date.as_deref(),
            path: m.path.as_deref(),
        }
    }
}

fn title(t: Option<&Title>) -> Option<&str> {
    t.and_then(|t| t.title.as_deref())
}

fn org_name(o: Option<&Organization>) -> Option<&str> {
    o.and_then(|o| o.name.as_deref())
}

struct Timeline<'d> {
    doc_orcid: Option<&'d str>,
    hash: Option<&'d str>,
    rows: Vec<Row>,
}

impl Timeline<'_> {
    /// The row's `orcid`: the document's, else the item path's first segment.
    fn orcid(&self, item_path: Option<&str>) -> Option<String> {
        if let Some(o) = self.doc_orcid {
            return Some(o.to_string());
        }
        let seg = first_path_segment(trim_xml_space(item_path?));
        if seg.is_empty() {
            None
        } else {
            Some(seg.to_string())
        }
    }

    /// The item's events: `created`, then `modified` if it differs; one row
    /// with NULL `event_at` / `event` for an undated item.
    fn push(&mut self, it: Item<'_>) {
        let row = |event_at: Option<&str>, event: Option<&str>| -> Row {
            vec![
                varchar(self.orcid(it.path).as_deref()),
                varchar(event_at),
                varchar(event),
                varchar(Some(it.kind)),
                integer(it.put_code),
                varchar(it.label),
                varchar(it.organization),
                varchar(it.visibility.map(|v| v.as_str())),
                varchar(it.source_name),
                varchar(it.created),
                varchar(it.last_modified),
                varchar(it.path),
                varchar(self.hash),
            ]
        };
        let mut out = Vec::with_capacity(2);
        if let Some(c) = it.created {
            out.push(row(Some(c), Some("created")));
        }
        if let Some(m) = it.last_modified.filter(|&m| it.created != Some(m)) {
            out.push(row(Some(m), Some("modified")));
        }
        if it.created.is_none() && it.last_modified.is_none() {
            out.push(row(None, None));
        }
        self.rows.extend(out);
    }

    fn document(&mut self, doc: &Document) {
        match &doc.root {
            Root::Record(r) => {
                if let Some(p) = &r.person {
                    self.person(p);
                }
                if let Some(a) = &r.activities_summary {
                    self.activities(a);
                }
            }
            Root::ActivitiesSummary(a) => self.activities(a),
            Root::Works(w) => self.works(w),
            Root::Work(w) => self.work(w),
            Root::WorkSummary(w) => self.work_summary(w),
            Root::Fundings(f) => self.fundings(f),
            Root::Funding(f) => self.funding(f),
            Root::FundingSummary(f) => self.funding_summary(f),
            Root::Affiliations(a) => self.affiliations(a),
            Root::Affiliation(a) | Root::AffiliationSummary(a) => self.affiliation(a),
            Root::PeerReviews(p) => self.peer_reviews(p),
            Root::PeerReview(p) => self.peer_review(p),
            Root::PeerReviewSummary(p) => self.peer_review_summary(p),
            Root::ResearchResources(r) => self.research_resources(r),
            Root::ResearchResource(r) => {
                self.research_resource(&r.meta, r.proposal.as_ref());
            }
            Root::ResearchResourceSummary(r) => {
                self.research_resource(&r.meta, r.proposal.as_ref());
            }
            Root::Person(p) => self.person(p),
            Root::PersonalDetails(p) => {
                self.name(p.name.as_ref());
                self.other_names(p.other_names.as_ref());
                self.biography(p.biography.as_ref());
            }
            Root::OtherNames(l) => self.other_names(Some(l)),
            Root::ResearcherUrls(l) => self.researcher_urls(Some(l)),
            Root::Emails(l) => self.emails(Some(l)),
            Root::Addresses(l) => self.addresses(Some(l)),
            Root::Keywords(l) => self.keywords(Some(l)),
            Root::ExternalIdentifiers(l) => self.external_identifiers(Some(l)),
            Root::Bulk(b) => {
                for item in &b.items {
                    if let BulkItem::Work(w) = item {
                        self.work(w);
                    }
                }
            }
            // A single person item, name or biography is not a timeline
            // item on its own (yorc-duckdb's patterns need its container).
            _ => {}
        }
    }

    // ------------------------------------------------------------- person

    fn person(&mut self, p: &Person) {
        self.name(p.name.as_ref());
        self.other_names(p.other_names.as_ref());
        self.biography(p.biography.as_ref());
        self.researcher_urls(p.researcher_urls.as_ref());
        self.emails(p.emails.as_ref());
        self.addresses(p.addresses.as_ref());
        self.keywords(p.keywords.as_ref());
        self.external_identifiers(p.external_identifiers.as_ref());
    }

    fn name(&mut self, n: Option<&Name>) {
        let Some(n) = n else {
            return;
        };
        self.push(Item {
            kind: "name",
            put_code: None,
            label: n.given_names.as_deref(),
            organization: None,
            visibility: n.visibility.as_ref(),
            source_name: None,
            created: n.created_date.as_deref(),
            last_modified: n.last_modified_date.as_deref(),
            path: n.path.as_deref(),
        });
    }

    fn biography(&mut self, b: Option<&Biography>) {
        let Some(b) = b else {
            return;
        };
        self.push(Item {
            kind: "biography",
            put_code: None,
            label: b.content.as_deref(),
            organization: None,
            visibility: b.visibility.as_ref(),
            source_name: None,
            created: b.created_date.as_deref(),
            last_modified: b.last_modified_date.as_deref(),
            path: b.path.as_deref(),
        });
    }

    fn other_names(&mut self, l: Option<&OtherNames>) {
        for x in l.map_or(&[][..], |l| l.items.as_slice()) {
            self.push(Item::of("other-name", &x.meta, x.content.as_deref(), None));
        }
    }

    fn researcher_urls(&mut self, l: Option<&ResearcherUrls>) {
        for x in l.map_or(&[][..], |l| l.items.as_slice()) {
            self.push(Item::of(
                "researcher-url",
                &x.meta,
                x.url_name.as_deref(),
                None,
            ));
        }
    }

    fn emails(&mut self, l: Option<&Emails>) {
        for x in l.map_or(&[][..], |l| l.items.as_slice()) {
            self.push(Item::of("email", &x.meta, x.email.as_deref(), None));
        }
    }

    fn addresses(&mut self, l: Option<&Addresses>) {
        for x in l.map_or(&[][..], |l| l.items.as_slice()) {
            self.push(Item::of("address", &x.meta, x.country.as_deref(), None));
        }
    }

    fn keywords(&mut self, l: Option<&Keywords>) {
        for x in l.map_or(&[][..], |l| l.items.as_slice()) {
            self.push(Item::of("keyword", &x.meta, x.content.as_deref(), None));
        }
    }

    fn external_identifiers(&mut self, l: Option<&ExternalIdentifiers>) {
        for x in l.map_or(&[][..], |l| l.items.as_slice()) {
            self.push(Item::of(
                "external-identifier",
                &x.meta,
                x.value.as_deref(),
                None,
            ));
        }
    }

    // --------------------------------------------------------- activities

    /// Sections in `activities-summary` (XSD) order.
    fn activities(&mut self, a: &ActivitiesSummary) {
        if let Some(x) = &a.distinctions {
            self.affiliations(x);
        }
        if let Some(x) = &a.educations {
            self.affiliations(x);
        }
        if let Some(x) = &a.employments {
            self.affiliations(x);
        }
        if let Some(x) = &a.fundings {
            self.fundings(x);
        }
        if let Some(x) = &a.invited_positions {
            self.affiliations(x);
        }
        if let Some(x) = &a.memberships {
            self.affiliations(x);
        }
        if let Some(x) = &a.peer_reviews {
            self.peer_reviews(x);
        }
        if let Some(x) = &a.qualifications {
            self.affiliations(x);
        }
        if let Some(x) = &a.research_resources {
            self.research_resources(x);
        }
        if let Some(x) = &a.services {
            self.affiliations(x);
        }
        if let Some(x) = &a.works {
            self.works(x);
        }
    }

    fn works(&mut self, w: &Works) {
        for g in &w.groups {
            for s in &g.work_summaries {
                self.work_summary(s);
            }
        }
    }

    fn work_summary(&mut self, w: &WorkSummary) {
        self.push(Item::of("work", &w.meta, title(w.title.as_ref()), None));
    }

    fn work(&mut self, w: &Work) {
        // `title/title`, else `country` (its place in the work's sequence)
        let label =
            title(w.title.as_ref()).or_else(|| w.country.as_ref().and_then(|c| c.value.as_deref()));
        self.push(Item::of("work", &w.meta, label, None));
    }

    fn fundings(&mut self, f: &Fundings) {
        for g in &f.groups {
            for s in &g.funding_summaries {
                self.funding_summary(s);
            }
        }
    }

    fn funding_summary(&mut self, f: &FundingSummary) {
        let org = org_name(f.organization.as_ref());
        self.push(Item::of("funding", &f.meta, title(f.title.as_ref()), org));
    }

    fn funding(&mut self, f: &Funding) {
        let org = org_name(f.organization.as_ref());
        self.push(Item::of("funding", &f.meta, title(f.title.as_ref()), org));
    }

    fn affiliations(&mut self, list: &Affiliations) {
        for g in &list.groups {
            for a in &g.summaries {
                self.affiliation(a);
            }
        }
    }

    fn affiliation(&mut self, a: &Affiliation) {
        let org = org_name(a.organization.as_ref());
        self.push(Item::of(
            a.kind.as_str(),
            &a.meta,
            a.role_title.as_deref(),
            org,
        ));
    }

    fn peer_reviews(&mut self, p: &PeerReviews) {
        for g in &p.groups {
            for d in &g.duplicates {
                for s in &d.summaries {
                    self.peer_review_summary(s);
                }
            }
        }
    }

    fn peer_review_summary(&mut self, p: &PeerReviewSummary) {
        let org = org_name(p.convening_organization.as_ref());
        self.push(Item::of("peer-review", &p.meta, None, org));
    }

    fn peer_review(&mut self, p: &PeerReview) {
        let org = org_name(p.convening_organization.as_ref());
        let label = title(p.subject_name.as_ref());
        self.push(Item::of("peer-review", &p.meta, label, org));
    }

    fn research_resources(&mut self, r: &ResearchResources) {
        for g in &r.groups {
            for s in &g.summaries {
                self.research_resource(&s.meta, s.proposal.as_ref());
            }
        }
    }

    fn research_resource(&mut self, meta: &ItemMeta, proposal: Option<&Proposal>) {
        let label = title(proposal.and_then(|p| p.title.as_ref()));
        let org = proposal.and_then(|p| p.hosts.iter().find_map(|o| o.name.as_deref()));
        self.push(Item::of("research-resource", meta, label, org));
    }
}

// ------------------------------------------------------------------ schema
// Generated from yorc-duckdb/src/core/sections.cpp (kColumns); keep in sync.

const TIMELINE: &[Column] = &[
    col("orcid", ColType::Varchar),
    col("event_at", ColType::Varchar),
    col("event", ColType::Varchar),
    col("kind", ColType::Varchar),
    col("put_code", ColType::Bigint),
    col("label", ColType::Varchar),
    col("organization", ColType::Varchar),
    col("visibility", ColType::Varchar),
    col("source_name", ColType::Varchar),
    col("created", ColType::Varchar),
    col("last_modified", ColType::Varchar),
    col("path", ColType::Varchar),
    col("source_hash", ColType::Varchar),
];

const GROUP_IDS: &[Column] = &[
    col("put_code", ColType::Bigint),
    col("name", ColType::Varchar),
    col("group_id", ColType::Varchar),
    col("description", ColType::Varchar),
    col("type", ColType::Varchar),
    col("source_name", ColType::Varchar),
    col("source_client_id", ColType::Varchar),
    col("source_orcid", ColType::Varchar),
    col("total", ColType::Integer),
    col("page", ColType::Integer),
    col("page_size", ColType::Integer),
    col("source_hash", ColType::Varchar),
];

// The extra sections are not part of `Section::ALL` (whose 14 sections and
// 300 columns stay as they are).
const _: () = assert!(Section::ALL.len() == 14);
