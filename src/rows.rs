//! The model flattened into yorc-duckdb's section tables: the eleven
//! record sections (`works` ... `history`), two API-message sections
//! (`search_results`, `errors`) and `research_resource_hosts` (and, opt-in,
//! `timeline`, `group_ids` and `orcid_changes`; see below), with the
//! same table names, column names,
//! column order and column types as `yorc-duckdb/src/core/sections.cpp`
//! (documented in `yorc-duckdb/docs/REFERENCE.md`), so
//! `Tables::from_xml(xml)` yields the rows `orcid_<section>_xml(xml)`
//! returns.
//!
//! How values are derived, as in the extension:
//! * `orcid`: the first segment of the root element's `path` attribute, else
//!   the first non-empty `orcid-identifier/path`, else the first segment of
//!   the item's own `path` attribute.
//! * `group_index`: 1-based position of the enclosing `group` /
//!   `affiliation-group` among its siblings (`peer-review-group` is not a group).
//! * `owner`/`owner_put_code` (contributors, external ids, resource items):
//!   the nearest enclosing item with a put-code; without one, the nearest
//!   `group`, `affiliation-group` or `peer-review-group` (and no put-code).
//! * `doi` (works): the first `external-ids/external-id` of the work with type
//!   `doi` and relationship `self`, ASCII-lowercased.
//! * `source_hash`: lowercase hex SHA-256 of the document bytes.
//! * Repeated single-valued elements: first non-empty value per column (for
//!   research-resource hosts, per host column: the first-host columns of
//!   `research_resources` and `research_resource_items`, kept for
//!   compatibility).
//! * `research_resource_hosts`: one row per host organization of a proposal
//!   (`container` = `proposal`, `item_ordinal` NULL) or of a resource item
//!   (`resource-item`, `item_ordinal` = the item's 1-based position, as
//!   `research_resource_items.ordinal`), `ordinal` = the host's 1-based
//!   position in its `hosts`, `owner_put_code` = the research resource's
//!   put-code (summaries included).
//! * `person.orcid_uri` / `orcid_host`: the record's `orcid-identifier`
//!   (NULL outside a record document).
//! * The assertion origin (`assertion_origin_name`, `_orcid`, `_client_id`)
//!   is read in every section whose items carry a `common:source`.
//! * `search_results`: one row per `search/result` (`item` = `result`) or
//!   `expanded-search/expanded-result` (`expanded-result`); `num_found` is
//!   the document's `@num-found` on every row, `ordinal` the 1-based
//!   position; `other_names`, `emails` and `institution_names` join every
//!   value with `; `.
//! * `errors`: one row per `error` document, or per `error` entry of a
//!   `bulk` document (`container` = `bulk`, `position` = the 1-based entry
//!   index among works and errors; NULL and 1 for a standalone error).
//!   `orcid` falls back to the valid iD after `ORCID ` in
//!   `developer-message`, then `user-message` (locked and deprecated
//!   records), and `primary_orcid` is the valid iD after `refer to account:`
//!   (deprecated records), as yorc-duckdb's `orcid_errors`.
//!
//! Known differences (only for documents outside the XSDs): items are found
//! where the schema puts them, not by element-path suffix anywhere; a
//! put-code attribute that is present but not an integer counts as absent;
//! `response-code`, `error-code` and `@num-found` with a leading `+` give a
//! number here and NULL in the extension, and a `search/result` without an
//! `orcid-identifier` gives no row here. The extension's `summary`,
//! `summary_items` and `notifications` sections are not produced (this crate
//! does not model those documents).
//!
//! Opt-in, outside [`Section::ALL`] and [`Tables`] (which they leave as they
//! are): the extension's `timeline` and `group_ids` sections ([`Extra`],
//! [`timeline`], [`group_ids`]), and its `orcid_changes(old, new)`, the
//! field-level differences between two snapshots ([`changes`]).

use crate::model::*;

mod changes;
mod extra;

pub use changes::{CHANGE_COLUMNS, Change, ChangeKind, DIFF_SECTIONS, changes};
pub use extra::{Extra, group_ids, timeline};

/// DuckDB column type of a section column.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColType {
    /// `VARCHAR`
    Varchar,
    /// `INTEGER`
    Integer,
    /// `BIGINT`
    Bigint,
    /// `BOOLEAN`
    Boolean,
}

/// One column of a section table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Column {
    /// Column name, as in yorc-duckdb.
    pub name: &'static str,
    /// DuckDB type.
    pub ty: ColType,
}

const fn col(name: &'static str, ty: ColType) -> Column {
    Column { name, ty }
}

/// A cell: VARCHAR → `Varchar`, INTEGER/BIGINT → `Integer`, BOOLEAN →
/// `Boolean`, NULL → `Null`.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cell {
    /// SQL `NULL` (absent in the document).
    Null,
    /// A `VARCHAR` value.
    Varchar(String),
    /// An `INTEGER` or `BIGINT` value.
    Integer(i64),
    /// A `BOOLEAN` value.
    Boolean(bool),
}

impl Cell {
    /// `true` for [`Cell::Null`].
    pub fn is_null(&self) -> bool {
        matches!(self, Cell::Null)
    }

    /// The text of a `Varchar` cell.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Cell::Varchar(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// The value of an `Integer` cell.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Cell::Integer(v) => Some(*v),
            _ => None,
        }
    }

    /// The value of a `Boolean` cell.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Cell::Boolean(v) => Some(*v),
            _ => None,
        }
    }

    fn fits(&self, ty: ColType) -> bool {
        match self {
            Cell::Null => true,
            Cell::Varchar(_) => ty == ColType::Varchar,
            Cell::Integer(_) => ty == ColType::Integer || ty == ColType::Bigint,
            Cell::Boolean(_) => ty == ColType::Boolean,
        }
    }
}

/// One row, cells in [`Section::columns`] order.
pub type Row = Vec<Cell>;

/// The sections of yorc-duckdb this crate produces, in its order: the
/// eleven record sections, then `search_results`, `errors` and
/// `research_resource_hosts`.
///
/// ```
/// use yorc::rows::{ColType, Section};
///
/// assert_eq!(Section::from_name("peer_reviews"), Some(Section::PeerReviews));
/// assert_eq!(Section::from_name("errors"), Some(Section::Errors));
/// assert_eq!(Section::Works.columns()[0].name, "orcid");
/// assert_eq!(Section::Works.columns()[0].ty, ColType::Varchar);
/// assert_eq!(Section::ALL.len(), 14);
/// assert_eq!(Section::ALL.iter().map(|s| s.columns().len()).sum::<usize>(), 300);
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    /// `works`
    Works,
    /// `fundings`
    Fundings,
    /// `affiliations`
    Affiliations,
    /// `peer_reviews`
    PeerReviews,
    /// `research_resources`
    ResearchResources,
    /// `research_resource_items`
    ResearchResourceItems,
    /// `contributors`
    Contributors,
    /// `external_ids`
    ExternalIds,
    /// `person`
    Person,
    /// `person_items`
    PersonItems,
    /// `history`
    History,
    /// `search_results` (search-3.0 and expanded-search-3.0 results)
    SearchResults,
    /// `errors` (error-3.0, standalone or per bulk entry)
    Errors,
    /// `research_resource_hosts` (every host of a proposal or resource item)
    ResearchResourceHosts,
}

impl Section {
    /// Every section, in yorc-duckdb's order (a slice, so that a new
    /// section is not a type change).
    pub const ALL: &'static [Section] = &[
        Section::Works,
        Section::Fundings,
        Section::Affiliations,
        Section::PeerReviews,
        Section::ResearchResources,
        Section::ResearchResourceItems,
        Section::Contributors,
        Section::ExternalIds,
        Section::Person,
        Section::PersonItems,
        Section::History,
        Section::SearchResults,
        Section::Errors,
        Section::ResearchResourceHosts,
    ];

    /// Table name (`works`, ..., `history`, `search_results`, `errors`,
    /// `research_resource_hosts`).
    pub fn name(self) -> &'static str {
        match self {
            Section::Works => "works",
            Section::Fundings => "fundings",
            Section::Affiliations => "affiliations",
            Section::PeerReviews => "peer_reviews",
            Section::ResearchResources => "research_resources",
            Section::ResearchResourceItems => "research_resource_items",
            Section::Contributors => "contributors",
            Section::ExternalIds => "external_ids",
            Section::Person => "person",
            Section::PersonItems => "person_items",
            Section::History => "history",
            Section::SearchResults => "search_results",
            Section::Errors => "errors",
            Section::ResearchResourceHosts => "research_resource_hosts",
        }
    }

    /// The section whose table name is `name` (`works`, ...).
    pub fn from_name(name: &str) -> Option<Section> {
        Section::ALL.iter().copied().find(|s| s.name() == name)
    }

    /// Columns in table order.
    pub fn columns(self) -> &'static [Column] {
        match self {
            Section::Works => WORKS,
            Section::Fundings => FUNDINGS,
            Section::Affiliations => AFFILIATIONS,
            Section::PeerReviews => PEER_REVIEWS,
            Section::ResearchResources => RESEARCH_RESOURCES,
            Section::ResearchResourceItems => RESEARCH_RESOURCE_ITEMS,
            Section::Contributors => CONTRIBUTORS,
            Section::ExternalIds => EXTERNAL_IDS,
            Section::Person => PERSON,
            Section::PersonItems => PERSON_ITEMS,
            Section::History => HISTORY,
            Section::SearchResults => SEARCH_RESULTS,
            Section::Errors => ERRORS,
            Section::ResearchResourceHosts => RESEARCH_RESOURCE_HOSTS,
        }
    }

    /// Position of a column, by name.
    pub fn column_index(self, name: &str) -> Option<usize> {
        self.columns().iter().position(|c| c.name == name)
    }

    fn index(self) -> usize {
        match self {
            Section::Works => 0,
            Section::Fundings => 1,
            Section::Affiliations => 2,
            Section::PeerReviews => 3,
            Section::ResearchResources => 4,
            Section::ResearchResourceItems => 5,
            Section::Contributors => 6,
            Section::ExternalIds => 7,
            Section::Person => 8,
            Section::PersonItems => 9,
            Section::History => 10,
            Section::SearchResults => 11,
            Section::Errors => 12,
            Section::ResearchResourceHosts => 13,
        }
    }
}

