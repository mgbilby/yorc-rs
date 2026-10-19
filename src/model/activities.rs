//! `activities-3.0.xsd` and the activity XSDs: works, fundings, the seven
//! affiliation kinds, peer reviews, research resources, their groups and the
//! activities summary.

use super::common::{
    Amount, Contributor, Country, ExternalId, FuzzyDate, ItemMeta, Organization, Title,
};

// ------------------------------------------------------------------- works

string_enum! {
    /// `work:work-type` (open list in the XSD; these are the v3.0 values).
    WorkType {
        Annotation => "annotation",
        ArtisticPerformance => "artistic-performance",
        BlogPost => "blog-post",
        BookChapter => "book-chapter",
        BookReview => "book-review",
        Book => "book",
        CartographicMaterial => "cartographic-material",
        ClinicalStudy => "clinical-study",
        ConferenceAbstract => "conference-abstract",
        ConferencePaper => "conference-paper",
        ConferencePoster => "conference-poster",
        ConferenceOutput => "conference-output",
        ConferencePresentation => "conference-presentation",
        ConferenceProceedings => "conference-proceedings",
        DataManagementPlan => "data-management-plan",
        DataSet => "data-set",
        Design => "design",
        DictionaryEntry => "dictionary-entry",
        Disclosure => "disclosure",
        Dissertation => "dissertation",
        DissertationThesis => "dissertation-thesis",
        EditedBook => "edited-book",
        EncyclopediaEntry => "encyclopedia-entry",
        Image => "image",
        Invention => "invention",
        JournalArticle => "journal-article",
        JournalIssue => "journal-issue",
        LearningObject => "learning-object",
        LectureSpeech => "lecture-speech",
        License => "license",
        MagazineArticle => "magazine-article",
        Manual => "manual",
        MovingImage => "moving-image",
        Musical => "musical-composition",
        NewsletterArticle => "newsletter-article",
        NewspaperArticle => "newspaper-article",
        OnlineResource => "online-resource",
        OtherWork => "other",
        Patent => "patent",
        PhysicalObject => "physical-object",
        Preprint => "preprint",
        PublicSpeech => "public-speech",
        RegisteredCopyright => "registered-copyright",
        Report => "report",
        ResearchProtocol => "research-protocol",
        ResearchTechnique => "research-technique",
        ResearchTool => "research-tool",
        Review => "review",
        Software => "software",
        Sound => "sound",
        SpinOffCompany => "spin-off-company",
        StandardsAndPolicy => "standards-and-policy",
        SupervisedStudentPublication => "supervised-student-publication",
        TechnicalStandard => "technical-standard",
        Test => "test",
        Trademark => "trademark",
        Translation => "translation",
        Undefined => "undefined",
        Website => "website",
        WorkingPaper => "working-paper",
    }
}

string_enum! {
    /// `work:citation-type`.
    CitationType {
        FormattedUnspecified => "formatted-unspecified",
        Bibtex => "bibtex",
        Ris => "ris",
        FormattedApa => "formatted-apa",
        FormattedHarvard => "formatted-harvard",
        FormattedIeee => "formatted-ieee",
        FormattedMla => "formatted-mla",
        FormattedVancouver => "formatted-vancouver",
        FormattedChicago => "formatted-chicago",
        Rif => "rif",
        CslJson => "csl-json",
    }
}

/// `work:citation`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Citation {
    /// `citation-type` (`Other(..)` keeps values this version does not know).
    pub citation_type: Option<CitationType>,
    /// `citation-value`: the citation text (BibTeX, RIS, ...).
    pub citation_value: Option<String>,
}

