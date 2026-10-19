//! `record-3.0.xsd`, `history-3.0.xsd`, `preferences-3.0.xsd`,
//! `bulk-3.0.xsd`, `error-3.0.xsd`, `search-3.0.xsd`,
//! `expanded-search-3.0.xsd`, and [`Document`]: any of the root elements
//! ORCID returns.

use super::activities::{
    ActivitiesSummary, Affiliation, Affiliations, Funding, FundingSummary, Fundings, PeerReview,
    PeerReviewSummary, PeerReviews, ResearchResource, ResearchResourceSummary, ResearchResources,
    Work, WorkSummary, Works,
};
use super::common::{OrcidIdentifier, Source, Visibility};
use super::group_id::{GroupIdRecord, GroupIdRecords};
use super::person::{
    Address, Addresses, Biography, Email, Emails, ExternalIdentifiers, Keyword, Keywords, Name,
    OtherName, OtherNames, Person, PersonExternalIdentifier, PersonalDetails, ResearcherUrl,
    ResearcherUrls,
};

string_enum! {
    /// `history:creation-method`.
    CreationMethod {
        Api => "API",
        Direct => "Direct",
        MemberReferred => "Member-referred",
        Website => "website",
    }
}

/// `history:history`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct History {
    /// `@visibility`.
    pub visibility: Option<Visibility>,
    /// `creation-method` (`Other(..)` keeps values this version does not know).
    pub creation_method: Option<CreationMethod>,
    /// `completion-date` (`xs:dateTime` text, as ORCID sent it).
    pub completion_date: Option<String>,
    /// `submission-date` (`xs:dateTime` text, as ORCID sent it).
    pub submission_date: Option<String>,
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// `claimed`: the owner has claimed the record.
    pub claimed: Option<bool>,
    /// `common:source`: who created the record.
    pub source: Option<Source>,
    /// `deactivation-date` (`xs:dateTime` text, as ORCID sent it).
    pub deactivation_date: Option<String>,
    /// `verified-email`.
    pub verified_email: Option<bool>,
    /// `verified-primary-email`.
    pub verified_primary_email: Option<bool>,
}

/// `preferences:preferences`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    /// `locale` (e.g. `en`).
    pub locale: Option<String>,
}

/// `record:record`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Record {
    /// `@path`: `/` + the iD.
    pub path: Option<String>,
    /// `common:orcid-identifier`.
    pub orcid_identifier: Option<OrcidIdentifier>,
    /// `preferences:preferences`.
    pub preferences: Option<Preferences>,
    /// `history:history`.
    pub history: Option<History>,
    /// `person:person`.
    pub person: Option<Person>,
    /// `activities:activities-summary`.
    pub activities_summary: Option<ActivitiesSummary>,
}

/// `error:error` (API error body; also a bulk item).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrcidError {
    /// `response-code`: the HTTP status.
    pub response_code: Option<i64>,
    /// `developer-message`.
    pub developer_message: Option<String>,
    /// `user-message`.
    pub user_message: Option<String>,
    /// `error-code`: ORCID's numeric code (e.g. `9016`).
    pub error_code: Option<i64>,
    /// `more-info`: a URL.
    pub more_info: Option<String>,
}

/// One entry of `bulk:bulk`.
// Unboxed on purpose: items are built once per document and matched by
// callers; boxing `Work` would only make construction and patterns clumsier.
#[allow(clippy::large_enum_variant)]
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BulkItem {
    /// `work:work`.
    Work(Work),
    /// `error:error`: this item failed (e.g. no such put-code).
    Error(OrcidError),
}

/// `bulk:bulk` (bulk work read/write).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bulk {
    /// Works and per-item errors, in document order.
    pub items: Vec<BulkItem>,
}

/// `search:search`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Search {
    /// `@num-found`
    pub num_found: Option<i64>,
    /// `result/orcid-identifier`
    pub results: Vec<OrcidIdentifier>,
}

/// `expanded-search:expanded-result`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpandedResult {
    /// `orcid-id`: the bare iD.
    pub orcid_id: Option<String>,
    /// `given-names`.
    pub given_names: Option<String>,
    /// `family-names`.
    pub family_names: Option<String>,
    /// `credit-name`.
    pub credit_name: Option<String>,
    /// `other-name` values.
    pub other_names: Vec<String>,
    /// `email` values (public ones only).
    pub emails: Vec<String>,
    /// `institution-name` values.
    pub institution_names: Vec<String>,
}

/// `expanded-search:expanded-search`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpandedSearch {
    /// `@num-found`
    pub num_found: Option<i64>,
    /// `expanded-result` entries.
    pub results: Vec<ExpandedResult>,
}