/// Rows of every section for one document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tables {
    rows: [Vec<Row>; 14],
}

impl Tables {
    /// Rows of all sections. `source_hash` fills the `source_hash` column
    /// (normally `hash::sha256_hex` of the document bytes).
    pub fn from_document(doc: &Document, source_hash: Option<&str>) -> Tables {
        let record_id = match &doc.root {
            Root::Record(r) => r.orcid_identifier.as_ref(),
            _ => None,
        };
        let mut w = Walker {
            doc_orcid: doc.orcid(),
            record_id,
            hash: source_hash,
            tables: Tables::default(),
        };
        w.document(doc);
        w.tables
    }

    /// Parses `xml` and returns the rows of all sections, with `source_hash`
    /// = SHA-256 of `xml`: what `orcid_<section>_xml(xml)` returns.
    ///
    /// ```
    /// use yorc::rows::{Section, Tables};
    ///
    /// let xml = br#"<record:record path="/0000-0002-1825-0097" xmlns:record="http://www.orcid.org/ns/record"
    ///   xmlns:person="http://www.orcid.org/ns/person" xmlns:personal-details="http://www.orcid.org/ns/personal-details">
    ///   <person:person><personal-details:name>
    ///     <personal-details:given-names>Josiah</personal-details:given-names>
    ///     <personal-details:family-name>Carberry</personal-details:family-name>
    ///   </personal-details:name></person:person></record:record>"#;
    /// let t = Tables::from_xml(xml)?;
    /// assert_eq!(t.get(Section::Person).len(), 1);
    /// assert_eq!(t.cell(Section::Person, 0, "orcid").and_then(|c| c.as_str()), Some("0000-0002-1825-0097"));
    /// assert_eq!(t.cell(Section::Person, 0, "family_name").and_then(|c| c.as_str()), Some("Carberry"));
    /// assert!(t.get(Section::Works).is_empty());
    /// # Ok::<(), yorc::Error>(())
    /// ```
    pub fn from_xml(xml: &[u8]) -> crate::Result<Tables> {
        let doc = crate::xml::from_slice(xml)?;
        let hash = crate::hash::sha256_hex(xml);
        Ok(Tables::from_document(&doc, Some(&hash)))
    }

    /// Rows of one section.
    pub fn get(&self, section: Section) -> &[Row] {
        &self.rows[section.index()]
    }

    /// Moves the rows of one section out (leaving none).
    pub fn take(&mut self, section: Section) -> Vec<Row> {
        std::mem::take(&mut self.rows[section.index()])
    }

    /// Value of column `name` in row `row` of `section`.
    pub fn cell(&self, section: Section, row: usize, name: &str) -> Option<&Cell> {
        let i = section.column_index(name)?;
        self.get(section).get(row)?.get(i)
    }
}

/// Rows of one section.
///
/// ```
/// use yorc::rows::{section_rows, Section};
///
/// let doc = yorc::xml::from_str(r#"<record:record path="/0000-0002-1825-0097" xmlns:record="http://www.orcid.org/ns/record"/>"#)?;
/// assert!(section_rows(&doc, Section::Works, None).is_empty());
/// # Ok::<(), yorc::Error>(())
/// ```
pub fn section_rows(doc: &Document, section: Section, source_hash: Option<&str>) -> Vec<Row> {
    Tables::from_document(doc, source_hash).take(section)
}

// ------------------------------------------------------------- row builder

struct RowBuilder {
    columns: &'static [Column],
    cells: Vec<Cell>,
}

impl RowBuilder {
    fn new(section: Section) -> RowBuilder {
        let columns = section.columns();
        RowBuilder {
            columns,
            cells: vec![Cell::Null; columns.len()],
        }
    }

    /// Sets a column this section must have.
    fn set(&mut self, name: &str, cell: Cell) {
        let found = self.try_set(name, cell);
        debug_assert!(found, "no column {name}");
    }

    /// Sets a column if this section has it; `false` if it does not.
    fn try_set(&mut self, name: &str, cell: Cell) -> bool {
        match self.columns.iter().position(|c| c.name == name) {
            Some(i) => {
                debug_assert!(cell.fits(self.columns[i].ty), "{name}: {cell:?}");
                self.cells[i] = cell;
                true
            }
            None => false,
        }
    }

    fn s(&mut self, name: &str, v: Option<&str>) {
        self.set(name, varchar(v));
    }

    fn i(&mut self, name: &str, v: Option<i64>) {
        self.set(name, integer(v));
    }

    fn i32(&mut self, name: &str, v: Option<i32>) {
        self.set(name, integer(v.map(i64::from)));
    }

    fn b(&mut self, name: &str, v: Option<bool>) {
        self.set(name, boolean(v));
    }
}

fn varchar(v: Option<&str>) -> Cell {
    match v {
        Some(s) => Cell::Varchar(s.to_string()),
        None => Cell::Null,
    }
}

fn integer(v: Option<i64>) -> Cell {
    match v {
        Some(n) => Cell::Integer(n),
        None => Cell::Null,
    }
}

fn boolean(v: Option<bool>) -> Cell {
    match v {
        Some(b) => Cell::Boolean(b),
        None => Cell::Null,
    }
}

fn id_path(id: Option<&OrcidIdentifier>) -> Option<&str> {
    id.and_then(|i| i.path.as_deref())
}

fn vis(v: Option<&Visibility>) -> Option<&str> {
    v.map(|v| v.as_str())
}

/// `put_code`, `display_index`, `visibility`, `path`, `created`,
/// `last_modified` and the source columns, where the section has them.
fn meta_cols(rb: &mut RowBuilder, m: &ItemMeta) {
    rb.try_set("put_code", integer(m.put_code));
    rb.try_set("display_index", integer(m.display_index.map(i64::from)));
    rb.try_set("visibility", varchar(vis(m.visibility.as_ref())));
    rb.try_set("path", varchar(m.path.as_deref()));
    rb.try_set("created", varchar(m.created_date.as_deref()));
    rb.try_set("last_modified", varchar(m.last_modified_date.as_deref()));
    source_cols(rb, m.source.as_ref());
}

fn source_cols(rb: &mut RowBuilder, s: Option<&Source>) {
    let Some(s) = s else {
        return;
    };
    rb.try_set("source_name", varchar(s.source_name.as_deref()));
    rb.try_set(
        "source_client_id",
        varchar(id_path(s.source_client_id.as_ref())),
    );
    rb.try_set("source_orcid", varchar(id_path(s.source_orcid.as_ref())));
    rb.try_set(
        "assertion_origin_name",
        varchar(s.assertion_origin_name.as_deref()),
    );
    rb.try_set(
        "assertion_origin_orcid",
        varchar(id_path(s.assertion_origin_orcid.as_ref())),
    );
    rb.try_set(
        "assertion_origin_client_id",
        varchar(id_path(s.assertion_origin_client_id.as_ref())),
    );
}

/// `normalized`, `normalized_error`, `normalized_error_code` of an external
/// id (ORCID's transient normalization).
fn normalized_cols(rb: &mut RowBuilder, x: &ExternalId) {
    rb.s(
        "normalized",
        x.normalized.as_ref().and_then(|n| n.value.as_deref()),
    );
    let err = x.normalized_error.as_ref();
    rb.s(
        "normalized_error",
        err.and_then(|n| n.error_message.as_deref()),
    );
    rb.s(
        "normalized_error_code",
        err.and_then(|n| n.error_code.as_deref()),
    );
}

/// `<prefix>_year`, `<prefix>_month`, `<prefix>_day`.
fn date_cols(rb: &mut RowBuilder, prefix: &str, d: Option<&FuzzyDate>) {
    rb.i32(&format!("{prefix}_year"), d.and_then(|d| d.year));
    rb.i32(&format!("{prefix}_month"), d.and_then(|d| d.month));
    rb.i32(&format!("{prefix}_day"), d.and_then(|d| d.day));
}