/// `work:work-summary`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkSummary {
    /// Put-code, visibility, display index, path, dates and source (`common:element-summary`).
    pub meta: ItemMeta,
    /// Title, subtitle (works only) and translated title.
    pub title: Option<Title>,
    /// `common:external-ids/common:external-id`.
    pub external_ids: Vec<ExternalId>,
    /// `common:url`.
    pub url: Option<String>,
    /// `work:type` (`Other(..)` keeps values this version does not know).
    pub work_type: Option<WorkType>,
    /// `common:publication-date`.
    pub publication_date: Option<FuzzyDate>,
    /// `journal-title`.
    pub journal_title: Option<String>,
}

/// `work:work`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Work {
    /// Put-code, visibility, display index, path, dates and source (`common:element-summary`).
    pub meta: ItemMeta,
    /// Title, subtitle (works only) and translated title.
    pub title: Option<Title>,
    /// `journal-title`.
    pub journal_title: Option<String>,
    /// `short-description`.
    pub short_description: Option<String>,
    /// `work:citation`.
    pub citation: Option<Citation>,
    /// `work:type` (`Other(..)` keeps values this version does not know).
    pub work_type: Option<WorkType>,
    /// `common:publication-date`.
    pub publication_date: Option<FuzzyDate>,
    /// `common:external-ids/common:external-id`.
    pub external_ids: Vec<ExternalId>,
    /// `common:url`.
    pub url: Option<String>,
    /// `work:contributors/work:contributor`
    pub contributors: Vec<Contributor>,
    /// `common:language-code` (ISO 639).
    pub language_code: Option<String>,
    /// `common:country`.
    pub country: Option<Country>,
}

/// `activities:work-group` (`activities:group` under `works`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkGroup {
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// The identifiers the group's items share.
    pub external_ids: Vec<ExternalId>,
    /// `work:work-summary` items.
    pub work_summaries: Vec<WorkSummary>,
}

/// `activities:works`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Works {
    /// `@path`: the element's API path (e.g. `/0000-0002-1825-0097/works`), as written.
    pub path: Option<String>,
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// `activities:group` entries: items ORCID groups by shared external ids.
    pub groups: Vec<WorkGroup>,
}

// ---------------------------------------------------------------- fundings

string_enum! {
    /// `funding:funding-type`.
    FundingType {
        Grant => "grant",
        Contract => "contract",
        Award => "award",
        SalaryAward => "salary-award",
    }
}

/// `funding:funding-summary`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FundingSummary {
    /// Put-code, visibility, display index, path, dates and source (`common:element-summary`).
    pub meta: ItemMeta,
    /// Title, subtitle (works only) and translated title.
    pub title: Option<Title>,
    /// `common:external-ids/common:external-id`.
    pub external_ids: Vec<ExternalId>,
    /// `common:url`.
    pub url: Option<String>,
    /// `funding:type` (`Other(..)` keeps values this version does not know).
    pub funding_type: Option<FundingType>,
    /// `common:start-date`.
    pub start_date: Option<FuzzyDate>,
    /// `common:end-date`.
    pub end_date: Option<FuzzyDate>,
    /// `common:organization`: name, address, disambiguated id.
    pub organization: Option<Organization>,
}

/// `funding:funding`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Funding {
    /// Put-code, visibility, display index, path, dates and source (`common:element-summary`).
    pub meta: ItemMeta,
    /// `funding:type` (`Other(..)` keeps values this version does not know).
    pub funding_type: Option<FundingType>,
    /// `organization-defined-type`.
    pub organization_defined_type: Option<String>,
    /// Title, subtitle (works only) and translated title.
    pub title: Option<Title>,
    /// `short-description`.
    pub short_description: Option<String>,
    /// `amount`.
    pub amount: Option<Amount>,
    /// `common:url`.
    pub url: Option<String>,
    /// `common:start-date`.
    pub start_date: Option<FuzzyDate>,
    /// `common:end-date`.
    pub end_date: Option<FuzzyDate>,
    /// `common:external-ids/common:external-id`.
    pub external_ids: Vec<ExternalId>,
    /// `funding:contributors/funding:contributor`
    pub contributors: Vec<Contributor>,
    /// `common:organization`: name, address, disambiguated id.
    pub organization: Option<Organization>,
}