/// The root element of an ORCID v3.0 document.
// Unboxed on purpose, as `BulkItem`: one value per parsed document.
#[allow(clippy::large_enum_variant)]
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Root {
    /// `record:record`.
    Record(Record),
    /// `activities:activities-summary`.
    ActivitiesSummary(ActivitiesSummary),
    /// `activities:works`.
    Works(Works),
    /// `work:work`.
    Work(Work),
    /// `work:work-summary`.
    WorkSummary(WorkSummary),
    /// `activities:fundings`.
    Fundings(Fundings),
    /// `funding:funding`.
    Funding(Funding),
    /// `funding:funding-summary`.
    FundingSummary(FundingSummary),
    /// `distinctions`, `educations`, ... (the kind is inside)
    Affiliations(Affiliations),
    /// `distinction`, `education`, ... (full item)
    Affiliation(Affiliation),
    /// `distinction-summary`, ...
    AffiliationSummary(Affiliation),
    /// `activities:peer-reviews`.
    PeerReviews(PeerReviews),
    /// `peer-review:peer-review`.
    PeerReview(PeerReview),
    /// `peer-review:peer-review-summary`.
    PeerReviewSummary(PeerReviewSummary),
    /// `activities:research-resources`.
    ResearchResources(ResearchResources),
    /// `research-resource:research-resource`.
    ResearchResource(ResearchResource),
    /// `research-resource:research-resource-summary`.
    ResearchResourceSummary(ResearchResourceSummary),
    /// `person:person`.
    Person(Person),
    /// `personal-details:personal-details`.
    PersonalDetails(PersonalDetails),
    /// `personal-details:name`.
    Name(Name),
    /// `personal-details:biography`.
    Biography(Biography),
    /// `personal-details:credit-name`
    CreditName(Option<String>),
    /// `other-name:other-names`.
    OtherNames(OtherNames),
    /// `other-name:other-name`.
    OtherName(OtherName),
    /// `researcher-url:researcher-urls`.
    ResearcherUrls(ResearcherUrls),
    /// `researcher-url:researcher-url`.
    ResearcherUrl(ResearcherUrl),
    /// `email:emails`.
    Emails(Emails),
    /// `email:email`.
    Email(Email),
    /// `address:addresses`.
    Addresses(Addresses),
    /// `address:address`.
    Address(Address),
    /// `keyword:keywords`.
    Keywords(Keywords),
    /// `keyword:keyword`.
    Keyword(Keyword),
    /// `external-identifier:external-identifiers`.
    ExternalIdentifiers(ExternalIdentifiers),
    /// `external-identifier:external-identifier`.
    ExternalIdentifier(PersonExternalIdentifier),
    /// `history:history`.
    History(History),
    /// `preferences:preferences`.
    Preferences(Preferences),
    /// `bulk:bulk`.
    Bulk(Bulk),
    /// `error:error`.
    Error(OrcidError),
    /// `search:search`.
    Search(Search),
    /// `expanded-search:expanded-search`.
    ExpandedSearch(ExpandedSearch),
    /// `group-id:group-id-record` (group-id-3.0).
    GroupIdRecord(GroupIdRecord),
    /// `group-id:group-id-records` (group-id-3.0), a page of records.
    GroupIdRecords(GroupIdRecords),
    /// A root element this crate does not know (its local name).
    Unknown(String),
}

/// A parsed ORCID document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    /// Local name of the root element.
    pub root_name: String,
    /// The root element's `path` attribute, as written (any root may carry
    /// one; yorc-duckdb takes the record's iD from it).
    pub root_path: Option<String>,
    /// The root element's content.
    pub root: Root,
}

impl Document {
    /// The record, if this is a `record:record` document.
    pub fn record(&self) -> Option<&Record> {
        match &self.root {
            Root::Record(r) => Some(r),
            _ => None,
        }
    }

    /// The activities summary of a record or an `activities-summary` document.
    pub fn activities_summary(&self) -> Option<&ActivitiesSummary> {
        match &self.root {
            Root::Record(r) => r.activities_summary.as_ref(),
            Root::ActivitiesSummary(a) => Some(a),
            _ => None,
        }
    }

    /// The record's iD as yorc-duckdb determines it: the first segment of the
    /// root `path` attribute (`/0000-0003-0100-6634` → `0000-0003-0100-6634`),
    /// else the first non-empty `orcid-identifier/path`.
    ///
    /// ```
    /// # #[cfg(feature = "xml")] {
    /// let doc = yorc::xml::from_str(
    ///     r#"<search:search num-found="1" xmlns:search="http://www.orcid.org/ns/search"
    ///          xmlns:common="http://www.orcid.org/ns/common"><search:result>
    ///        <common:orcid-identifier><common:path>0000-0002-1825-0097</common:path></common:orcid-identifier>
    ///        </search:result></search:search>"#,
    /// )?;
    /// assert_eq!(doc.orcid(), Some("0000-0002-1825-0097"));
    /// # }
    /// # Ok::<(), yorc::Error>(())
    /// ```
    pub fn orcid(&self) -> Option<&str> {
        if let Some(p) = self.root_path.as_deref() {
            let seg = first_path_segment(trim_xml_space(p));
            if !seg.is_empty() {
                return Some(seg);
            }
        }
        match &self.root {
            Root::Record(r) => r.orcid_identifier.as_ref().and_then(identifier_path),
            Root::Search(s) => s.results.iter().find_map(identifier_path),
            _ => None,
        }
    }
}

fn identifier_path(id: &OrcidIdentifier) -> Option<&str> {
    id.path.as_deref().filter(|p| !p.is_empty())
}

/// Trims ASCII space, tab, CR and LF (yorc-duckdb's `Trim`).
///
/// ```
/// assert_eq!(yorc::model::trim_xml_space("\t vents \r\n"), "vents");
/// assert_eq!(yorc::model::trim_xml_space("\u{a0}x"), "\u{a0}x"); // ASCII only
/// ```
pub fn trim_xml_space(s: &str) -> &str {
    s.trim_matches(|c: char| c == ' ' || c == '\t' || c == '\r' || c == '\n')
}

/// First segment of an ORCID element path:
/// `/0000-0003-0100-6634/work/1` → `0000-0003-0100-6634`.
///
/// ```
/// use yorc::model::first_path_segment;
///
/// assert_eq!(first_path_segment("/0000-0003-0100-6634/work/1"), "0000-0003-0100-6634");
/// assert_eq!(first_path_segment("0000-0003-0100-6634"), "0000-0003-0100-6634");
/// ```
pub fn first_path_segment(path: &str) -> &str {
    let rest = path.strip_prefix('/').unwrap_or(path);
    match rest.find('/') {
        Some(i) => &rest[..i],
        None => rest,
    }
}