/// First non-empty value of a field across organizations (one organization,
/// or research-resource hosts).
fn first_org<'a, F>(orgs: &'a [Organization], f: F) -> Option<&'a str>
where
    F: Fn(&'a Organization) -> Option<&'a str>,
{
    orgs.iter().find_map(f)
}

/// `<prefix>_name`, `_city`, `_region`, `_country`, `_disambiguated_id`,
/// `_disambiguation_source`.
fn org_cols(rb: &mut RowBuilder, prefix: &str, orgs: &[Organization]) {
    rb.s(
        &format!("{prefix}_name"),
        first_org(orgs, |o| o.name.as_deref()),
    );
    rb.s(
        &format!("{prefix}_city"),
        first_org(orgs, |o| o.address.as_ref().and_then(|a| a.city.as_deref())),
    );
    rb.s(
        &format!("{prefix}_region"),
        first_org(orgs, |o| {
            o.address.as_ref().and_then(|a| a.region.as_deref())
        }),
    );
    rb.s(
        &format!("{prefix}_country"),
        first_org(orgs, |o| {
            o.address.as_ref().and_then(|a| a.country.as_deref())
        }),
    );
    rb.s(
        &format!("{prefix}_disambiguated_id"),
        first_org(orgs, |o| {
            o.disambiguated_organization
                .as_ref()
                .and_then(|d| d.identifier.as_deref())
        }),
    );
    rb.s(
        &format!("{prefix}_disambiguation_source"),
        first_org(orgs, |o| {
            o.disambiguated_organization
                .as_ref()
                .and_then(|d| d.source.as_deref())
        }),
    );
}

fn one_org(o: Option<&Organization>) -> &[Organization] {
    match o {
        Some(o) => std::slice::from_ref(o),
        None => &[],
    }
}

/// `doi` of a work: first `doi`/`self` external id, ASCII-lowercased.
fn work_doi(ids: &[ExternalId]) -> Option<String> {
    ids.iter().find_map(|x| {
        let is_doi = x.id_type.as_deref() == Some("doi");
        let is_self = x.relationship.as_ref().map(|r| r.as_str()) == Some("self");
        match (&x.value, is_doi && is_self) {
            (Some(v), true) => Some(v.to_ascii_lowercase()),
            _ => None,
        }
    })
}

/// Owner of nested rows (contributors, external ids).
#[derive(Clone, Copy)]
struct Owner {
    name: Option<&'static str>,
    put_code: Option<i64>,
}

impl Owner {
    const NONE: Owner = Owner {
        name: None,
        put_code: None,
    };

    fn group(name: &'static str) -> Owner {
        Owner {
            name: Some(name),
            put_code: None,
        }
    }

    /// The item itself if it has a put-code, else `fallback` (a group).
    fn item(name: &'static str, put_code: Option<i64>, fallback: Owner) -> Owner {
        if put_code.is_some() {
            Owner {
                name: Some(name),
                put_code,
            }
        } else {
            fallback
        }
    }
}

/// Fields shared by `work` and `work-summary`.
struct WorkView<'a> {
    meta: &'a ItemMeta,
    title: Option<&'a Title>,
    journal_title: Option<&'a str>,
    work_type: Option<&'a WorkType>,
    publication_date: Option<&'a FuzzyDate>,
    url: Option<&'a str>,
    external_ids: &'a [ExternalId],
    short_description: Option<&'a str>,
    citation: Option<&'a Citation>,
    language_code: Option<&'a str>,
    country: Option<&'a str>,
}

/// Fields shared by `funding` and `funding-summary`.
struct FundingView<'a> {
    meta: &'a ItemMeta,
    title: Option<&'a Title>,
    funding_type: Option<&'a FundingType>,
    organization_defined_type: Option<&'a str>,
    short_description: Option<&'a str>,
    amount: Option<&'a Amount>,
    start_date: Option<&'a FuzzyDate>,
    end_date: Option<&'a FuzzyDate>,
    organization: Option<&'a Organization>,
    url: Option<&'a str>,
}

/// Fields shared by `peer-review` and `peer-review-summary`.
struct PeerReviewView<'a> {
    meta: &'a ItemMeta,
    reviewer_role: Option<&'a PeerReviewRole>,
    review_type: Option<&'a PeerReviewType>,
    review_url: Option<&'a str>,
    completion_date: Option<&'a FuzzyDate>,
    review_group_id: Option<&'a str>,
    subject_type: Option<&'a str>,
    subject_name: Option<&'a Title>,
    subject_container_name: Option<&'a str>,
    subject_url: Option<&'a str>,
    subject_external_identifier: Option<&'a ExternalId>,
    convening_organization: Option<&'a Organization>,
}

// ------------------------------------------------------------------ walker

struct Walker<'d> {
    doc_orcid: Option<&'d str>,
    /// The record's `orcid-identifier` (record documents only).
    record_id: Option<&'d OrcidIdentifier>,
    hash: Option<&'d str>,
    tables: Tables,
}

impl<'d> Walker<'d> {
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

    fn push(&mut self, section: Section, mut rb: RowBuilder) {
        rb.s("source_hash", self.hash);
        self.tables.rows[section.index()].push(rb.cells);
    }

    fn document(&mut self, doc: &Document) {
        match &doc.root {
            Root::Record(r) => {
                if let Some(h) = &r.history {
                    self.history(h);
                }
                if let Some(p) = &r.person {
                    self.person(p);
                }
                if let Some(a) = &r.activities_summary {
                    self.activities(a);
                }
            }
            Root::ActivitiesSummary(a) => self.activities(a),
            Root::Works(w) => self.works(w),
            Root::Work(w) => self.work(w, None, Owner::NONE),
            Root::WorkSummary(w) => self.work_summary(w, None, Owner::NONE),
            Root::Fundings(f) => self.fundings(f),
            Root::Funding(f) => self.funding(f, None, Owner::NONE),
            Root::FundingSummary(f) => self.funding_summary(f, None, Owner::NONE),
            Root::Affiliations(a) => self.affiliations(a),
            Root::Affiliation(a) => self.affiliation(a, a.kind.as_str(), None, Owner::NONE),
            Root::AffiliationSummary(a) => {
                self.affiliation(a, a.kind.summary_element(), None, Owner::NONE)
            }
            Root::PeerReviews(p) => self.peer_reviews(p),
            Root::PeerReview(p) => self.peer_review(p),
            Root::PeerReviewSummary(p) => self.peer_review_summary(p, None, Owner::NONE),
            Root::ResearchResources(r) => self.research_resources(r),
            Root::ResearchResource(r) => self.research_resource(r),
            Root::ResearchResourceSummary(r) => {
                self.research_resource_summary(r, None, Owner::NONE)
            }
            Root::Person(p) => self.person(p),
            Root::PersonalDetails(p) => self.personal_details(p),
            Root::OtherNames(l) => {
                for x in &l.items {
                    self.other_name(x);
                }
            }
            Root::OtherName(x) => self.other_name(x),
            Root::ResearcherUrls(l) => {
                for x in &l.items {
                    self.researcher_url(x);
                }
            }
            Root::ResearcherUrl(x) => self.researcher_url(x),
            Root::Emails(l) => {
                for x in &l.items {
                    self.email(x);
                }
            }
            Root::Email(x) => self.email(x),
            Root::Addresses(l) => {
                for x in &l.items {
                    self.address(x);
                }
            }
            Root::Address(x) => self.address(x),
            Root::Keywords(l) => {
                for x in &l.items {
                    self.keyword(x);
                }
            }
            Root::Keyword(x) => self.keyword(x),
            Root::ExternalIdentifiers(l) => {
                for x in &l.items {
                    self.person_external_identifier(x);
                }
            }
            Root::ExternalIdentifier(x) => self.person_external_identifier(x),
            Root::History(h) => self.history(h),
            Root::Bulk(b) => {
                for (i, item) in b.items.iter().enumerate() {
                    match item {
                        BulkItem::Work(w) => self.work(w, None, Owner::NONE),
                        BulkItem::Error(e) => self.error(e, Some("bulk"), i as i64 + 1),
                    }
                }
            }
            Root::Error(e) => self.error(e, None, 1),
            Root::Search(s) => self.search(s),
            Root::ExpandedSearch(s) => self.expanded_search(s),
            // group ids: `group_ids` rows (`Extra::GroupIds`, extra.rs)
            Root::GroupIdRecord(_)
            | Root::GroupIdRecords(_)
            | Root::Name(_)
            | Root::Biography(_)
            | Root::CreditName(_)
            | Root::Preferences(_)
            | Root::Unknown(_) => {}
        }
    }

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

    // ------------------------------------------------------- external ids

    fn external_ids(
        &mut self,
        ids: &[ExternalId],
        owner: Owner,
        group: Option<i64>,
        container: &str,
    ) {
        for x in ids {
            self.external_id(x, owner, group, container);
        }
    }

    fn external_id(&mut self, x: &ExternalId, owner: Owner, group: Option<i64>, container: &str) {
        let mut rb = RowBuilder::new(Section::ExternalIds);
        rb.set(
            "orcid",
            varchar(self.orcid(x.meta.path.as_deref()).as_deref()),
        );
        rb.s("owner", owner.name);
        rb.i("owner_put_code", owner.put_code);
        rb.i("group_index", group);
        rb.s("container", Some(container));
        rb.s("type", x.id_type.as_deref());
        rb.s("value", x.value.as_deref());
        normalized_cols(&mut rb, x);
        rb.s("url", x.url.as_deref());
        rb.s("relationship", x.relationship.as_ref().map(|r| r.as_str()));
        self.push(Section::ExternalIds, rb);
    }