/// `activities:funding-group`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FundingGroup {
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// The identifiers the group's items share.
    pub external_ids: Vec<ExternalId>,
    /// `funding:funding-summary` items.
    pub funding_summaries: Vec<FundingSummary>,
}

/// `activities:fundings`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fundings {
    /// `@path`: the element's API path (e.g. `/0000-0002-1825-0097/works`), as written.
    pub path: Option<String>,
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// `activities:group` entries: items ORCID groups by shared external ids.
    pub groups: Vec<FundingGroup>,
}

// ------------------------------------------------------------ affiliations

/// The seven affiliation sections; they share `common:affiliation`.
///
/// ```
/// use yorc::model::AffiliationKind;
///
/// let k = AffiliationKind::from_container_element("invited-positions");
/// assert_eq!(k, Some(AffiliationKind::InvitedPosition));
/// assert_eq!(AffiliationKind::InvitedPosition.as_str(), "invited-position");
/// assert_eq!(AffiliationKind::InvitedPosition.summary_element(), "invited-position-summary");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AffiliationKind {
    /// `distinction`
    Distinction,
    /// `education`
    Education,
    /// `employment`
    Employment,
    /// `invited-position`
    InvitedPosition,
    /// `membership`
    Membership,
    /// `qualification`
    Qualification,
    /// `service`
    Service,
}

impl AffiliationKind {
    /// The seven kinds, in alphabetical (XSD) order.
    pub const ALL: [AffiliationKind; 7] = [
        AffiliationKind::Distinction,
        AffiliationKind::Education,
        AffiliationKind::Employment,
        AffiliationKind::InvitedPosition,
        AffiliationKind::Membership,
        AffiliationKind::Qualification,
        AffiliationKind::Service,
    ];

    /// Element name of the full item: `distinction`, ..., `invited-position`.
    pub fn as_str(self) -> &'static str {
        match self {
            AffiliationKind::Distinction => "distinction",
            AffiliationKind::Education => "education",
            AffiliationKind::Employment => "employment",
            AffiliationKind::InvitedPosition => "invited-position",
            AffiliationKind::Membership => "membership",
            AffiliationKind::Qualification => "qualification",
            AffiliationKind::Service => "service",
        }
    }

    /// Element name of the summary item: `distinction-summary`, ...
    pub fn summary_element(self) -> &'static str {
        match self {
            AffiliationKind::Distinction => "distinction-summary",
            AffiliationKind::Education => "education-summary",
            AffiliationKind::Employment => "employment-summary",
            AffiliationKind::InvitedPosition => "invited-position-summary",
            AffiliationKind::Membership => "membership-summary",
            AffiliationKind::Qualification => "qualification-summary",
            AffiliationKind::Service => "service-summary",
        }
    }

    /// Element name of the section container: `distinctions`, ...
    pub fn container_element(self) -> &'static str {
        match self {
            AffiliationKind::Distinction => "distinctions",
            AffiliationKind::Education => "educations",
            AffiliationKind::Employment => "employments",
            AffiliationKind::InvitedPosition => "invited-positions",
            AffiliationKind::Membership => "memberships",
            AffiliationKind::Qualification => "qualifications",
            AffiliationKind::Service => "services",
        }
    }

    /// The kind whose full item element is `name`.
    pub fn from_element(name: &str) -> Option<AffiliationKind> {
        AffiliationKind::ALL
            .into_iter()
            .find(|k| k.as_str() == name)
    }

    /// The kind whose summary element is `name`.
    pub fn from_summary_element(name: &str) -> Option<AffiliationKind> {
        AffiliationKind::ALL
            .into_iter()
            .find(|k| k.summary_element() == name)
    }

    /// The kind whose container element is `name`.
    pub fn from_container_element(name: &str) -> Option<AffiliationKind> {
        AffiliationKind::ALL
            .into_iter()
            .find(|k| k.container_element() == name)
    }
}

