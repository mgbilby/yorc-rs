//! `common-3.0.xsd`: types shared by every section.

/// ORCID put-code (`common:put-code`, an `xs:integer`).
pub type PutCode = i64;

string_enum! {
    /// `common:visibility`.
    Visibility {
        Private => "private",
        Limited => "limited",
        Public => "public",
        RegisteredOnly => "registered-only",
    }
}

string_enum! {
    /// `common:relationship-type` (`external-id-relationship`).
    Relationship {
        SelfRef => "self",
        PartOf => "part-of",
        VersionOf => "version-of",
        FundedBy => "funded-by",
    }
}

/// `common:orcid-id` (`orcid-identifier`, `source-orcid`, `contributor-orcid`,
/// `assertion-origin-orcid`, ...) and, with the same shape, `common:client-id`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrcidIdentifier {
    /// `uri`: `https://orcid.org/XXXX-XXXX-XXXX-XXXX` (or the client URI).
    pub uri: Option<String>,
    /// `path`: the bare iD (or client id), e.g. `0000-0002-1825-0097`.
    pub path: Option<String>,
    /// `host`: `orcid.org` or `sandbox.orcid.org`.
    pub host: Option<String>,
}

/// `common:client-id` (`source-client-id`, `assertion-origin-client-id`).
pub type ClientId = OrcidIdentifier;

/// `common:source-type`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Source {
    /// `source-orcid`: the user who added the item, if a user did.
    pub source_orcid: Option<OrcidIdentifier>,
    /// `source-client-id`: the member client that added the item.
    pub source_client_id: Option<ClientId>,
    /// `source-name`.
    pub source_name: Option<String>,
    /// `assertion-origin-orcid`: the user on whose behalf a client asserted the item.
    pub assertion_origin_orcid: Option<OrcidIdentifier>,
    /// `assertion-origin-client-id`: the client on whose behalf another client asserted the item.
    pub assertion_origin_client_id: Option<ClientId>,
    /// `assertion-origin-name`.
    pub assertion_origin_name: Option<String>,
}

/// `common:element-summary`: the attributes and metadata every activity and
/// person item carries.
///
/// ```
/// # #[cfg(feature = "xml")] {
/// use yorc::model::{Root, Visibility};
///
/// let doc = yorc::xml::from_str(
///     r#"<keyword:keyword put-code="7" visibility="public" display-index="-1"
///          xmlns:keyword="http://www.orcid.org/ns/keyword"><keyword:content>x</keyword:content></keyword:keyword>"#,
/// )?;
/// let Root::Keyword(k) = doc.root else { panic!() };
/// assert_eq!(k.meta.put_code, Some(7));
/// assert_eq!(k.meta.visibility, Some(Visibility::Public));
/// assert_eq!(k.meta.display_index, None);
/// assert_eq!(k.meta.display_index_text.as_deref(), Some("-1")); // kept for the writer
/// # }
/// # Ok::<(), yorc::Error>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemMeta {
    /// `@put-code`
    pub put_code: Option<PutCode>,
    /// `@visibility`
    pub visibility: Option<Visibility>,
    /// `@display-index` (an `xs:string` in the XSD; parsed as an integer
    /// with yorc-duckdb's rule, so `-1` is `None` here)
    pub display_index: Option<i32>,
    /// `@display-index` verbatim (trimmed), kept only when it is not exactly
    /// `display_index` written in decimal (e.g. `-1`, which ORCID emits).
    /// The writer uses it in preference to `display_index`.
    pub display_index_text: Option<String>,
    /// `@path`
    pub path: Option<String>,
    /// `common:created-date`
    pub created_date: Option<String>,
    /// `common:last-modified-date`
    pub last_modified_date: Option<String>,
    /// `common:source`
    pub source: Option<Source>,
}

/// `common:fuzzy-date` (`start-date`, `end-date`, `publication-date`, ...).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FuzzyDate {
    /// `year` (four digits in the XSD).
    pub year: Option<i32>,
    /// `month`, 1..=12.
    pub month: Option<i32>,
    /// `day`, 1..=31 (only with a month).
    pub day: Option<i32>,
}

/// `common:organization-address`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrganizationAddress {
    /// `city`.
    pub city: Option<String>,
    /// `region`.
    pub region: Option<String>,
    /// `country`: ISO 3166-1 alpha-2 code.
    pub country: Option<String>,
}