    fn contributors(&mut self, list: &[Contributor], owner: Owner) {
        for (i, c) in list.iter().enumerate() {
            let mut rb = RowBuilder::new(Section::Contributors);
            rb.set("orcid", varchar(self.orcid(None).as_deref()));
            rb.s("owner", owner.name);
            rb.i("owner_put_code", owner.put_code);
            rb.i("ordinal", Some(i as i64 + 1));
            rb.s("contributor_orcid", id_path(c.contributor_orcid.as_ref()));
            rb.s(
                "contributor_orcid_uri",
                c.contributor_orcid.as_ref().and_then(|o| o.uri.as_deref()),
            );
            rb.s(
                "contributor_orcid_host",
                c.contributor_orcid.as_ref().and_then(|o| o.host.as_deref()),
            );
            rb.s("credit_name", c.credit_name.as_deref());
            rb.s("email", c.contributor_email.as_deref());
            let attrs = c.attributes.as_ref();
            rb.s(
                "sequence",
                attrs.and_then(|a| a.sequence.as_ref()).map(|s| s.as_str()),
            );
            rb.s(
                "role",
                attrs.and_then(|a| a.role.as_ref()).map(|r| r.as_str()),
            );
            self.push(Section::Contributors, rb);
        }
    }

    // -------------------------------------------------------------- works

    fn works(&mut self, w: &Works) {
        for (i, g) in w.groups.iter().enumerate() {
            let gi = Some(i as i64 + 1);
            self.external_ids(&g.external_ids, Owner::group("group"), gi, "external-ids");
            for s in &g.work_summaries {
                self.work_summary(s, gi, Owner::group("group"));
            }
        }
    }

    fn work_summary(&mut self, w: &WorkSummary, group: Option<i64>, fallback: Owner) {
        self.work_row(
            "work-summary",
            group,
            WorkView {
                meta: &w.meta,
                title: w.title.as_ref(),
                journal_title: w.journal_title.as_deref(),
                work_type: w.work_type.as_ref(),
                publication_date: w.publication_date.as_ref(),
                url: w.url.as_deref(),
                external_ids: &w.external_ids,
                short_description: None,
                citation: None,
                language_code: None,
                country: None,
            },
        );
        let owner = Owner::item("work-summary", w.meta.put_code, fallback);
        self.external_ids(&w.external_ids, owner, group, "external-ids");
    }

    fn work(&mut self, w: &Work, group: Option<i64>, fallback: Owner) {
        self.work_row(
            "work",
            group,
            WorkView {
                meta: &w.meta,
                title: w.title.as_ref(),
                journal_title: w.journal_title.as_deref(),
                work_type: w.work_type.as_ref(),
                publication_date: w.publication_date.as_ref(),
                url: w.url.as_deref(),
                external_ids: &w.external_ids,
                short_description: w.short_description.as_deref(),
                citation: w.citation.as_ref(),
                language_code: w.language_code.as_deref(),
                country: w.country.as_ref().and_then(|c| c.value.as_deref()),
            },
        );
        let owner = Owner::item("work", w.meta.put_code, fallback);
        self.external_ids(&w.external_ids, owner, group, "external-ids");
        self.contributors(&w.contributors, owner);
    }

    fn work_row(&mut self, item: &'static str, group: Option<i64>, w: WorkView<'_>) {
        let mut rb = RowBuilder::new(Section::Works);
        rb.set(
            "orcid",
            varchar(self.orcid(w.meta.path.as_deref()).as_deref()),
        );
        meta_cols(&mut rb, w.meta);
        rb.i("group_index", group);
        rb.s("title", w.title.and_then(|t| t.title.as_deref()));
        rb.s("subtitle", w.title.and_then(|t| t.subtitle.as_deref()));
        let tt = w.title.and_then(|t| t.translated_title.as_ref());
        rb.s("translated_title", tt.and_then(|t| t.value.as_deref()));
        rb.s(
            "translated_title_language",
            tt.and_then(|t| t.language_code.as_deref()),
        );
        rb.s("journal", w.journal_title);
        rb.s("type", w.work_type.map(|t| t.as_str()));
        date_cols(&mut rb, "pub", w.publication_date);
        rb.s("doi", work_doi(w.external_ids).as_deref());
        rb.s("url", w.url);
        rb.s("short_description", w.short_description);
        rb.s(
            "citation_type",
            w.citation
                .and_then(|c| c.citation_type.as_ref())
                .map(|t| t.as_str()),
        );
        rb.s(
            "citation_value",
            w.citation.and_then(|c| c.citation_value.as_deref()),
        );
        rb.s("language_code", w.language_code);
        rb.s("country", w.country);
        rb.s("item", Some(item));
        self.push(Section::Works, rb);
    }

    // ----------------------------------------------------------- fundings

    fn fundings(&mut self, f: &Fundings) {
        for (i, g) in f.groups.iter().enumerate() {
            let gi = Some(i as i64 + 1);
            self.external_ids(&g.external_ids, Owner::group("group"), gi, "external-ids");
            for s in &g.funding_summaries {
                self.funding_summary(s, gi, Owner::group("group"));
            }
        }
    }

    fn funding_summary(&mut self, f: &FundingSummary, group: Option<i64>, fallback: Owner) {
        self.funding_row(
            "funding-summary",
            group,
            FundingView {
                meta: &f.meta,
                title: f.title.as_ref(),
                funding_type: f.funding_type.as_ref(),
                organization_defined_type: None,
                short_description: None,
                amount: None,
                start_date: f.start_date.as_ref(),
                end_date: f.end_date.as_ref(),
                organization: f.organization.as_ref(),
                url: f.url.as_deref(),
            },
        );
        let owner = Owner::item("funding-summary", f.meta.put_code, fallback);
        self.external_ids(&f.external_ids, owner, group, "external-ids");
    }

    fn funding(&mut self, f: &Funding, group: Option<i64>, fallback: Owner) {
        self.funding_row(
            "funding",
            group,
            FundingView {
                meta: &f.meta,
                title: f.title.as_ref(),
                funding_type: f.funding_type.as_ref(),
                organization_defined_type: f.organization_defined_type.as_deref(),
                short_description: f.short_description.as_deref(),
                amount: f.amount.as_ref(),
                start_date: f.start_date.as_ref(),
                end_date: f.end_date.as_ref(),
                organization: f.organization.as_ref(),
                url: f.url.as_deref(),
            },
        );
        let owner = Owner::item("funding", f.meta.put_code, fallback);
        self.external_ids(&f.external_ids, owner, group, "external-ids");
        self.contributors(&f.contributors, owner);
    }

    fn funding_row(&mut self, item: &'static str, group: Option<i64>, f: FundingView<'_>) {
        let mut rb = RowBuilder::new(Section::Fundings);
        rb.set(
            "orcid",
            varchar(self.orcid(f.meta.path.as_deref()).as_deref()),
        );
        meta_cols(&mut rb, f.meta);
        rb.i("group_index", group);
        rb.s("title", f.title.and_then(|t| t.title.as_deref()));
        let tt = f.title.and_then(|t| t.translated_title.as_ref());
        rb.s("translated_title", tt.and_then(|t| t.value.as_deref()));
        rb.s(
            "translated_title_language",
            tt.and_then(|t| t.language_code.as_deref()),
        );
        rb.s("type", f.funding_type.map(|t| t.as_str()));
        rb.s("organization_defined_type", f.organization_defined_type);
        rb.s("short_description", f.short_description);
        rb.s("amount", f.amount.and_then(|a| a.value.as_deref()));
        rb.s(
            "currency",
            f.amount.and_then(|a| a.currency_code.as_deref()),
        );
        date_cols(&mut rb, "start", f.start_date);
        date_cols(&mut rb, "end", f.end_date);
        org_cols(&mut rb, "org", one_org(f.organization));
        rb.s("url", f.url);
        rb.s("item", Some(item));
        self.push(Section::Fundings, rb);
    }

    // ------------------------------------------------------- affiliations

    fn affiliations(&mut self, list: &Affiliations) {
        for (i, g) in list.groups.iter().enumerate() {
            let gi = Some(i as i64 + 1);
            self.external_ids(
                &g.external_ids,
                Owner::group("affiliation-group"),
                gi,
                "external-ids",
            );
            for a in &g.summaries {
                self.affiliation(
                    a,
                    a.kind.summary_element(),
                    gi,
                    Owner::group("affiliation-group"),
                );
            }
        }
    }

    fn affiliation(
        &mut self,
        a: &Affiliation,
        item: &'static str,
        group: Option<i64>,
        fallback: Owner,
    ) {
        let mut rb = RowBuilder::new(Section::Affiliations);
        rb.set(
            "orcid",
            varchar(self.orcid(a.meta.path.as_deref()).as_deref()),
        );
        rb.s("affiliation_type", Some(a.kind.as_str()));
        meta_cols(&mut rb, &a.meta);
        rb.i("group_index", group);
        rb.s("department", a.department_name.as_deref());
        rb.s("role_title", a.role_title.as_deref());
        date_cols(&mut rb, "start", a.start_date.as_ref());
        date_cols(&mut rb, "end", a.end_date.as_ref());
        org_cols(&mut rb, "org", one_org(a.organization.as_ref()));
        rb.s("url", a.url.as_deref());
        rb.s("item", Some(item));
        self.push(Section::Affiliations, rb);
        let owner = Owner::item(item, a.meta.put_code, fallback);
        self.external_ids(&a.external_ids, owner, group, "external-ids");
    }