/// `common:affiliation` / `common:affiliation-summary` (identical content).
/// Used for both the summary (`employment-summary`, inside groups) and the
/// full item (`employment`); the document or container says which.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Affiliation {
    /// Which of the seven affiliation kinds.
    pub kind: AffiliationKind,
    /// Put-code, visibility, display index, path, dates and source (`common:element-summary`).
    pub meta: ItemMeta,
    /// `department-name`.
    pub department_name: Option<String>,
    /// `role-title`.
    pub role_title: Option<String>,
    /// `common:start-date`.
    pub start_date: Option<FuzzyDate>,
    /// `common:end-date`.
    pub end_date: Option<FuzzyDate>,
    /// `common:organization`: name, address, disambiguated id.
    pub organization: Option<Organization>,
    /// `common:url`.
    pub url: Option<String>,
    /// `common:external-ids/common:external-id`.
    pub external_ids: Vec<ExternalId>,
}

impl Affiliation {
    /// An empty affiliation of `kind` (there is no `Default`: the kind is required).
    pub fn new(kind: AffiliationKind) -> Affiliation {
        Affiliation {
            kind,
            meta: ItemMeta::default(),
            department_name: None,
            role_title: None,
            start_date: None,
            end_date: None,
            organization: None,
            url: None,
            external_ids: Vec::new(),
        }
    }
}

/// `activities:affiliation-group`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AffiliationGroup {
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// The identifiers the group's items share.
    pub external_ids: Vec<ExternalId>,
    /// `*-summary` items (normally all of the container's kind).
    pub summaries: Vec<Affiliation>,
}

/// `activities:distinctions`, `educations`, `employments`,
/// `invited-positions`, `memberships`, `qualifications`, `services`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Affiliations {
    /// Which of the seven affiliation sections.
    pub kind: AffiliationKind,
    /// `@path`: the element's API path (e.g. `/0000-0002-1825-0097/works`), as written.
    pub path: Option<String>,
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// `activities:affiliation-group` entries.
    pub groups: Vec<AffiliationGroup>,
}

impl Affiliations {
    /// An empty container of `kind`.
    pub fn new(kind: AffiliationKind) -> Affiliations {
        Affiliations {
            kind,
            path: None,
            last_modified_date: None,
            groups: Vec::new(),
        }
    }
}

// ------------------------------------------------------------ peer reviews

string_enum! {
    /// `peer-review:role`.
    PeerReviewRole {
        Reviewer => "reviewer",
        Editor => "editor",
        Member => "member",
        Chair => "chair",
        Organizer => "organizer",
    }
}

string_enum! {
    /// `peer-review:type`.
    PeerReviewType {
        Review => "review",
        Evaluation => "evaluation",
    }
}

/// `peer-review:peer-review-summary`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeerReviewSummary {
    /// Put-code, visibility, display index, path, dates and source (`common:element-summary`).
    pub meta: ItemMeta,
    /// `reviewer-role` (`Other(..)` keeps values this version does not know).
    pub reviewer_role: Option<PeerReviewRole>,
    /// `common:external-ids/common:external-id`.
    pub external_ids: Vec<ExternalId>,
    /// `review-url`.
    pub review_url: Option<String>,
    /// `review-type` (`Other(..)` keeps values this version does not know).
    pub review_type: Option<PeerReviewType>,
    /// `completion-date`.
    pub completion_date: Option<FuzzyDate>,
    /// `review-group-id`: the peer-review group (`issn:...`, `orcid-generated:...`, ...).
    pub review_group_id: Option<String>,
    /// `convening-organization`.
    pub convening_organization: Option<Organization>,
}

