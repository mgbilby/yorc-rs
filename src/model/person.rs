//! `person-3.0.xsd`, `personal-details-3.0.xsd` and the person item XSDs
//! (other names, researcher URLs, emails, addresses, keywords, external
//! identifiers).

use super::common::{ExternalId, ItemMeta, Visibility};

/// `personal-details:name`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Name {
    /// `@visibility`.
    pub visibility: Option<Visibility>,
    /// `@path`: the element's API path (e.g. `/0000-0002-1825-0097/works`), as written.
    pub path: Option<String>,
    /// `common:created-date` (`xs:dateTime` text, as ORCID sent it).
    pub created_date: Option<String>,
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// `given-names`.
    pub given_names: Option<String>,
    /// `family-name`.
    pub family_name: Option<String>,
    /// `credit-name` (the published name).
    pub credit_name: Option<String>,
}

/// `personal-details:biography`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Biography {
    /// `@visibility`.
    pub visibility: Option<Visibility>,
    /// `@path`: the element's API path (e.g. `/0000-0002-1825-0097/works`), as written.
    pub path: Option<String>,
    /// `common:created-date` (`xs:dateTime` text, as ORCID sent it).
    pub created_date: Option<String>,
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// The biography text.
    pub content: Option<String>,
}

/// `other-name:other-name`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OtherName {
    /// Put-code, visibility, display index, path, dates and source (`common:element-summary`).
    pub meta: ItemMeta,
    /// The other name.
    pub content: Option<String>,
}

/// `researcher-url:researcher-url`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResearcherUrl {
    /// Put-code, visibility, display index, path, dates and source (`common:element-summary`).
    pub meta: ItemMeta,
    /// `url-name`: the link text.
    pub url_name: Option<String>,
    /// `url`.
    pub url: Option<String>,
}

/// `email:email`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Email {
    /// Put-code, visibility, display index, path, dates and source (`common:element-summary`).
    pub meta: ItemMeta,
    /// The address.
    pub email: Option<String>,
    /// `@verified`
    pub verified: Option<bool>,
    /// `@primary`
    pub primary: Option<bool>,
}

/// `address:address`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Address {
    /// Put-code, visibility, display index, path, dates and source (`common:element-summary`).
    pub meta: ItemMeta,
    /// ISO 3166 country code
    pub country: Option<String>,
}

/// `keyword:keyword`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keyword {
    /// Put-code, visibility, display index, path, dates and source (`common:element-summary`).
    pub meta: ItemMeta,
    /// The keyword.
    pub content: Option<String>,
}

/// `external-identifier:external-identifier` (a `common:external-id`).
pub type PersonExternalIdentifier = ExternalId;

/// A person item container: `other-names`, `researcher-urls`, `emails`,
/// `addresses`, `keywords`, `external-identifiers`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemList<T> {
    /// `@path`: the element's API path (e.g. `/0000-0002-1825-0097/works`), as written.
    pub path: Option<String>,
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// The items, in document order.
    pub items: Vec<T>,
}

impl<T> Default for ItemList<T> {
    fn default() -> Self {
        ItemList {
            path: None,
            last_modified_date: None,
            items: Vec::new(),
        }
    }
}

/// `other-name:other-names`.
pub type OtherNames = ItemList<OtherName>;
/// `researcher-url:researcher-urls`.
pub type ResearcherUrls = ItemList<ResearcherUrl>;
/// `email:emails`.
pub type Emails = ItemList<Email>;
/// `address:addresses`.
pub type Addresses = ItemList<Address>;
/// `keyword:keywords`.
pub type Keywords = ItemList<Keyword>;
/// `external-identifier:external-identifiers`.
pub type ExternalIdentifiers = ItemList<PersonExternalIdentifier>;

/// `person:person`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Person {
    /// `@path`: the element's API path (e.g. `/0000-0002-1825-0097/works`), as written.
    pub path: Option<String>,
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// `personal-details:name`.
    pub name: Option<Name>,
    /// `other-name:other-names`.
    pub other_names: Option<OtherNames>,
    /// `personal-details:biography`.
    pub biography: Option<Biography>,
    /// `researcher-url:researcher-urls`.
    pub researcher_urls: Option<ResearcherUrls>,
    /// `email:emails`.
    pub emails: Option<Emails>,
    /// `address:addresses`.
    pub addresses: Option<Addresses>,
    /// `keyword:keywords`.
    pub keywords: Option<Keywords>,
    /// `external-identifier:external-identifiers`.
    pub external_identifiers: Option<ExternalIdentifiers>,
}

/// `personal-details:personal-details`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PersonalDetails {
    /// `@path`: the element's API path (e.g. `/0000-0002-1825-0097/works`), as written.
    pub path: Option<String>,
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// `personal-details:name`.
    pub name: Option<Name>,
    /// `other-name:other-names`.
    pub other_names: Option<OtherNames>,
    /// `personal-details:biography`.
    pub biography: Option<Biography>,
}