    // ------------------------------------------------------- peer reviews

    fn peer_reviews(&mut self, p: &PeerReviews) {
        for (i, g) in p.groups.iter().enumerate() {
            let gi = Some(i as i64 + 1);
            self.external_ids(&g.external_ids, Owner::group("group"), gi, "external-ids");
            for d in &g.duplicates {
                self.external_ids(
                    &d.external_ids,
                    Owner::group("peer-review-group"),
                    gi,
                    "external-ids",
                );
                for s in &d.summaries {
                    self.peer_review_summary(s, gi, Owner::group("peer-review-group"));
                }
            }
        }
    }

    fn peer_review_summary(&mut self, p: &PeerReviewSummary, group: Option<i64>, fallback: Owner) {
        self.peer_review_row(
            "peer-review-summary",
            group,
            PeerReviewView {
                meta: &p.meta,
                reviewer_role: p.reviewer_role.as_ref(),
                review_type: p.review_type.as_ref(),
                review_url: p.review_url.as_deref(),
                completion_date: p.completion_date.as_ref(),
                review_group_id: p.review_group_id.as_deref(),
                subject_type: None,
                subject_name: None,
                subject_container_name: None,
                subject_url: None,
                subject_external_identifier: None,
                convening_organization: p.convening_organization.as_ref(),
            },
        );
        let owner = Owner::item("peer-review-summary", p.meta.put_code, fallback);
        self.external_ids(&p.external_ids, owner, group, "external-ids");
    }

    fn peer_review(&mut self, p: &PeerReview) {
        self.peer_review_row(
            "peer-review",
            None,
            PeerReviewView {
                meta: &p.meta,
                reviewer_role: p.reviewer_role.as_ref(),
                review_type: p.review_type.as_ref(),
                review_url: p.review_url.as_deref(),
                completion_date: p.review_completion_date.as_ref(),
                review_group_id: p.review_group_id.as_deref(),
                subject_type: p.subject_type.as_deref(),
                subject_name: p.subject_name.as_ref(),
                subject_container_name: p.subject_container_name.as_deref(),
                subject_url: p.subject_url.as_deref(),
                subject_external_identifier: p.subject_external_identifier.as_ref(),
                convening_organization: p.convening_organization.as_ref(),
            },
        );
        let owner = Owner::item("peer-review", p.meta.put_code, Owner::NONE);
        self.external_ids(&p.review_identifiers, owner, None, "review-identifiers");
        if let Some(x) = &p.subject_external_identifier {
            self.external_id(x, owner, None, "peer-review");
        }
    }

    fn peer_review_row(&mut self, item: &'static str, group: Option<i64>, p: PeerReviewView<'_>) {
        let mut rb = RowBuilder::new(Section::PeerReviews);
        rb.set(
            "orcid",
            varchar(self.orcid(p.meta.path.as_deref()).as_deref()),
        );
        meta_cols(&mut rb, p.meta);
        rb.i("group_index", group);
        rb.s("reviewer_role", p.reviewer_role.map(|r| r.as_str()));
        rb.s("review_type", p.review_type.map(|t| t.as_str()));
        rb.s("review_url", p.review_url);
        date_cols(&mut rb, "completion", p.completion_date);
        rb.s("review_group_id", p.review_group_id);
        rb.s("subject_type", p.subject_type);
        rb.s(
            "subject_name",
            p.subject_name.and_then(|t| t.title.as_deref()),
        );
        rb.s(
            "subject_subtitle",
            p.subject_name.and_then(|t| t.subtitle.as_deref()),
        );
        rb.s(
            "subject_translated_name",
            p.subject_name
                .and_then(|t| t.translated_title.as_ref())
                .and_then(|t| t.value.as_deref()),
        );
        rb.s(
            "subject_translated_name_language",
            p.subject_name
                .and_then(|t| t.translated_title.as_ref())
                .and_then(|t| t.language_code.as_deref()),
        );
        rb.s("subject_container_name", p.subject_container_name);
        rb.s("subject_url", p.subject_url);
        rb.s(
            "subject_id_type",
            p.subject_external_identifier
                .and_then(|x| x.id_type.as_deref()),
        );
        rb.s(
            "subject_id_value",
            p.subject_external_identifier
                .and_then(|x| x.value.as_deref()),
        );
        org_cols(&mut rb, "convening_org", one_org(p.convening_organization));
        rb.s("item", Some(item));
        self.push(Section::PeerReviews, rb);
    }

    // ------------------------------------------------- research resources

    fn research_resources(&mut self, r: &ResearchResources) {
        for (i, g) in r.groups.iter().enumerate() {
            let gi = Some(i as i64 + 1);
            self.external_ids(&g.external_ids, Owner::group("group"), gi, "external-ids");
            for s in &g.summaries {
                self.research_resource_summary(s, gi, Owner::group("group"));
            }
        }
    }

    fn research_resource_summary(
        &mut self,
        r: &ResearchResourceSummary,
        group: Option<i64>,
        fallback: Owner,
    ) {
        let item = "research-resource-summary";
        self.research_resource_row(item, group, &r.meta, r.proposal.as_ref());
        if let Some(p) = &r.proposal {
            let owner = Owner::item(item, r.meta.put_code, fallback);
            self.external_ids(&p.external_ids, owner, group, "external-ids");
            self.hosts(&p.hosts, r.meta.put_code, "proposal", None);
        }
    }

    fn research_resource(&mut self, r: &ResearchResource) {
        let item = "research-resource";
        self.research_resource_row(item, None, &r.meta, r.proposal.as_ref());
        let owner = Owner::item(item, r.meta.put_code, Owner::NONE);
        if let Some(p) = &r.proposal {
            self.external_ids(&p.external_ids, owner, None, "external-ids");
            self.hosts(&p.hosts, owner.put_code, "proposal", None);
        }
        for (i, ri) in r.resource_items.iter().enumerate() {
            let mut rb = RowBuilder::new(Section::ResearchResourceItems);
            rb.set("orcid", varchar(self.orcid(None).as_deref()));
            rb.i("owner_put_code", owner.put_code);
            rb.i("ordinal", Some(i as i64 + 1));
            rb.s("resource_name", ri.resource_name.as_deref());
            rb.s(
                "resource_type",
                ri.resource_type.as_ref().map(|t| t.as_str()),
            );
            org_cols(&mut rb, "host", &ri.hosts);
            rb.s("url", ri.url.as_deref());
            self.push(Section::ResearchResourceItems, rb);
            self.external_ids(&ri.external_ids, owner, None, "external-ids");
            let ordinal = Some(i as i64 + 1);
            self.hosts(&ri.hosts, owner.put_code, "resource-item", ordinal);
        }
    }

    /// `research_resource_hosts`: one row per host, in document order.
    /// `container` is the element holding `hosts` (`proposal`,
    /// `resource-item`), `item_ordinal` the resource item's position.
    fn hosts(
        &mut self,
        hosts: &[Organization],
        put_code: Option<i64>,
        container: &'static str,
        item_ordinal: Option<i64>,
    ) {
        for (i, o) in hosts.iter().enumerate() {
            let mut rb = RowBuilder::new(Section::ResearchResourceHosts);
            rb.set("orcid", varchar(self.orcid(None).as_deref()));
            rb.i("owner_put_code", put_code);
            rb.i("ordinal", Some(i as i64 + 1));
            org_cols(&mut rb, "host", std::slice::from_ref(o));
            rb.s("container", Some(container));
            rb.i("item_ordinal", item_ordinal);
            self.push(Section::ResearchResourceHosts, rb);
        }
    }

    fn research_resource_row(
        &mut self,
        item: &'static str,
        group: Option<i64>,
        meta: &ItemMeta,
        proposal: Option<&Proposal>,
    ) {
        let mut rb = RowBuilder::new(Section::ResearchResources);
        rb.set(
            "orcid",
            varchar(self.orcid(meta.path.as_deref()).as_deref()),
        );
        meta_cols(&mut rb, meta);
        rb.i("group_index", group);
        let title = proposal.and_then(|p| p.title.as_ref());
        rb.s("title", title.and_then(|t| t.title.as_deref()));
        let tt = title.and_then(|t| t.translated_title.as_ref());
        rb.s("translated_title", tt.and_then(|t| t.value.as_deref()));
        rb.s(
            "translated_title_language",
            tt.and_then(|t| t.language_code.as_deref()),
        );
        let hosts: &[Organization] = match proposal {
            Some(p) => &p.hosts,
            None => &[],
        };
        org_cols(&mut rb, "host", hosts);
        date_cols(
            &mut rb,
            "start",
            proposal.and_then(|p| p.start_date.as_ref()),
        );
        date_cols(&mut rb, "end", proposal.and_then(|p| p.end_date.as_ref()));
        rb.s("url", proposal.and_then(|p| p.url.as_deref()));
        rb.s("item", Some(item));
        self.push(Section::ResearchResources, rb);
    }

    // ------------------------------------------------------------- person