/// `peer-review:peer-review`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeerReview {
    /// Put-code, visibility, display index, path, dates and source (`common:element-summary`).
    pub meta: ItemMeta,
    /// `reviewer-role` (`Other(..)` keeps values this version does not know).
    pub reviewer_role: Option<PeerReviewRole>,
    /// `review-identifiers/external-id`
    pub review_identifiers: Vec<ExternalId>,
    /// `review-url`.
    pub review_url: Option<String>,
    /// `review-type` (`Other(..)` keeps values this version does not know).
    pub review_type: Option<PeerReviewType>,
    /// `review-completion-date`.
    pub review_completion_date: Option<FuzzyDate>,
    /// `review-group-id`: the peer-review group (`issn:...`, `orcid-generated:...`, ...).
    pub review_group_id: Option<String>,
    /// `subject-external-identifier`: the reviewed item.
    pub subject_external_identifier: Option<ExternalId>,
    /// `subject-container-name` (e.g. the journal).
    pub subject_container_name: Option<String>,
    /// `subject-type` (work types plus e.g. `award`, `grant`; an open list)
    pub subject_type: Option<String>,
    /// `subject-name`: title, subtitle, translated title
    pub subject_name: Option<Title>,
    /// `subject-url`.
    pub subject_url: Option<String>,
    /// `convening-organization`.
    pub convening_organization: Option<Organization>,
}

/// `activities:peer-review-duplicates` (element `activities:peer-review-group`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeerReviewDuplicates {
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// The identifiers the group's items share.
    pub external_ids: Vec<ExternalId>,
    /// `peer-review:peer-review-summary` items.
    pub summaries: Vec<PeerReviewSummary>,
}

/// `activities:peer-review-group` (element `activities:group`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeerReviewGroup {
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// The identifiers the group's items share.
    pub external_ids: Vec<ExternalId>,
    /// `peer-review-group` entries.
    pub duplicates: Vec<PeerReviewDuplicates>,
}

/// `activities:peer-reviews`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeerReviews {
    /// `@path`: the element's API path (e.g. `/0000-0002-1825-0097/works`), as written.
    pub path: Option<String>,
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// `activities:group` entries: items ORCID groups by shared external ids.
    pub groups: Vec<PeerReviewGroup>,
}

// ------------------------------------------------------ research resources

string_enum! {
    /// `research-resource:resource-type`.
    ResourceType {
        Infrastructures => "infrastructures",
        Collections => "collections",
        Equipment => "equipment",
        Services => "services",
    }
}

/// `research-resource:proposal`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Proposal {
    /// Title, subtitle (works only) and translated title.
    pub title: Option<Title>,
    /// `hosts/organization` (1..10)
    pub hosts: Vec<Organization>,
    /// `common:external-ids/common:external-id`.
    pub external_ids: Vec<ExternalId>,
    /// `common:start-date`.
    pub start_date: Option<FuzzyDate>,
    /// `common:end-date`.
    pub end_date: Option<FuzzyDate>,
    /// `common:url`.
    pub url: Option<String>,
}

/// `research-resource:resource-item`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceItem {
    /// `resource-name`.
    pub resource_name: Option<String>,
    /// `resource-type` (`Other(..)` keeps values this version does not know).
    pub resource_type: Option<ResourceType>,
    /// `hosts/organization`.
    pub hosts: Vec<Organization>,
    /// `common:external-ids/common:external-id`.
    pub external_ids: Vec<ExternalId>,
    /// `common:url`.
    pub url: Option<String>,
}

/// `research-resource:research-resource-summary`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResearchResourceSummary {
    /// Put-code, visibility, display index, path, dates and source (`common:element-summary`).
    pub meta: ItemMeta,
    /// `research-resource:proposal`: title, hosts, ids, dates, URL.
    pub proposal: Option<Proposal>,
}

/// `research-resource:research-resource`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResearchResource {
    /// Put-code, visibility, display index, path, dates and source (`common:element-summary`).
    pub meta: ItemMeta,
    /// `research-resource:proposal`: title, hosts, ids, dates, URL.
    pub proposal: Option<Proposal>,
    /// `resource-items/resource-item`
    pub resource_items: Vec<ResourceItem>,
}

