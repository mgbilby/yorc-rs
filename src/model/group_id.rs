//! `group-id-3.0.xsd`: peer-review group ids (`group-id-record`, and the
//! `group-id-records` page the API returns for a search), as ORCID's
//! `GroupIdRecord` / `GroupIdRecords` (`org.orcid.jaxb.model.v3.release.groupid`)
//! hold them.

use super::common::{PutCode, Source};

string_enum! {
    /// `group-id:type`: the kind of group a peer review belongs to.
    GroupType {
        Publisher => "publisher",
        Institution => "institution",
        Journal => "journal",
        Conference => "conference",
        Newspaper => "newspaper",
        Newsletter => "newsletter",
        Magazine => "magazine",
        PeerReviewService => "peer-review service",
    }
}

/// `group-id:group-id-record`: one peer-review group.
///
/// The XSD extends common-2.1's `source-type`, so its source fields
/// (`source-orcid`, `source-client-id`, `source-name`) may sit directly in
/// the record; ORCID's v3.0 API wraps them in `common:source`. Both read into
/// [`GroupIdRecord::source`]; the writers write `common:source`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GroupIdRecord {
    /// `@put-code`
    pub put_code: Option<PutCode>,
    /// `common:created-date` (`xs:dateTime` text, as ORCID sent it; XML only:
    /// ORCID's JSON leaves it out).
    pub created_date: Option<String>,
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it; XML
    /// only, as `created_date`).
    pub last_modified_date: Option<String>,
    /// `name`: the group's display name.
    pub name: Option<String>,
    /// `group-id`: the group's identifier (`issn:0000-0001`, `ringgold:355321`, ...).
    pub group_id: Option<String>,
    /// `description`.
    pub description: Option<String>,
    /// `type` (`Other(..)` keeps values this version does not know).
    pub group_type: Option<GroupType>,
    /// `common:source` (or the inline source fields of the XSD).
    pub source: Option<Source>,
}

/// `group-id:group-id-records`: one page of group-id records.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GroupIdRecords {
    /// `common:last-modified-date` (`xs:dateTime` text, as ORCID sent it).
    pub last_modified_date: Option<String>,
    /// `total`: records matching the search (1 to 9 ASCII digits).
    pub total: Option<i64>,
    /// `page`: the page number (1 to 9 ASCII digits).
    pub page: Option<i64>,
    /// `page-size` (1 to 9 ASCII digits).
    pub page_size: Option<i64>,
    /// The `group-id-record` entries, in document order.
    pub records: Vec<GroupIdRecord>,
}