    fn person(&mut self, p: &Person) {
        self.person_row(
            "person",
            p.path.as_deref(),
            p.last_modified_date.as_deref(),
            p.name.as_ref(),
            p.biography.as_ref(),
        );
        if let Some(l) = &p.other_names {
            for x in &l.items {
                self.other_name(x);
            }
        }
        if let Some(l) = &p.researcher_urls {
            for x in &l.items {
                self.researcher_url(x);
            }
        }
        if let Some(l) = &p.emails {
            for x in &l.items {
                self.email(x);
            }
        }
        if let Some(l) = &p.addresses {
            for x in &l.items {
                self.address(x);
            }
        }
        if let Some(l) = &p.keywords {
            for x in &l.items {
                self.keyword(x);
            }
        }
        if let Some(l) = &p.external_identifiers {
            for x in &l.items {
                self.person_external_identifier(x);
            }
        }
    }

    fn personal_details(&mut self, p: &PersonalDetails) {
        self.person_row(
            "personal-details",
            p.path.as_deref(),
            p.last_modified_date.as_deref(),
            p.name.as_ref(),
            p.biography.as_ref(),
        );
        if let Some(l) = &p.other_names {
            for x in &l.items {
                self.other_name(x);
            }
        }
    }

    fn person_row(
        &mut self,
        item: &'static str,
        path: Option<&str>,
        last_modified: Option<&str>,
        name: Option<&Name>,
        bio: Option<&Biography>,
    ) {
        let mut rb = RowBuilder::new(Section::Person);
        rb.set("orcid", varchar(self.orcid(path).as_deref()));
        rb.s("given_names", name.and_then(|n| n.given_names.as_deref()));
        rb.s("family_name", name.and_then(|n| n.family_name.as_deref()));
        rb.s("credit_name", name.and_then(|n| n.credit_name.as_deref()));
        rb.s(
            "name_visibility",
            vis(name.and_then(|n| n.visibility.as_ref())),
        );
        rb.s("biography", bio.and_then(|b| b.content.as_deref()));
        rb.s(
            "biography_visibility",
            vis(bio.and_then(|b| b.visibility.as_ref())),
        );
        rb.s("last_modified", last_modified);
        rb.s(
            "name_last_modified",
            name.and_then(|n| n.last_modified_date.as_deref()),
        );
        rb.s(
            "biography_last_modified",
            bio.and_then(|b| b.last_modified_date.as_deref()),
        );
        rb.s("path", path);
        rb.s("item", Some(item));
        rb.s("orcid_uri", self.record_id.and_then(|i| i.uri.as_deref()));
        rb.s("orcid_host", self.record_id.and_then(|i| i.host.as_deref()));
        self.push(Section::Person, rb);
    }

    #[allow(clippy::too_many_arguments)]
    fn person_item_row(
        &mut self,
        kind: &'static str,
        meta: &ItemMeta,
        value: Option<&str>,
        url: Option<&str>,
        id_type: Option<&str>,
        relationship: Option<&str>,
        verified: Option<bool>,
        primary: Option<bool>,
        ext: Option<&ExternalId>,
    ) {
        let mut rb = RowBuilder::new(Section::PersonItems);
        rb.set(
            "orcid",
            varchar(self.orcid(meta.path.as_deref()).as_deref()),
        );
        rb.s("kind", Some(kind));
        meta_cols(&mut rb, meta);
        rb.s("value", value);
        rb.s("url", url);
        rb.s("type", id_type);
        rb.s("relationship", relationship);
        rb.b("verified", verified);
        rb.b("primary", primary);
        if let Some(x) = ext {
            normalized_cols(&mut rb, x);
        }
        self.push(Section::PersonItems, rb);
    }

    fn other_name(&mut self, x: &OtherName) {
        let v = x.content.as_deref();
        self.person_item_row("other-name", &x.meta, v, None, None, None, None, None, None);
    }

    fn researcher_url(&mut self, x: &ResearcherUrl) {
        let (v, u) = (x.url_name.as_deref(), x.url.as_deref());
        self.person_item_row(
            "researcher-url",
            &x.meta,
            v,
            u,
            None,
            None,
            None,
            None,
            None,
        );
    }

    fn email(&mut self, x: &Email) {
        let v = x.email.as_deref();
        let (ver, pri) = (x.verified, x.primary);
        self.person_item_row("email", &x.meta, v, None, None, None, ver, pri, None);
    }

    fn address(&mut self, x: &Address) {
        let v = x.country.as_deref();
        self.person_item_row("address", &x.meta, v, None, None, None, None, None, None);
    }

    fn keyword(&mut self, x: &Keyword) {
        let v = x.content.as_deref();
        self.person_item_row("keyword", &x.meta, v, None, None, None, None, None, None);
    }

    fn person_external_identifier(&mut self, x: &ExternalId) {
        self.person_item_row(
            "external-identifier",
            &x.meta,
            x.value.as_deref(),
            x.url.as_deref(),
            x.id_type.as_deref(),
            x.relationship.as_ref().map(|r| r.as_str()),
            None,
            None,
            Some(x),
        );
    }

    // ------------------------------------------------------------ history

    fn history(&mut self, h: &History) {
        let mut rb = RowBuilder::new(Section::History);
        rb.set("orcid", varchar(self.orcid(None).as_deref()));
        rb.s(
            "creation_method",
            h.creation_method.as_ref().map(|m| m.as_str()),
        );
        rb.s("completion_date", h.completion_date.as_deref());
        rb.s("submission_date", h.submission_date.as_deref());
        rb.s("last_modified", h.last_modified_date.as_deref());
        rb.b("claimed", h.claimed);
        rb.s("deactivation_date", h.deactivation_date.as_deref());
        rb.b("verified_email", h.verified_email);
        rb.b("verified_primary_email", h.verified_primary_email);
        rb.s("visibility", vis(h.visibility.as_ref()));
        source_cols(&mut rb, h.source.as_ref());
        self.push(Section::History, rb);
    }

    // ----------------------------------------------------- search results

    fn search(&mut self, s: &Search) {
        for (i, id) in s.results.iter().enumerate() {
            let mut rb = RowBuilder::new(Section::SearchResults);
            rb.i("num_found", digits(s.num_found, MAX_BIGINT));
            rb.i("ordinal", Some(i as i64 + 1));
            rb.s("orcid", id.path.as_deref());
            rb.s("orcid_uri", id.uri.as_deref());
            rb.s("orcid_host", id.host.as_deref());
            rb.s("item", Some("result"));
            self.push(Section::SearchResults, rb);
        }
    }

    fn expanded_search(&mut self, s: &ExpandedSearch) {
        for (i, r) in s.results.iter().enumerate() {
            let mut rb = RowBuilder::new(Section::SearchResults);
            rb.i("num_found", digits(s.num_found, MAX_BIGINT));
            rb.i("ordinal", Some(i as i64 + 1));
            rb.s("orcid", r.orcid_id.as_deref());
            rb.s("given_names", r.given_names.as_deref());
            rb.s("family_names", r.family_names.as_deref());
            rb.s("credit_name", r.credit_name.as_deref());
            let (others, emails) = (joined(&r.other_names), joined(&r.emails));
            let institutions = joined(&r.institution_names);
            rb.s("other_names", others.as_deref());
            rb.s("emails", emails.as_deref());
            rb.s("institution_names", institutions.as_deref());
            rb.s("item", Some("expanded-result"));
            self.push(Section::SearchResults, rb);
        }
    }

    // ------------------------------------------------------------- errors

    /// One `errors` row. `container` is the parent element (`bulk`, or
    /// `None` for the document element), `position` the 1-based position
    /// among its siblings.
    fn error(&mut self, e: &OrcidError, container: Option<&'static str>, position: i64) {
        let mut rb = RowBuilder::new(Section::Errors);
        let orcid = self.orcid(None).or_else(|| message_id(e, "ORCID "));
        rb.s("orcid", orcid.as_deref());
        rb.i("response_code", digits(e.response_code, MAX_INTEGER));
        rb.s("developer_message", e.developer_message.as_deref());
        rb.s("user_message", e.user_message.as_deref());
        rb.i("error_code", digits(e.error_code, MAX_INTEGER));
        rb.s("more_info", e.more_info.as_deref());
        rb.s("container", container);
        rb.i("position", Some(position));
        rb.s(
            "primary_orcid",
            message_id(e, "refer to account:").as_deref(),
        );
        self.push(Section::Errors, rb);
    }
}

/// The largest INTEGER column value yorc-duckdb reads: 9 digits.
const MAX_INTEGER: i64 = 999_999_999;
/// The largest BIGINT column value yorc-duckdb reads: 18 digits.
const MAX_BIGINT: i64 = 999_999_999_999_999_999;

/// yorc-duckdb's integer rule on a number the model already holds: 0 to
/// `max` (1 to 9 or 18 ASCII digits), anything else NULL.
fn digits(v: Option<i64>, max: i64) -> Option<i64> {
    v.filter(|n| (0..=max).contains(n))
}

/// Every value joined with `; ` (yorc-duckdb's `JOIN_SEPARATOR`), `None`
/// for none.
fn joined(values: &[String]) -> Option<String> {
    if values.is_empty() {
        None
    } else {
        Some(values.join("; "))
    }
}