/// `activities:research-resource-group`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResearchResourceGroup {
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// The identifiers the group's items share.
    pub external_ids: Vec<ExternalId>,
    /// `research-resource-summary` items.
    pub summaries: Vec<ResearchResourceSummary>,
}

/// `activities:research-resources`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResearchResources {
    /// `@path`: the element's API path (e.g. `/0000-0002-1825-0097/works`), as written.
    pub path: Option<String>,
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// `activities:group` entries: items ORCID groups by shared external ids.
    pub groups: Vec<ResearchResourceGroup>,
}

// ----------------------------------------------------- activities summary

/// `activities:activities-summary`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActivitiesSummary {
    /// `@path`: the element's API path (e.g. `/0000-0002-1825-0097/works`), as written.
    pub path: Option<String>,
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// `activities:distinctions`.
    pub distinctions: Option<Affiliations>,
    /// `activities:educations`.
    pub educations: Option<Affiliations>,
    /// `activities:employments`.
    pub employments: Option<Affiliations>,
    /// `activities:fundings`.
    pub fundings: Option<Fundings>,
    /// `activities:invited-positions`.
    pub invited_positions: Option<Affiliations>,
    /// `activities:memberships`.
    pub memberships: Option<Affiliations>,
    /// `activities:peer-reviews`.
    pub peer_reviews: Option<PeerReviews>,
    /// `activities:qualifications`.
    pub qualifications: Option<Affiliations>,
    /// `activities:research-resources`.
    pub research_resources: Option<ResearchResources>,
    /// `activities:services`.
    pub services: Option<Affiliations>,
    /// `activities:works`.
    pub works: Option<Works>,
}

impl ActivitiesSummary {
    /// The affiliation container of a kind.
    pub fn affiliations(&self, kind: AffiliationKind) -> Option<&Affiliations> {
        match kind {
            AffiliationKind::Distinction => self.distinctions.as_ref(),
            AffiliationKind::Education => self.educations.as_ref(),
            AffiliationKind::Employment => self.employments.as_ref(),
            AffiliationKind::InvitedPosition => self.invited_positions.as_ref(),
            AffiliationKind::Membership => self.memberships.as_ref(),
            AffiliationKind::Qualification => self.qualifications.as_ref(),
            AffiliationKind::Service => self.services.as_ref(),
        }
    }

    /// Mutable slot of the affiliation container of a kind.
    pub fn affiliations_slot(&mut self, kind: AffiliationKind) -> &mut Option<Affiliations> {
        match kind {
            AffiliationKind::Distinction => &mut self.distinctions,
            AffiliationKind::Education => &mut self.educations,
            AffiliationKind::Employment => &mut self.employments,
            AffiliationKind::InvitedPosition => &mut self.invited_positions,
            AffiliationKind::Membership => &mut self.memberships,
            AffiliationKind::Qualification => &mut self.qualifications,
            AffiliationKind::Service => &mut self.services,
        }
    }

    /// Every work summary, in document order.
    ///
    /// ```
    /// use yorc::model::{ActivitiesSummary, WorkGroup, WorkSummary, Works};
    ///
    /// let summary = ActivitiesSummary {
    ///     works: Some(Works {
    ///         groups: vec![WorkGroup { work_summaries: vec![WorkSummary::default(); 2], ..WorkGroup::default() }],
    ///         ..Works::default()
    ///     }),
    ///     ..ActivitiesSummary::default()
    /// };
    /// assert_eq!(summary.work_summaries().count(), 2);
    /// ```
    pub fn work_summaries(&self) -> impl Iterator<Item = &WorkSummary> {
        self.works
            .iter()
            .flat_map(|w| w.groups.iter())
            .flat_map(|g| g.work_summaries.iter())
    }
}