/// `common:disambiguated-organization`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisambiguatedOrganization {
    /// `disambiguated-organization-identifier`
    pub identifier: Option<String>,
    /// `disambiguation-source` (GRID, RINGGOLD, FUNDREF, ROR, LEI, ...)
    pub source: Option<String>,
}

/// `common:organization`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Organization {
    /// `common:name`.
    pub name: Option<String>,
    /// `common:address`.
    pub address: Option<OrganizationAddress>,
    /// `common:disambiguated-organization` (ROR, GRID, RINGGOLD, FUNDREF, ...).
    pub disambiguated_organization: Option<DisambiguatedOrganization>,
}

/// `common:transient-non-empty-string` (`external-id-normalized`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransientString {
    /// The normalised value.
    pub value: Option<String>,
    /// `@transient`
    pub transient: Option<bool>,
}

/// `common:transient-error` (`external-id-normalized-error`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransientError {
    /// `error-code`.
    pub error_code: Option<String>,
    /// `error-message`.
    pub error_message: Option<String>,
    /// `@transient`
    pub transient: Option<bool>,
}

/// `common:external-id` (also the person `external-identifier`, the peer
/// review `subject-external-identifier` and `review-identifiers` entries).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExternalId {
    /// `common:external-id` extends `element-summary`.
    pub meta: ItemMeta,
    /// `external-id-type` (doi, isbn, grant_number, agr, ...; an open list)
    pub id_type: Option<String>,
    /// `external-id-value`
    pub value: Option<String>,
    /// `external-id-normalized`
    pub normalized: Option<TransientString>,
    /// `external-id-normalized-error`
    pub normalized_error: Option<TransientError>,
    /// `external-id-url`
    pub url: Option<String>,
    /// `external-id-relationship`
    pub relationship: Option<Relationship>,
}

/// `common:translated-title` (text + `@language-code`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TranslatedTitle {
    /// The translated title.
    pub value: Option<String>,
    /// `@language-code` (ISO 639).
    pub language_code: Option<String>,
}

/// `work:work-title`, `funding:funding-title`, `research-resource-title`,
/// `peer-review:subject-name`: `common:title`, `common:subtitle`
/// (works and peer-review subjects only), `common:translated-title`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Title {
    /// `common:title`.
    pub title: Option<String>,
    /// `common:subtitle` (works and peer-review subjects only).
    pub subtitle: Option<String>,
    /// `common:translated-title`.
    pub translated_title: Option<TranslatedTitle>,
}

/// `common:amount` (text + `@currency-code`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Amount {
    /// The amount as written (not parsed), e.g. `1000000`.
    pub value: Option<String>,
    /// `@currency-code` (ISO 4217).
    pub currency_code: Option<String>,
}

/// `common:country` element (text + `@visibility`), as used by works.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Country {
    /// ISO 3166-1 alpha-2 country code.
    pub value: Option<String>,
    /// `@visibility` (`public`, `limited`, `registered-only`, `private`).
    pub visibility: Option<Visibility>,
}

string_enum! {
    /// `contributor-sequence`.
    ContributorSequence {
        First => "first",
        Additional => "additional",
    }
}

string_enum! {
    /// `contributor-role`: the legacy work and funding roles. CRediT role
    /// URIs (`https://credit.niso.org/contributor-roles/...`) are `Other`.
    ContributorRole {
        Author => "author",
        Assignee => "assignee",
        Editor => "editor",
        ChairOrTranslator => "chair-or-translator",
        CoInvestigator => "co-investigator",
        CoInventor => "co-inventor",
        GraduateStudent => "graduate-student",
        OtherInventor => "other-inventor",
        PrincipalInvestigator => "principal-investigator",
        PostdoctoralResearcher => "postdoctoral-researcher",
        SupportStaff => "support-staff",
        Lead => "lead",
        CoLead => "co-lead",
        SupportedBy => "supported-by",
        OtherContribution => "other-contribution",
    }
}

/// `contributor-attributes` (works: sequence + role; fundings: role).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContributorAttributes {
    /// `contributor-sequence` (works only).
    pub sequence: Option<ContributorSequence>,
    /// `contributor-role`; CRediT role URIs are `Other(..)`.
    pub role: Option<ContributorRole>,
}

/// `work:contributor` / `funding:contributor`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Contributor {
    /// `contributor-orcid`.
    pub contributor_orcid: Option<OrcidIdentifier>,
    /// `credit-name`.
    pub credit_name: Option<String>,
    /// `contributor-email`.
    pub contributor_email: Option<String>,
    /// `contributor-attributes`.
    pub attributes: Option<ContributorAttributes>,
}