/// The first valid iD named after `marker` in the error's
/// `developer-message`, then its `user-message` (yorc-duckdb's
/// `$message_orcid` / `$message_primary_orcid`).
fn message_id(e: &OrcidError, marker: &str) -> Option<String> {
    [e.developer_message.as_deref(), e.user_message.as_deref()]
        .into_iter()
        .flatten()
        .find_map(|text| id_after(text, marker))
}

/// The first valid iD that directly follows `marker` in `text` (bare or as
/// an orcid.org URL; spaces after the marker skipped, trailing `.`, `,`,
/// `;`, `)` and `/` ignored), canonical (yorc-duckdb's `IdAfter`).
fn id_after(text: &str, marker: &str) -> Option<String> {
    let mut from = 0;
    while let Some(found) = text.get(from..).and_then(|t| t.find(marker)) {
        let start = from + found;
        let rest = text[start + marker.len()..].trim_start_matches(' ');
        let end = rest.find([' ', '\t', '\n', '\r']).unwrap_or(rest.len());
        let token = rest[..end].trim_end_matches(['.', ',', ';', ')', '/']);
        if let Some(id) = crate::id::normalize(token) {
            return Some(id);
        }
        // `marker` starts with an ASCII byte here, so this is a char boundary.
        from = start + 1;
    }
    None
}

// ------------------------------------------------------------------ schema
// Generated from yorc-duckdb/src/core/sections.cpp (kColumns); keep in sync.

const WORKS: &[Column] = &[
    col("orcid", ColType::Varchar),
    col("put_code", ColType::Bigint),
    col("group_index", ColType::Integer),
    col("display_index", ColType::Integer),
    col("title", ColType::Varchar),
    col("subtitle", ColType::Varchar),
    col("journal", ColType::Varchar),
    col("type", ColType::Varchar),
    col("pub_year", ColType::Integer),
    col("pub_month", ColType::Integer),
    col("pub_day", ColType::Integer),
    col("doi", ColType::Varchar),
    col("url", ColType::Varchar),
    col("source_name", ColType::Varchar),
    col("visibility", ColType::Varchar),
    col("created", ColType::Varchar),
    col("last_modified", ColType::Varchar),
    col("source_hash", ColType::Varchar),
    col("translated_title", ColType::Varchar),
    col("translated_title_language", ColType::Varchar),
    col("short_description", ColType::Varchar),
    col("citation_type", ColType::Varchar),
    col("citation_value", ColType::Varchar),
    col("language_code", ColType::Varchar),
    col("country", ColType::Varchar),
    col("path", ColType::Varchar),
    col("source_client_id", ColType::Varchar),
    col("source_orcid", ColType::Varchar),
    col("assertion_origin_name", ColType::Varchar),
    col("assertion_origin_orcid", ColType::Varchar),
    col("assertion_origin_client_id", ColType::Varchar),
    col("item", ColType::Varchar),
];

const FUNDINGS: &[Column] = &[
    col("orcid", ColType::Varchar),
    col("put_code", ColType::Bigint),
    col("group_index", ColType::Integer),
    col("display_index", ColType::Integer),
    col("title", ColType::Varchar),
    col("translated_title", ColType::Varchar),
    col("translated_title_language", ColType::Varchar),
    col("type", ColType::Varchar),
    col("organization_defined_type", ColType::Varchar),
    col("short_description", ColType::Varchar),
    col("amount", ColType::Varchar),
    col("currency", ColType::Varchar),
    col("start_year", ColType::Integer),
    col("start_month", ColType::Integer),
    col("start_day", ColType::Integer),
    col("end_year", ColType::Integer),
    col("end_month", ColType::Integer),
    col("end_day", ColType::Integer),
    col("org_name", ColType::Varchar),
    col("org_city", ColType::Varchar),
    col("org_region", ColType::Varchar),
    col("org_country", ColType::Varchar),
    col("org_disambiguated_id", ColType::Varchar),
    col("org_disambiguation_source", ColType::Varchar),
    col("url", ColType::Varchar),
    col("visibility", ColType::Varchar),
    col("path", ColType::Varchar),
    col("source_name", ColType::Varchar),
    col("source_client_id", ColType::Varchar),
    col("source_orcid", ColType::Varchar),
    col("assertion_origin_name", ColType::Varchar),
    col("created", ColType::Varchar),
    col("last_modified", ColType::Varchar),
    col("item", ColType::Varchar),
    col("source_hash", ColType::Varchar),
    // Appended, as yorc-duckdb appends them (sections.cpp; CROSSCHECK #10)
    col("assertion_origin_orcid", ColType::Varchar),
    col("assertion_origin_client_id", ColType::Varchar),
];

const AFFILIATIONS: &[Column] = &[
    col("orcid", ColType::Varchar),
    col("affiliation_type", ColType::Varchar),
    col("put_code", ColType::Bigint),
    col("group_index", ColType::Integer),
    col("display_index", ColType::Integer),
    col("department", ColType::Varchar),
    col("role_title", ColType::Varchar),
    col("start_year", ColType::Integer),
    col("start_month", ColType::Integer),
    col("start_day", ColType::Integer),
    col("end_year", ColType::Integer),
    col("end_month", ColType::Integer),
    col("end_day", ColType::Integer),
    col("org_name", ColType::Varchar),
    col("org_city", ColType::Varchar),
    col("org_region", ColType::Varchar),
    col("org_country", ColType::Varchar),
    col("org_disambiguated_id", ColType::Varchar),
    col("org_disambiguation_source", ColType::Varchar),
    col("url", ColType::Varchar),
    col("visibility", ColType::Varchar),
    col("path", ColType::Varchar),
    col("source_name", ColType::Varchar),
    col("source_client_id", ColType::Varchar),
    col("source_orcid", ColType::Varchar),
    col("assertion_origin_name", ColType::Varchar),
    col("created", ColType::Varchar),
    col("last_modified", ColType::Varchar),
    col("item", ColType::Varchar),
    col("source_hash", ColType::Varchar),
    // Appended, as yorc-duckdb appends them (sections.cpp; CROSSCHECK #10)
    col("assertion_origin_orcid", ColType::Varchar),
    col("assertion_origin_client_id", ColType::Varchar),
];

const PEER_REVIEWS: &[Column] = &[
    col("orcid", ColType::Varchar),
    col("put_code", ColType::Bigint),
    col("group_index", ColType::Integer),
    col("display_index", ColType::Integer),
    col("reviewer_role", ColType::Varchar),
    col("review_type", ColType::Varchar),
    col("review_url", ColType::Varchar),
    col("completion_year", ColType::Integer),
    col("completion_month", ColType::Integer),
    col("completion_day", ColType::Integer),
    col("review_group_id", ColType::Varchar),
    col("subject_type", ColType::Varchar),
    col("subject_name", ColType::Varchar),
    col("subject_subtitle", ColType::Varchar),
    col("subject_translated_name", ColType::Varchar),
    col("subject_container_name", ColType::Varchar),
    col("subject_url", ColType::Varchar),
    col("subject_id_type", ColType::Varchar),
    col("subject_id_value", ColType::Varchar),
    col("convening_org_name", ColType::Varchar),
    col("convening_org_city", ColType::Varchar),
    col("convening_org_region", ColType::Varchar),
    col("convening_org_country", ColType::Varchar),
    col("convening_org_disambiguated_id", ColType::Varchar),
    col("convening_org_disambiguation_source", ColType::Varchar),
    col("visibility", ColType::Varchar),
    col("path", ColType::Varchar),
    col("source_name", ColType::Varchar),
    col("source_client_id", ColType::Varchar),
    col("source_orcid", ColType::Varchar),
    col("assertion_origin_name", ColType::Varchar),
    col("created", ColType::Varchar),
    col("last_modified", ColType::Varchar),
    col("item", ColType::Varchar),
    col("source_hash", ColType::Varchar),
    // After source_hash, as yorc-duckdb appends it (sections.cpp)
    col("subject_translated_name_language", ColType::Varchar),
    // Appended, as yorc-duckdb appends them (sections.cpp; CROSSCHECK #10)
    col("assertion_origin_orcid", ColType::Varchar),
    col("assertion_origin_client_id", ColType::Varchar),
];

const RESEARCH_RESOURCES: &[Column] = &[
    col("orcid", ColType::Varchar),
    col("put_code", ColType::Bigint),
    col("group_index", ColType::Integer),
    col("display_index", ColType::Integer),
    col("title", ColType::Varchar),
    col("translated_title", ColType::Varchar),
    col("translated_title_language", ColType::Varchar),
    col("host_name", ColType::Varchar),
    col("host_city", ColType::Varchar),
    col("host_region", ColType::Varchar),
    col("host_country", ColType::Varchar),
    col("host_disambiguated_id", ColType::Varchar),
    col("host_disambiguation_source", ColType::Varchar),
    col("start_year", ColType::Integer),
    col("start_month", ColType::Integer),
    col("start_day", ColType::Integer),
    col("end_year", ColType::Integer),
    col("end_month", ColType::Integer),
    col("end_day", ColType::Integer),
    col("url", ColType::Varchar),
    col("visibility", ColType::Varchar),
    col("path", ColType::Varchar),
    col("source_name", ColType::Varchar),
    col("source_client_id", ColType::Varchar),
    col("source_orcid", ColType::Varchar),
    col("assertion_origin_name", ColType::Varchar),
    col("created", ColType::Varchar),
    col("last_modified", ColType::Varchar),
    col("item", ColType::Varchar),
    col("source_hash", ColType::Varchar),
    // Appended, as yorc-duckdb appends them (sections.cpp; CROSSCHECK #10)
    col("assertion_origin_orcid", ColType::Varchar),
    col("assertion_origin_client_id", ColType::Varchar),
];

const RESEARCH_RESOURCE_ITEMS: &[Column] = &[
    col("orcid", ColType::Varchar),
    col("owner_put_code", ColType::Bigint),
    col("ordinal", ColType::Integer),
    col("resource_name", ColType::Varchar),
    col("resource_type", ColType::Varchar),
    col("host_name", ColType::Varchar),
    col("host_city", ColType::Varchar),
    col("host_region", ColType::Varchar),
    col("host_country", ColType::Varchar),
    col("host_disambiguated_id", ColType::Varchar),
    col("host_disambiguation_source", ColType::Varchar),
    col("url", ColType::Varchar),
    col("source_hash", ColType::Varchar),
];

const CONTRIBUTORS: &[Column] = &[
    col("orcid", ColType::Varchar),
    col("owner", ColType::Varchar),
    col("owner_put_code", ColType::Bigint),
    col("ordinal", ColType::Integer),
    col("contributor_orcid", ColType::Varchar),
    col("contributor_orcid_uri", ColType::Varchar),
    col("credit_name", ColType::Varchar),
    col("email", ColType::Varchar),
    col("sequence", ColType::Varchar),
    col("role", ColType::Varchar),
    col("source_hash", ColType::Varchar),
    // Appended, as yorc-duckdb appends them (sections.cpp; CROSSCHECK #10)
    col("contributor_orcid_host", ColType::Varchar),
];

const EXTERNAL_IDS: &[Column] = &[
    col("orcid", ColType::Varchar),
    col("owner", ColType::Varchar),
    col("owner_put_code", ColType::Bigint),
    col("group_index", ColType::Integer),
    col("container", ColType::Varchar),
    col("type", ColType::Varchar),
    col("value", ColType::Varchar),
    col("normalized", ColType::Varchar),
    col("normalized_error", ColType::Varchar),
    col("url", ColType::Varchar),
    col("relationship", ColType::Varchar),
    col("source_hash", ColType::Varchar),
    // Appended, as yorc-duckdb appends them (sections.cpp; CROSSCHECK #10)
    col("normalized_error_code", ColType::Varchar),
];

const PERSON: &[Column] = &[
    col("orcid", ColType::Varchar),
    col("given_names", ColType::Varchar),
    col("family_name", ColType::Varchar),
    col("credit_name", ColType::Varchar),
    col("name_visibility", ColType::Varchar),
    col("biography", ColType::Varchar),
    col("biography_visibility", ColType::Varchar),
    col("last_modified", ColType::Varchar),
    col("name_last_modified", ColType::Varchar),
    col("biography_last_modified", ColType::Varchar),
    col("path", ColType::Varchar),
    col("item", ColType::Varchar),
    col("source_hash", ColType::Varchar),
    // Appended, as yorc-duckdb appends them (sections.cpp; CROSSCHECK #10)
    col("orcid_uri", ColType::Varchar),
    col("orcid_host", ColType::Varchar),
];

const PERSON_ITEMS: &[Column] = &[
    col("orcid", ColType::Varchar),
    col("kind", ColType::Varchar),
    col("put_code", ColType::Bigint),
    col("display_index", ColType::Integer),
    col("value", ColType::Varchar),
    col("url", ColType::Varchar),
    col("type", ColType::Varchar),
    col("relationship", ColType::Varchar),
    col("verified", ColType::Boolean),
    col("primary", ColType::Boolean),
    col("visibility", ColType::Varchar),
    col("path", ColType::Varchar),
    col("source_name", ColType::Varchar),
    col("source_client_id", ColType::Varchar),
    col("source_orcid", ColType::Varchar),
    col("created", ColType::Varchar),
    col("last_modified", ColType::Varchar),
    col("source_hash", ColType::Varchar),
    // Appended, as yorc-duckdb appends them (sections.cpp; CROSSCHECK #10)
    col("assertion_origin_name", ColType::Varchar),
    col("assertion_origin_orcid", ColType::Varchar),
    col("assertion_origin_client_id", ColType::Varchar),
    col("normalized", ColType::Varchar),
    col("normalized_error", ColType::Varchar),
    col("normalized_error_code", ColType::Varchar),
];

const HISTORY: &[Column] = &[
    col("orcid", ColType::Varchar),
    col("creation_method", ColType::Varchar),
    col("completion_date", ColType::Varchar),
    col("submission_date", ColType::Varchar),
    col("last_modified", ColType::Varchar),
    col("claimed", ColType::Boolean),
    col("deactivation_date", ColType::Varchar),
    col("verified_email", ColType::Boolean),
    col("verified_primary_email", ColType::Boolean),
    col("visibility", ColType::Varchar),
    col("source_name", ColType::Varchar),
    col("source_client_id", ColType::Varchar),
    col("source_orcid", ColType::Varchar),
    col("source_hash", ColType::Varchar),
    // Appended, as yorc-duckdb appends them (sections.cpp; CROSSCHECK #10)
    col("assertion_origin_name", ColType::Varchar),
    col("assertion_origin_orcid", ColType::Varchar),
    col("assertion_origin_client_id", ColType::Varchar),
];

const SEARCH_RESULTS: &[Column] = &[
    col("num_found", ColType::Bigint),
    col("ordinal", ColType::Integer),
    col("orcid", ColType::Varchar),
    col("orcid_uri", ColType::Varchar),
    col("orcid_host", ColType::Varchar),
    col("given_names", ColType::Varchar),
    col("family_names", ColType::Varchar),
    col("credit_name", ColType::Varchar),
    col("other_names", ColType::Varchar),
    col("emails", ColType::Varchar),
    col("institution_names", ColType::Varchar),
    col("item", ColType::Varchar),
    col("source_hash", ColType::Varchar),
];

const ERRORS: &[Column] = &[
    col("orcid", ColType::Varchar),
    col("response_code", ColType::Integer),
    col("developer_message", ColType::Varchar),
    col("user_message", ColType::Varchar),
    col("error_code", ColType::Integer),
    col("more_info", ColType::Varchar),
    col("container", ColType::Varchar),
    col("position", ColType::Integer),
    col("source_hash", ColType::Varchar),
    // After source_hash, as yorc-duckdb appends it (sections.cpp)
    col("primary_orcid", ColType::Varchar),
];

/// CROSSCHECK #9: every host of a proposal or resource item.
const RESEARCH_RESOURCE_HOSTS: &[Column] = &[
    col("orcid", ColType::Varchar),
    col("owner_put_code", ColType::Bigint),
    col("ordinal", ColType::Integer),
    col("host_name", ColType::Varchar),
    col("host_city", ColType::Varchar),
    col("host_region", ColType::Varchar),
    col("host_country", ColType::Varchar),
    col("host_disambiguated_id", ColType::Varchar),
    col("host_disambiguation_source", ColType::Varchar),
    col("container", ColType::Varchar),
    col("item_ordinal", ColType::Integer),
    col("source_hash", ColType::Varchar),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_after_follows_yorc_duckdb() {
        let locked = "409 Conflict: The ORCID record is locked and cannot be edited. \
                      ORCID https://orcid.org/0000-0001-5272-5001";
        // the first "ORCID " is followed by "record": skipped
        assert_eq!(
            id_after(locked, "ORCID ").as_deref(),
            Some("0000-0001-5272-5001")
        );
        let deprecated = "Please refer to account: https://orcid.org/0009-0008-8963-1314.";
        assert_eq!(
            id_after(deprecated, "refer to account:").as_deref(),
            Some("0009-0008-8963-1314")
        );
        // bare, several spaces, trailing punctuation
        assert_eq!(
            id_after("ORCID   0000-0002-1825-0097);", "ORCID ").as_deref(),
            Some("0000-0002-1825-0097")
        );
        // a wrong check digit, no marker, nothing after the marker
        assert_eq!(id_after("ORCID 0000-0002-1825-0098", "ORCID "), None);
        assert_eq!(id_after("no iD here", "ORCID "), None);
        assert_eq!(id_after("ORCID ", "ORCID "), None);
        // non-ASCII text around the marker
        assert_eq!(
            id_after("é ORCID 0000-0002-1825-0097 ü", "ORCID ").as_deref(),
            Some("0000-0002-1825-0097")
        );
    }

    #[test]
    fn integer_columns_follow_yorc_duckdb() {
        assert_eq!(digits(Some(409), MAX_INTEGER), Some(409));
        assert_eq!(digits(Some(-1), MAX_INTEGER), None);
        let big = 1_000_000_000; // 10 digits
        assert_eq!(digits(Some(big), MAX_INTEGER), None);
        assert_eq!(digits(Some(big), MAX_BIGINT), Some(big));
        assert_eq!(joined(&[]), None);
        let two = ["a".to_string(), "b".to_string()];
        assert_eq!(joined(&two).as_deref(), Some("a; b"));
    }
}
