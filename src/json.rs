//! ORCID's JSON form (feature `json`): the `application/vnd.orcid+json`
//! documents of the v3.0 API, read into and written from the same model as
//! the XML ([`crate::model`]), through `serde_json::Value`.
//!
//! The shape follows ORCID's samples (`reference/orcid-model/record_3.0/samples`,
//! `*.json`) and the v3.0 Java model they come from:
//! * kebab-case keys, the XML element names without prefixes; attributes are
//!   ordinary keys (`put-code`, `path`, `visibility`, `display-index`, ...);
//! * many text values are wrapped as `{"value": ...}` (titles, `journal-title`,
//!   `credit-name`, `source-name`, every URL, fuzzy-date parts, `country` of
//!   works and addresses, `amount` with `currency-code`); others are bare
//!   strings (`type`, `content`, `department-name`, organisation fields, ...);
//! * timestamps are `{"value": <epoch milliseconds>}`; the model keeps the
//!   XML text, so reading turns them into `YYYY-MM-DDTHH:MM:SS.mmmZ` and
//!   writing parses any `xs:dateTime` (offset or `Z`; none means UTC) back;
//! * lists are arrays under the item name (`"external-id": [...]`,
//!   `"work-summary": [...]`), affiliation groups hold
//!   `"summaries": [{"employment-summary": {...}}]`, research resources have
//!   `"resource-item": [...]` directly, bulk is `{"bulk": [{"work": ...}, {"error": ...}]}`;
//! * `history/creation-method` uses the Java enum names (`MEMBER_REFERRED`,
//!   `API`, `DIRECT`, `WEBSITE`);
//! * `history/@visibility` is XML-only: ORCID's JSON has no such member (its
//!   Java model refuses unknown members), so writing leaves it out and reading
//!   still takes it if present. An XML -> JSON -> XML round trip loses it.
//! * a work's `country/@visibility` is XML-only for the same reason (ORCID's
//!   `Country` has only `value`).
//!
//! Reading is lenient like the XML reader: unknown keys and `null` are
//! ignored, a text may come bare or wrapped (`"x"` or `{"value": "x"}`),
//! numbers and strings are accepted for each other, text is trimmed and empty
//! text is absent. Writing omits absent values (no `null`s), always writes the
//! containers the XSD requires, and sorts keys (deterministic output).
//!
//! JSON documents do not name their root, so reading takes the document kind:
//! the local name of the XML root element (`"record"`, `"work"`,
//! `"employment"`, `"works"`, `"bulk"`, `"error"`, ...; see [`KINDS`]).

use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::model::*;

/// Every document kind [`from_str`] accepts (XML root local names).
pub const KINDS: &[&str] = &[
    "record",
    "activities-summary",
    "works",
    "work",
    "work-summary",
    "fundings",
    "funding",
    "funding-summary",
    "distinctions",
    "educations",
    "employments",
    "invited-positions",
    "memberships",
    "qualifications",
    "services",
    "distinction",
    "education",
    "employment",
    "invited-position",
    "membership",
    "qualification",
    "service",
    "distinction-summary",
    "education-summary",
    "employment-summary",
    "invited-position-summary",
    "membership-summary",
    "qualification-summary",
    "service-summary",
    "peer-reviews",
    "peer-review",
    "peer-review-summary",
    "research-resources",
    "research-resource",
    "research-resource-summary",
    "person",
    "personal-details",
    "name",
    "biography",
    "credit-name",
    "other-names",
    "other-name",
    "researcher-urls",
    "researcher-url",
    "emails",
    "email",
    "addresses",
    "address",
    "keywords",
    "keyword",
    "external-identifiers",
    "external-identifier",
    "history",
    "preferences",
    "bulk",
    "error",
    "search",
    "expanded-search",
    "group-id-record",
    "group-id-records",
];

// ------------------------------------------------------------ timestamps

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Epoch milliseconds → `YYYY-MM-DDTHH:MM:SS.mmmZ`.
///
/// ```
/// assert_eq!(yorc::json::millis_to_datetime(0), "1970-01-01T00:00:00.000Z");
/// assert_eq!(yorc::json::millis_to_datetime(1_487_783_673_349), "2017-02-22T17:14:33.349Z");
/// ```
pub fn millis_to_datetime(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3_600_000,
        rem / 60_000 % 60,
        rem / 1000 % 60,
        rem % 1000
    )
}

/// `xs:dateTime` (`YYYY-MM-DDTHH:MM:SS[.f+][Z|±HH:MM]`, no zone = UTC) →
/// epoch milliseconds (fractions beyond milliseconds are truncated).
///
/// ```
/// use yorc::json::datetime_to_millis;
///
/// assert_eq!(datetime_to_millis("2017-02-22T17:14:33.349Z"), Some(1_487_783_673_349));
/// assert_eq!(datetime_to_millis("1970-01-01T01:00:00+01:00"), Some(0));
/// assert_eq!(datetime_to_millis("yesterday"), None);
/// ```
pub fn datetime_to_millis(s: &str) -> Option<i64> {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() < 19
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let t = s.get(r)?;
        if t.bytes().all(|c| c.is_ascii_digit()) {
            t.parse().ok()
        } else {
            None
        }
    };
    let (y, mo, d, h, mi, sec) = (
        num(0..4)?,
        num(5..7)?,
        num(8..10)?,
        num(11..13)?,
        num(14..16)?,
        num(17..19)?,
    );
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 24 || mi > 59 || sec > 60 {
        return None;
    }
    let mut i = 19;
    let mut ms = 0i64;
    if b.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
        let frac = &s[start..i.min(start + 3)];
        ms = frac.parse::<i64>().ok()? * 10i64.pow(3 - frac.len() as u32);
    }
    let offset_min = match &s[i..] {
        "" | "Z" => 0,
        // bytes, not `&z[3..4]`: a non-ASCII character there is not a char
        // boundary, and slicing it would panic
        z if z.len() == 6
            && (z.starts_with('+') || z.starts_with('-'))
            && z.as_bytes()[3] == b':' =>
        {
            let hh: i64 = z[1..3].parse().ok()?;
            let mm: i64 = z[4..6].parse().ok()?;
            let sign = if z.starts_with('-') { -1 } else { 1 };
            sign * (hh * 60 + mm)
        }
        _ => return None,
    };
    let days = days_from_civil(y, mo, d);
    Some(((days * 24 + h) * 60 + mi - offset_min) * 60_000 + sec * 1000 + ms)
}

// ----------------------------------------------------------------- reading

fn trimmed(s: &str) -> Option<String> {
    let t = s.trim_matches(|c: char| c == ' ' || c == '\t' || c == '\r' || c == '\n');
    (!t.is_empty()).then(|| t.to_string())
}

/// A JSON scalar (or `{"value": scalar}`) as text.
fn scalar_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => trimmed(s),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Object(o) => o.get("value").and_then(scalar_text),
        _ => None,
    }
}

fn digits(s: &str, max: usize) -> Option<i64> {
    if s.is_empty() || s.len() > max || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Read view of one JSON object (absent, `null` and non-objects read as empty).
#[derive(Clone, Copy)]
struct R<'a>(Option<&'a Map<String, Value>>);

impl<'a> R<'a> {
    fn of(v: Option<&'a Value>) -> R<'a> {
        R(v.and_then(Value::as_object))
    }

    fn get(&self, k: &str) -> Option<&'a Value> {
        self.0?.get(k).filter(|v| !v.is_null())
    }

    fn has(&self, k: &str) -> bool {
        self.get(k).is_some_and(|v| v.is_object())
    }

    fn sub(&self, k: &str) -> R<'a> {
        R::of(self.get(k))
    }

    fn text(&self, k: &str) -> Option<String> {
        self.get(k).and_then(scalar_text)
    }

    fn time(&self, k: &str) -> Option<String> {
        let v = self.get(k)?;
        let inner = match v {
            Value::Object(o) => o.get("value")?,
            other => other,
        };
        match inner {
            Value::Number(n) => n.as_i64().map(millis_to_datetime),
            other => scalar_text(other),
        }
    }

    fn i32_digits(&self, k: &str) -> Option<i32> {
        self.text(k)
            .and_then(|t| digits(&t, 9))
            .and_then(|v| i32::try_from(v).ok())
    }

    fn i64_signed(&self, k: &str) -> Option<i64> {
        self.text(k).and_then(|t| t.parse().ok())
    }

    fn bool(&self, k: &str) -> Option<bool> {
        match self.get(k)? {
            Value::Bool(b) => Some(*b),
            v => match scalar_text(v)?.as_str() {
                "true" | "1" => Some(true),
                "false" | "0" => Some(false),
                _ => None,
            },
        }
    }

    fn enumv<T: for<'s> From<&'s str>>(&self, k: &str) -> Option<T> {
        self.text(k).map(|t| T::from(t.as_str()))
    }

    /// The objects of an array (a single object counts as a one-element array).
    fn list(&self, k: &str) -> Vec<R<'a>> {
        match self.get(k) {
            Some(Value::Array(a)) => a
                .iter()
                .filter(|v| v.is_object())
                .map(|v| R::of(Some(v)))
                .collect(),
            Some(v @ Value::Object(_)) => vec![R::of(Some(v))],
            _ => Vec::new(),
        }
    }

    fn strings(&self, k: &str) -> Vec<String> {
        match self.get(k) {
            Some(Value::Array(a)) => a.iter().filter_map(scalar_text).collect(),
            Some(v) => scalar_text(v).into_iter().collect(),
            None => Vec::new(),
        }
    }
}

fn r_meta(r: R) -> ItemMeta {
    let di = r.text("display-index");
    let display_index = di
        .as_deref()
        .and_then(|t| digits(t, 9))
        .and_then(|v| i32::try_from(v).ok());
    ItemMeta {
        put_code: r.text("put-code").and_then(|t| digits(&t, 18)),
        visibility: r.enumv("visibility"),
        display_index,
        display_index_text: di
            .filter(|t| display_index.map(|v| v.to_string()).as_deref() != Some(t.as_str())),
        path: r.text("path"),
        created_date: r.time("created-date"),
        last_modified_date: r.time("last-modified-date"),
        source: r.has("source").then(|| r_source(r.sub("source"))),
    }
}

fn r_ident(r: R) -> OrcidIdentifier {
    OrcidIdentifier {
        uri: r.text("uri"),
        path: r.text("path"),
        host: r.text("host"),
    }
}

fn opt_ident(r: R, k: &str) -> Option<OrcidIdentifier> {
    r.has(k).then(|| r_ident(r.sub(k)))
}

fn r_source(r: R) -> Source {
    Source {
        source_orcid: opt_ident(r, "source-orcid"),
        source_client_id: opt_ident(r, "source-client-id"),
        source_name: r.text("source-name"),
        assertion_origin_orcid: opt_ident(r, "assertion-origin-orcid"),
        assertion_origin_client_id: opt_ident(r, "assertion-origin-client-id"),
        assertion_origin_name: r.text("assertion-origin-name"),
    }
}

fn r_date(r: R, k: &str) -> Option<FuzzyDate> {
    r.has(k).then(|| {
        let d = r.sub(k);
        FuzzyDate {
            year: d.i32_digits("year"),
            month: d.i32_digits("month"),
            day: d.i32_digits("day"),
        }
    })
}

fn r_org(r: R) -> Organization {
    Organization {
        name: r.text("name"),
        address: r.has("address").then(|| {
            let a = r.sub("address");
            OrganizationAddress {
                city: a.text("city"),
                region: a.text("region"),
                country: a.text("country"),
            }
        }),
        disambiguated_organization: r.has("disambiguated-organization").then(|| {
            let d = r.sub("disambiguated-organization");
            DisambiguatedOrganization {
                identifier: d.text("disambiguated-organization-identifier"),
                source: d.text("disambiguation-source"),
            }
        }),
    }
}

fn opt_org(r: R, k: &str) -> Option<Organization> {
    r.has(k).then(|| r_org(r.sub(k)))
}

fn r_ext_id(r: R) -> ExternalId {
    ExternalId {
        meta: r_meta(r),
        id_type: r.text("external-id-type"),
        value: r.text("external-id-value"),
        normalized: r.get("external-id-normalized").map(|v| {
            let n = R::of(Some(v));
            TransientString {
                value: scalar_text(v),
                transient: n.bool("transient"),
            }
        }),
        normalized_error: r.has("external-id-normalized-error").then(|| {
            let e = r.sub("external-id-normalized-error");
            TransientError {
                error_code: e.text("error-code"),
                error_message: e.text("error-message"),
                transient: e.bool("transient"),
            }
        }),
        url: r.text("external-id-url"),
        relationship: r.enumv("external-id-relationship"),
    }
}

/// `{"external-id": [...]}` under key `k`.
fn r_ext_ids(r: R, k: &str) -> Vec<ExternalId> {
    r.sub(k)
        .list("external-id")
        .into_iter()
        .map(r_ext_id)
        .collect()
}

fn r_title(r: R, k: &str) -> Option<Title> {
    r.has(k).then(|| {
        let t = r.sub(k);
        Title {
            title: t.text("title"),
            subtitle: t.text("subtitle"),
            translated_title: t.get("translated-title").map(|v| {
                let tt = R::of(Some(v));
                TranslatedTitle {
                    value: scalar_text(v),
                    language_code: tt.text("language-code"),
                }
            }),
        }
    })
}

fn r_contributors(r: R) -> Vec<Contributor> {
    r.sub("contributors")
        .list("contributor")
        .into_iter()
        .map(|c| Contributor {
            contributor_orcid: opt_ident(c, "contributor-orcid"),
            credit_name: c.text("credit-name"),
            contributor_email: c.text("contributor-email"),
            attributes: c.has("contributor-attributes").then(|| {
                let a = c.sub("contributor-attributes");
                ContributorAttributes {
                    sequence: a.enumv("contributor-sequence"),
                    role: a.enumv("contributor-role"),
                }
            }),
        })
        .collect()
}

fn r_work(r: R) -> Work {
    Work {
        meta: r_meta(r),
        title: r_title(r, "title"),
        journal_title: r.text("journal-title"),
        short_description: r.text("short-description"),
        citation: r.has("citation").then(|| {
            let c = r.sub("citation");
            Citation {
                citation_type: c.enumv("citation-type"),
                citation_value: c.text("citation-value"),
            }
        }),
        work_type: r.enumv("type"),
        publication_date: r_date(r, "publication-date"),
        external_ids: r_ext_ids(r, "external-ids"),
        url: r.text("url"),
        contributors: r_contributors(r),
        language_code: r.text("language-code"),
        country: r.get("country").map(|v| Country {
            value: scalar_text(v),
            visibility: R::of(Some(v)).enumv("visibility"),
        }),
    }
}

fn r_work_summary(r: R) -> WorkSummary {
    WorkSummary {
        meta: r_meta(r),
        title: r_title(r, "title"),
        external_ids: r_ext_ids(r, "external-ids"),
        url: r.text("url"),
        work_type: r.enumv("type"),
        publication_date: r_date(r, "publication-date"),
        journal_title: r.text("journal-title"),
    }
}

fn r_works(r: R) -> Works {
    Works {
        path: r.text("path"),
        last_modified_date: r.time("last-modified-date"),
        groups: r
            .list("group")
            .into_iter()
            .map(|g| WorkGroup {
                last_modified_date: g.time("last-modified-date"),
                external_ids: r_ext_ids(g, "external-ids"),
                work_summaries: g
                    .list("work-summary")
                    .into_iter()
                    .map(r_work_summary)
                    .collect(),
            })
            .collect(),
    }
}

fn r_amount(r: R) -> Option<Amount> {
    r.get("amount").map(|v| Amount {
        value: scalar_text(v),
        currency_code: R::of(Some(v)).text("currency-code"),
    })
}

fn r_funding(r: R) -> Funding {
    Funding {
        meta: r_meta(r),
        funding_type: r.enumv("type"),
        organization_defined_type: r.text("organization-defined-type"),
        title: r_title(r, "title"),
        short_description: r.text("short-description"),
        amount: r_amount(r),
        url: r.text("url"),
        start_date: r_date(r, "start-date"),
        end_date: r_date(r, "end-date"),
        external_ids: r_ext_ids(r, "external-ids"),
        contributors: r_contributors(r),
        organization: opt_org(r, "organization"),
    }
}

fn r_funding_summary(r: R) -> FundingSummary {
    FundingSummary {
        meta: r_meta(r),
        title: r_title(r, "title"),
        external_ids: r_ext_ids(r, "external-ids"),
        url: r.text("url"),
        funding_type: r.enumv("type"),
        start_date: r_date(r, "start-date"),
        end_date: r_date(r, "end-date"),
        organization: opt_org(r, "organization"),
    }
}

fn r_fundings(r: R) -> Fundings {
    Fundings {
        path: r.text("path"),
        last_modified_date: r.time("last-modified-date"),
        groups: r
            .list("group")
            .into_iter()
            .map(|g| FundingGroup {
                last_modified_date: g.time("last-modified-date"),
                external_ids: r_ext_ids(g, "external-ids"),
                funding_summaries: g
                    .list("funding-summary")
                    .into_iter()
                    .map(r_funding_summary)
                    .collect(),
            })
            .collect(),
    }
}

fn r_affiliation(kind: AffiliationKind, r: R) -> Affiliation {
    Affiliation {
        kind,
        meta: r_meta(r),
        department_name: r.text("department-name"),
        role_title: r.text("role-title"),
        start_date: r_date(r, "start-date"),
        end_date: r_date(r, "end-date"),
        organization: opt_org(r, "organization"),
        url: r.text("url"),
        external_ids: r_ext_ids(r, "external-ids"),
    }
}

fn r_affiliations(kind: AffiliationKind, r: R) -> Affiliations {
    Affiliations {
        kind,
        path: r.text("path"),
        last_modified_date: r.time("last-modified-date"),
        groups: r
            .list("affiliation-group")
            .into_iter()
            .map(|g| {
                let mut summaries = Vec::new();
                for wrapper in g.list("summaries") {
                    for k in AffiliationKind::ALL {
                        for s in wrapper.list(k.summary_element()) {
                            summaries.push(r_affiliation(k, s));
                        }
                    }
                }
                AffiliationGroup {
                    last_modified_date: g.time("last-modified-date"),
                    external_ids: r_ext_ids(g, "external-ids"),
                    summaries,
                }
            })
            .collect(),
    }
}

fn r_peer_review_summary(r: R) -> PeerReviewSummary {
    PeerReviewSummary {
        meta: r_meta(r),
        reviewer_role: r.enumv("reviewer-role"),
        external_ids: r_ext_ids(r, "external-ids"),
        review_url: r.text("review-url"),
        review_type: r.enumv("review-type"),
        completion_date: r_date(r, "completion-date"),
        review_group_id: r.text("review-group-id"),
        convening_organization: opt_org(r, "convening-organization"),
    }
}

fn r_peer_review(r: R) -> PeerReview {
    PeerReview {
        meta: r_meta(r),
        reviewer_role: r.enumv("reviewer-role"),
        review_identifiers: r_ext_ids(r, "review-identifiers"),
        review_url: r.text("review-url"),
        review_type: r.enumv("review-type"),
        review_completion_date: r_date(r, "review-completion-date"),
        review_group_id: r.text("review-group-id"),
        subject_external_identifier: r
            .has("subject-external-identifier")
            .then(|| r_ext_id(r.sub("subject-external-identifier"))),
        subject_container_name: r.text("subject-container-name"),
        subject_type: r.text("subject-type"),
        subject_name: r_title(r, "subject-name"),
        subject_url: r.text("subject-url"),
        convening_organization: opt_org(r, "convening-organization"),
    }
}

fn r_peer_reviews(r: R) -> PeerReviews {
    PeerReviews {
        path: r.text("path"),
        last_modified_date: r.time("last-modified-date"),
        groups: r
            .list("group")
            .into_iter()
            .map(|g| PeerReviewGroup {
                last_modified_date: g.time("last-modified-date"),
                external_ids: r_ext_ids(g, "external-ids"),
                duplicates: g
                    .list("peer-review-group")
                    .into_iter()
                    .map(|d| PeerReviewDuplicates {
                        last_modified_date: d.time("last-modified-date"),
                        external_ids: r_ext_ids(d, "external-ids"),
                        summaries: d
                            .list("peer-review-summary")
                            .into_iter()
                            .map(r_peer_review_summary)
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn r_hosts(r: R) -> Vec<Organization> {
    r.sub("hosts")
        .list("organization")
        .into_iter()
        .map(r_org)
        .collect()
}

fn r_proposal(r: R) -> Option<Proposal> {
    r.has("proposal").then(|| {
        let p = r.sub("proposal");
        Proposal {
            title: r_title(p, "title"),
            hosts: r_hosts(p),
            external_ids: r_ext_ids(p, "external-ids"),
            start_date: r_date(p, "start-date"),
            end_date: r_date(p, "end-date"),
            url: p.text("url"),
        }
    })
}

fn r_research_resource(r: R) -> ResearchResource {
    // `resource-item` directly (the samples), or wrapped in `resource-items`.
    let mut items = r.list("resource-item");
    items.extend(r.sub("resource-items").list("resource-item"));
    ResearchResource {
        meta: r_meta(r),
        proposal: r_proposal(r),
        resource_items: items
            .into_iter()
            .map(|i| ResourceItem {
                resource_name: i.text("resource-name"),
                resource_type: i.enumv("resource-type"),
                hosts: r_hosts(i),
                external_ids: r_ext_ids(i, "external-ids"),
                url: i.text("url"),
            })
            .collect(),
    }
}

fn r_research_resource_summary(r: R) -> ResearchResourceSummary {
    ResearchResourceSummary {
        meta: r_meta(r),
        proposal: r_proposal(r),
    }
}

fn r_research_resources(r: R) -> ResearchResources {
    ResearchResources {
        path: r.text("path"),
        last_modified_date: r.time("last-modified-date"),
        groups: r
            .list("group")
            .into_iter()
            .map(|g| ResearchResourceGroup {
                last_modified_date: g.time("last-modified-date"),
                external_ids: r_ext_ids(g, "external-ids"),
                summaries: g
                    .list("research-resource-summary")
                    .into_iter()
                    .map(r_research_resource_summary)
                    .collect(),
            })
            .collect(),
    }
}

fn r_activities(r: R) -> ActivitiesSummary {
    let aff = |k: AffiliationKind| {
        r.has(k.container_element())
            .then(|| r_affiliations(k, r.sub(k.container_element())))
    };
    ActivitiesSummary {
        path: r.text("path"),
        last_modified_date: r.time("last-modified-date"),
        distinctions: aff(AffiliationKind::Distinction),
        educations: aff(AffiliationKind::Education),
        employments: aff(AffiliationKind::Employment),
        fundings: r.has("fundings").then(|| r_fundings(r.sub("fundings"))),
        invited_positions: aff(AffiliationKind::InvitedPosition),
        memberships: aff(AffiliationKind::Membership),
        peer_reviews: r
            .has("peer-reviews")
            .then(|| r_peer_reviews(r.sub("peer-reviews"))),
        qualifications: aff(AffiliationKind::Qualification),
        research_resources: r
            .has("research-resources")
            .then(|| r_research_resources(r.sub("research-resources"))),
        services: aff(AffiliationKind::Service),
        works: r.has("works").then(|| r_works(r.sub("works"))),
    }
}

fn r_name(r: R) -> Name {
    Name {
        visibility: r.enumv("visibility"),
        path: r.text("path"),
        created_date: r.time("created-date"),
        last_modified_date: r.time("last-modified-date"),
        given_names: r.text("given-names"),
        family_name: r.text("family-name"),
        credit_name: r.text("credit-name"),
    }
}

fn r_biography(r: R) -> Biography {
    Biography {
        visibility: r.enumv("visibility"),
        path: r.text("path"),
        created_date: r.time("created-date"),
        last_modified_date: r.time("last-modified-date"),
        content: r.text("content"),
    }
}

fn r_other_name(r: R) -> OtherName {
    OtherName {
        meta: r_meta(r),
        content: r.text("content"),
    }
}

fn r_researcher_url(r: R) -> ResearcherUrl {
    ResearcherUrl {
        meta: r_meta(r),
        url_name: r.text("url-name"),
        url: r.text("url"),
    }
}

fn r_email(r: R) -> Email {
    Email {
        meta: r_meta(r),
        email: r.text("email"),
        verified: r.bool("verified"),
        primary: r.bool("primary"),
    }
}

fn r_address(r: R) -> Address {
    Address {
        meta: r_meta(r),
        country: r.text("country"),
    }
}

fn r_keyword(r: R) -> Keyword {
    Keyword {
        meta: r_meta(r),
        content: r.text("content"),
    }
}

fn r_list<T>(r: R, item: &str, f: fn(R) -> T) -> ItemList<T> {
    ItemList {
        path: r.text("path"),
        last_modified_date: r.time("last-modified-date"),
        items: r.list(item).into_iter().map(f).collect(),
    }
}

fn r_person(r: R) -> Person {
    Person {
        path: r.text("path"),
        last_modified_date: r.time("last-modified-date"),
        name: r.has("name").then(|| r_name(r.sub("name"))),
        other_names: r
            .has("other-names")
            .then(|| r_list(r.sub("other-names"), "other-name", r_other_name)),
        biography: r.has("biography").then(|| r_biography(r.sub("biography"))),
        researcher_urls: r
            .has("researcher-urls")
            .then(|| r_list(r.sub("researcher-urls"), "researcher-url", r_researcher_url)),
        emails: r
            .has("emails")
            .then(|| r_list(r.sub("emails"), "email", r_email)),
        addresses: r
            .has("addresses")
            .then(|| r_list(r.sub("addresses"), "address", r_address)),
        keywords: r
            .has("keywords")
            .then(|| r_list(r.sub("keywords"), "keyword", r_keyword)),
        external_identifiers: r.has("external-identifiers").then(|| {
            r_list(
                r.sub("external-identifiers"),
                "external-identifier",
                r_ext_id,
            )
        }),
    }
}

fn r_personal_details(r: R) -> PersonalDetails {
    PersonalDetails {
        path: r.text("path"),
        last_modified_date: r.time("last-modified-date"),
        name: r.has("name").then(|| r_name(r.sub("name"))),
        other_names: r
            .has("other-names")
            .then(|| r_list(r.sub("other-names"), "other-name", r_other_name)),
        biography: r.has("biography").then(|| r_biography(r.sub("biography"))),
    }
}

fn creation_method_from_json(s: &str) -> CreationMethod {
    match s {
        "API" => CreationMethod::Api,
        "DIRECT" => CreationMethod::Direct,
        "MEMBER_REFERRED" => CreationMethod::MemberReferred,
        "WEBSITE" => CreationMethod::Website,
        other => CreationMethod::from(other),
    }
}

fn creation_method_to_json(m: &CreationMethod) -> String {
    match m {
        CreationMethod::Api => "API".to_string(),
        CreationMethod::Direct => "DIRECT".to_string(),
        CreationMethod::MemberReferred => "MEMBER_REFERRED".to_string(),
        CreationMethod::Website => "WEBSITE".to_string(),
        CreationMethod::Other(s) => s.clone(),
    }
}

fn r_history(r: R) -> History {
    History {
        visibility: r.enumv("visibility"),
        creation_method: r
            .text("creation-method")
            .map(|t| creation_method_from_json(&t)),
        completion_date: r.time("completion-date"),
        submission_date: r.time("submission-date"),
        last_modified_date: r.time("last-modified-date"),
        claimed: r.bool("claimed"),
        source: r.has("source").then(|| r_source(r.sub("source"))),
        deactivation_date: r.time("deactivation-date"),
        verified_email: r.bool("verified-email"),
        verified_primary_email: r.bool("verified-primary-email"),
    }
}

fn r_record(r: R) -> Record {
    Record {
        path: r.text("path"),
        orcid_identifier: opt_ident(r, "orcid-identifier"),
        preferences: r.has("preferences").then(|| Preferences {
            locale: r.sub("preferences").text("locale"),
        }),
        history: r.has("history").then(|| r_history(r.sub("history"))),
        person: r.has("person").then(|| r_person(r.sub("person"))),
        activities_summary: r
            .has("activities-summary")
            .then(|| r_activities(r.sub("activities-summary"))),
    }
}

fn r_error(r: R) -> OrcidError {
    OrcidError {
        response_code: r.i64_signed("response-code"),
        developer_message: r.text("developer-message"),
        user_message: r.text("user-message"),
        error_code: r.i64_signed("error-code"),
        more_info: r.text("more-info"),
    }
}

fn r_bulk(r: R) -> Bulk {
    let mut items = Vec::new();
    for e in r.list("bulk") {
        if e.has("work") {
            items.push(BulkItem::Work(r_work(e.sub("work"))));
        } else if e.has("error") {
            items.push(BulkItem::Error(r_error(e.sub("error"))));
        }
    }
    Bulk { items }
}

fn r_search(r: R) -> Search {
    Search {
        num_found: r.i64_signed("num-found"),
        results: r
            .list("result")
            .into_iter()
            .filter(|x| x.has("orcid-identifier"))
            .map(|x| r_ident(x.sub("orcid-identifier")))
            .collect(),
    }
}

fn r_expanded_search(r: R) -> ExpandedSearch {
    ExpandedSearch {
        num_found: r.i64_signed("num-found"),
        results: r
            .list("expanded-result")
            .into_iter()
            .map(|x| ExpandedResult {
                orcid_id: x.text("orcid-id"),
                given_names: x.text("given-names"),
                family_names: x.text("family-names"),
                credit_name: x.text("credit-name"),
                other_names: x.strings("other-name"),
                emails: x.strings("email"),
                institution_names: x.strings("institution-name"),
            })
            .collect(),
    }
}

/// `group-id-record`: `source`, or the inline source keys of the XSD's
/// common-2.1 `source-type` (`source-orcid`, `source-client-id`,
/// `source-name`), which fill what `source` leaves empty.
fn r_group_id_record(r: R) -> GroupIdRecord {
    let mut source = r.has("source").then(|| r_source(r.sub("source")));
    let inline_orcid = opt_ident(r, "source-orcid");
    let inline_client = opt_ident(r, "source-client-id");
    let inline_name = r.text("source-name");
    if inline_orcid.is_some() || inline_client.is_some() || inline_name.is_some() {
        let s = source.get_or_insert_with(Source::default);
        if s.source_orcid.is_none() {
            s.source_orcid = inline_orcid;
        }
        if s.source_client_id.is_none() {
            s.source_client_id = inline_client;
        }
        if s.source_name.is_none() {
            s.source_name = inline_name;
        }
    }
    GroupIdRecord {
        put_code: r.text("put-code").and_then(|t| digits(&t, 18)),
        created_date: r.time("created-date"),
        last_modified_date: r.time("last-modified-date"),
        name: r.text("name"),
        group_id: r.text("group-id"),
        description: r.text("description"),
        group_type: r.enumv("type"),
        source,
    }
}

fn r_group_id_records(r: R) -> GroupIdRecords {
    GroupIdRecords {
        last_modified_date: r.time("last-modified-date"),
        total: r.text("total").and_then(|t| digits(&t, 9)),
        page: r.text("page").and_then(|t| digits(&t, 9)),
        page_size: r.text("page-size").and_then(|t| digits(&t, 9)),
        records: r
            .list("group-id-record")
            .into_iter()
            .map(r_group_id_record)
            .collect(),
    }
}

/// Reads a JSON document of the given kind (the XML root local name, see [`KINDS`]).
///
/// ```
/// let doc = yorc::json::from_str("keyword", r#"{"content": "vents", "put-code": 7}"#)?;
/// let yorc::model::Root::Keyword(k) = doc.root else { panic!() };
/// assert_eq!(k.content.as_deref(), Some("vents"));
/// assert_eq!(k.meta.put_code, Some(7));
/// # Ok::<(), yorc::Error>(())
/// ```
pub fn from_str(kind: &str, json: &str) -> Result<Document> {
    let v: Value = serde_json::from_str(json).map_err(|e| Error::Json {
        message: e.to_string(),
    })?;
    from_value(kind, &v)
}

/// [`from_str`] over bytes (UTF-8, optional BOM).
///
/// ```
/// let doc = yorc::json::from_slice("preferences", br#"{"locale": "en"}"#)?;
/// assert_eq!(doc.root_name, "preferences");
/// # Ok::<(), yorc::Error>(())
/// ```
pub fn from_slice(kind: &str, bytes: &[u8]) -> Result<Document> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let s = std::str::from_utf8(bytes).map_err(|e| Error::Utf8 {
        valid_up_to: e.valid_up_to(),
    })?;
    from_str(kind, s)
}

/// Maps a parsed JSON value of the given kind onto the model.
///
/// ```
/// let v = serde_json::json!({"content": {"value": "vents"}});
/// let doc = yorc::json::from_value("keyword", &v)?;
/// assert_eq!(doc.root_name, "keyword");
/// # Ok::<(), yorc::Error>(())
/// ```
pub fn from_value(kind: &str, v: &Value) -> Result<Document> {
    if !v.is_object() {
        return Err(Error::Json {
            message: format!("a {kind} document must be a JSON object"),
        });
    }
    let r = R::of(Some(v));
    let root = match kind {
        "record" => Root::Record(r_record(r)),
        "activities-summary" => Root::ActivitiesSummary(r_activities(r)),
        "works" => Root::Works(r_works(r)),
        "work" => Root::Work(r_work(r)),
        "work-summary" => Root::WorkSummary(r_work_summary(r)),
        "fundings" => Root::Fundings(r_fundings(r)),
        "funding" => Root::Funding(r_funding(r)),
        "funding-summary" => Root::FundingSummary(r_funding_summary(r)),
        "peer-reviews" => Root::PeerReviews(r_peer_reviews(r)),
        "peer-review" => Root::PeerReview(r_peer_review(r)),
        "peer-review-summary" => Root::PeerReviewSummary(r_peer_review_summary(r)),
        "research-resources" => Root::ResearchResources(r_research_resources(r)),
        "research-resource" => Root::ResearchResource(r_research_resource(r)),
        "research-resource-summary" => {
            Root::ResearchResourceSummary(r_research_resource_summary(r))
        }
        "person" => Root::Person(r_person(r)),
        "personal-details" => Root::PersonalDetails(r_personal_details(r)),
        "name" => Root::Name(r_name(r)),
        "biography" => Root::Biography(r_biography(r)),
        "credit-name" => Root::CreditName(scalar_text(v)),
        "other-names" => Root::OtherNames(r_list(r, "other-name", r_other_name)),
        "other-name" => Root::OtherName(r_other_name(r)),
        "researcher-urls" => Root::ResearcherUrls(r_list(r, "researcher-url", r_researcher_url)),
        "researcher-url" => Root::ResearcherUrl(r_researcher_url(r)),
        "emails" => Root::Emails(r_list(r, "email", r_email)),
        "email" => Root::Email(r_email(r)),
        "addresses" => Root::Addresses(r_list(r, "address", r_address)),
        "address" => Root::Address(r_address(r)),
        "keywords" => Root::Keywords(r_list(r, "keyword", r_keyword)),
        "keyword" => Root::Keyword(r_keyword(r)),
        "external-identifiers" => {
            Root::ExternalIdentifiers(r_list(r, "external-identifier", r_ext_id))
        }
        "external-identifier" => Root::ExternalIdentifier(r_ext_id(r)),
        "history" => Root::History(r_history(r)),
        "preferences" => Root::Preferences(Preferences {
            locale: r.text("locale"),
        }),
        "bulk" => Root::Bulk(r_bulk(r)),
        "error" => Root::Error(r_error(r)),
        "search" => Root::Search(r_search(r)),
        "expanded-search" => Root::ExpandedSearch(r_expanded_search(r)),
        "group-id-record" => Root::GroupIdRecord(r_group_id_record(r)),
        "group-id-records" => Root::GroupIdRecords(r_group_id_records(r)),
        other => {
            if let Some(k) = AffiliationKind::from_container_element(other) {
                Root::Affiliations(r_affiliations(k, r))
            } else if let Some(k) = AffiliationKind::from_element(other) {
                Root::Affiliation(r_affiliation(k, r))
            } else if let Some(k) = AffiliationKind::from_summary_element(other) {
                Root::AffiliationSummary(r_affiliation(k, r))
            } else {
                return Err(Error::Json {
                    message: format!("unknown document kind {other:?}"),
                });
            }
        }
    };
    let root_path = match &root {
        Root::Record(x) => x.path.clone(),
        _ => r.text("path"),
    };
    Ok(Document {
        root_name: kind.to_string(),
        root_path,
        root,
    })
}

// ----------------------------------------------------------------- writing

/// Write builder for one JSON object.
struct W {
    m: Map<String, Value>,
    path: String,
}

fn value_obj(s: &str) -> Value {
    let mut m = Map::new();
    m.insert("value".to_string(), Value::String(s.to_string()));
    Value::Object(m)
}

impl W {
    fn new(path: &str) -> W {
        W {
            m: Map::new(),
            path: path.to_string(),
        }
    }

    fn at(&self, k: &str) -> String {
        format!("{}/{k}", self.path)
    }

    /// Bare string.
    fn s(&mut self, k: &str, v: &Option<String>) {
        if let Some(v) = v {
            self.m.insert(k.to_string(), Value::String(v.clone()));
        }
    }

    /// `{"value": "..."}`.
    fn v(&mut self, k: &str, v: &Option<String>) {
        if let Some(v) = v {
            self.m.insert(k.to_string(), value_obj(v));
        }
    }

    /// `{"value": <epoch ms>}`.
    fn t(&mut self, k: &str, v: &Option<String>) -> Result<()> {
        if let Some(v) = v {
            let ms = datetime_to_millis(v).ok_or_else(|| Error::Unwritable {
                path: self.at(k),
                reason: format!("{v:?} is not an xs:dateTime"),
            })?;
            let mut m = Map::new();
            m.insert("value".to_string(), Value::from(ms));
            self.m.insert(k.to_string(), Value::Object(m));
        }
        Ok(())
    }

    fn b(&mut self, k: &str, v: Option<bool>) {
        if let Some(v) = v {
            self.m.insert(k.to_string(), Value::Bool(v));
        }
    }

    fn n(&mut self, k: &str, v: Option<i64>) {
        if let Some(v) = v {
            self.m.insert(k.to_string(), Value::from(v));
        }
    }

    fn e<T: std::fmt::Display>(&mut self, k: &str, v: &Option<T>) {
        if let Some(v) = v {
            self.m.insert(k.to_string(), Value::String(v.to_string()));
        }
    }

    fn o(&mut self, k: &str, v: Option<Value>) {
        if let Some(v) = v {
            self.m.insert(k.to_string(), v);
        }
    }

    fn a(&mut self, k: &str, v: Vec<Value>) {
        self.m.insert(k.to_string(), Value::Array(v));
    }

    fn done(self) -> Value {
        Value::Object(self.m)
    }
}

fn w_meta(w: &mut W, m: &ItemMeta) -> Result<()> {
    w.n("put-code", m.put_code);
    w.s("path", &m.path);
    w.e("visibility", &m.visibility);
    match (&m.display_index_text, m.display_index) {
        (Some(t), _) => w.s("display-index", &Some(t.clone())),
        (None, Some(i)) => w.n("display-index", Some(i64::from(i))),
        _ => {}
    }
    w.t("created-date", &m.created_date)?;
    w.t("last-modified-date", &m.last_modified_date)?;
    if let Some(s) = &m.source {
        let v = w_source(&w.at("source"), s);
        w.o("source", Some(v));
    }
    Ok(())
}

fn w_ident(i: &OrcidIdentifier) -> Value {
    let mut w = W::new("");
    w.s("uri", &i.uri);
    w.s("path", &i.path);
    w.s("host", &i.host);
    w.done()
}

fn w_source(path: &str, s: &Source) -> Value {
    let mut w = W::new(path);
    w.o("source-orcid", s.source_orcid.as_ref().map(w_ident));
    w.o("source-client-id", s.source_client_id.as_ref().map(w_ident));
    w.v("source-name", &s.source_name);
    w.o(
        "assertion-origin-orcid",
        s.assertion_origin_orcid.as_ref().map(w_ident),
    );
    w.o(
        "assertion-origin-client-id",
        s.assertion_origin_client_id.as_ref().map(w_ident),
    );
    w.v("assertion-origin-name", &s.assertion_origin_name);
    w.done()
}

fn w_date(d: &Option<FuzzyDate>) -> Option<Value> {
    d.as_ref().map(|d| {
        let mut w = W::new("");
        w.v("year", &d.year.map(|y| format!("{y:04}")));
        w.v("month", &d.month.map(|m| format!("{m:02}")));
        w.v("day", &d.day.map(|x| format!("{x:02}")));
        w.done()
    })
}

fn w_org(o: &Organization) -> Value {
    let mut w = W::new("");
    w.s("name", &o.name);
    if let Some(a) = &o.address {
        let mut aw = W::new("");
        aw.s("city", &a.city);
        aw.s("region", &a.region);
        aw.s("country", &a.country);
        w.o("address", Some(aw.done()));
    }
    if let Some(d) = &o.disambiguated_organization {
        let mut dw = W::new("");
        dw.s("disambiguated-organization-identifier", &d.identifier);
        dw.s("disambiguation-source", &d.source);
        w.o("disambiguated-organization", Some(dw.done()));
    }
    w.done()
}

fn w_ext_id(path: &str, x: &ExternalId) -> Result<Value> {
    let mut w = W::new(path);
    w_meta(&mut w, &x.meta)?;
    w.s("external-id-type", &x.id_type);
    w.s("external-id-value", &x.value);
    if let Some(n) = &x.normalized {
        let mut nw = W::new("");
        nw.s("value", &n.value);
        nw.b("transient", n.transient);
        w.o("external-id-normalized", Some(nw.done()));
    }
    if let Some(e) = &x.normalized_error {
        let mut ew = W::new("");
        ew.s("error-code", &e.error_code);
        ew.s("error-message", &e.error_message);
        ew.b("transient", e.transient);
        w.o("external-id-normalized-error", Some(ew.done()));
    }
    w.v("external-id-url", &x.url);
    w.e("external-id-relationship", &x.relationship);
    Ok(w.done())
}

/// `{"external-id": [...]}`.
fn w_ext_ids(path: &str, ids: &[ExternalId]) -> Result<Value> {
    let mut w = W::new(path);
    let items = ids
        .iter()
        .enumerate()
        .map(|(i, x)| w_ext_id(&format!("{path}/external-id[{}]", i + 1), x))
        .collect::<Result<Vec<_>>>()?;
    w.a("external-id", items);
    Ok(w.done())
}

fn opt_ext_ids(w: &mut W, k: &str, ids: &[ExternalId]) -> Result<()> {
    if !ids.is_empty() {
        let v = w_ext_ids(&w.at(k), ids)?;
        w.o(k, Some(v));
    }
    Ok(())
}

fn req_ext_ids(w: &mut W, k: &str, ids: &[ExternalId]) -> Result<()> {
    let v = w_ext_ids(&w.at(k), ids)?;
    w.o(k, Some(v));
    Ok(())
}

fn w_title(t: &Option<Title>) -> Option<Value> {
    t.as_ref().map(|t| {
        let mut w = W::new("");
        w.v("title", &t.title);
        w.v("subtitle", &t.subtitle);
        if let Some(tt) = &t.translated_title {
            let mut tw = W::new("");
            tw.s("value", &tt.value);
            tw.s("language-code", &tt.language_code);
            w.o("translated-title", Some(tw.done()));
        }
        w.done()
    })
}

fn w_contributors(w: &mut W, cs: &[Contributor]) {
    if cs.is_empty() {
        return;
    }
    let items = cs
        .iter()
        .map(|c| {
            let mut cw = W::new("");
            cw.o(
                "contributor-orcid",
                c.contributor_orcid.as_ref().map(w_ident),
            );
            cw.v("credit-name", &c.credit_name);
            cw.s("contributor-email", &c.contributor_email);
            if let Some(a) = &c.attributes {
                let mut aw = W::new("");
                aw.e("contributor-sequence", &a.sequence);
                aw.e("contributor-role", &a.role);
                cw.o("contributor-attributes", Some(aw.done()));
            }
            cw.done()
        })
        .collect();
    let mut list = W::new("");
    list.a("contributor", items);
    w.o("contributors", Some(list.done()));
}

fn w_work(path: &str, x: &Work) -> Result<Value> {
    let mut w = W::new(path);
    w_meta(&mut w, &x.meta)?;
    w.o("title", w_title(&x.title));
    w.v("journal-title", &x.journal_title);
    w.s("short-description", &x.short_description);
    if let Some(c) = &x.citation {
        let mut cw = W::new("");
        cw.e("citation-type", &c.citation_type);
        cw.s("citation-value", &c.citation_value);
        w.o("citation", Some(cw.done()));
    }
    w.e("type", &x.work_type);
    w.o("publication-date", w_date(&x.publication_date));
    opt_ext_ids(&mut w, "external-ids", &x.external_ids)?;
    w.v("url", &x.url);
    w_contributors(&mut w, &x.contributors);
    w.s("language-code", &x.language_code);
    if let Some(c) = &x.country {
        let mut cw = W::new("");
        cw.s("value", &c.value);
        // country/@visibility is XML-only, like history/@visibility: ORCID's
        // `Country` has no such member (its JSON reader would reject it)
        w.o("country", Some(cw.done()));
    }
    Ok(w.done())
}

fn w_work_summary(path: &str, x: &WorkSummary) -> Result<Value> {
    let mut w = W::new(path);
    w_meta(&mut w, &x.meta)?;
    w.o("title", w_title(&x.title));
    opt_ext_ids(&mut w, "external-ids", &x.external_ids)?;
    w.v("url", &x.url);
    w.e("type", &x.work_type);
    w.o("publication-date", w_date(&x.publication_date));
    w.v("journal-title", &x.journal_title);
    Ok(w.done())
}

/// A container: `path`, `last-modified-date`.
fn w_container(path: &str, p: &Option<String>, lmd: &Option<String>) -> Result<W> {
    let mut w = W::new(path);
    w.s("path", p);
    w.t("last-modified-date", lmd)?;
    Ok(w)
}

/// A group: `last-modified-date`, `external-ids` (always).
fn w_group(path: &str, lmd: &Option<String>, ids: &[ExternalId]) -> Result<W> {
    let mut w = W::new(path);
    w.t("last-modified-date", lmd)?;
    req_ext_ids(&mut w, "external-ids", ids)?;
    Ok(w)
}

fn w_works(path: &str, x: &Works) -> Result<Value> {
    let mut w = w_container(path, &x.path, &x.last_modified_date)?;
    let mut groups = Vec::new();
    for (i, g) in x.groups.iter().enumerate() {
        let gp = format!("{path}/group[{}]", i + 1);
        let mut gw = w_group(&gp, &g.last_modified_date, &g.external_ids)?;
        let items = g
            .work_summaries
            .iter()
            .map(|s| w_work_summary(&format!("{gp}/work-summary"), s))
            .collect::<Result<Vec<_>>>()?;
        gw.a("work-summary", items);
        groups.push(gw.done());
    }
    w.a("group", groups);
    Ok(w.done())
}

fn w_amount(a: &Option<Amount>) -> Option<Value> {
    a.as_ref().map(|a| {
        let mut w = W::new("");
        w.s("value", &a.value);
        w.s("currency-code", &a.currency_code);
        w.done()
    })
}

fn w_funding(path: &str, x: &Funding) -> Result<Value> {
    let mut w = W::new(path);
    w_meta(&mut w, &x.meta)?;
    w.e("type", &x.funding_type);
    w.v("organization-defined-type", &x.organization_defined_type);
    w.o("title", w_title(&x.title));
    w.s("short-description", &x.short_description);
    w.o("amount", w_amount(&x.amount));
    w.v("url", &x.url);
    w.o("start-date", w_date(&x.start_date));
    w.o("end-date", w_date(&x.end_date));
    opt_ext_ids(&mut w, "external-ids", &x.external_ids)?;
    w_contributors(&mut w, &x.contributors);
    w.o("organization", x.organization.as_ref().map(w_org));
    Ok(w.done())
}

fn w_funding_summary(path: &str, x: &FundingSummary) -> Result<Value> {
    let mut w = W::new(path);
    w_meta(&mut w, &x.meta)?;
    w.o("title", w_title(&x.title));
    opt_ext_ids(&mut w, "external-ids", &x.external_ids)?;
    w.v("url", &x.url);
    w.e("type", &x.funding_type);
    w.o("start-date", w_date(&x.start_date));
    w.o("end-date", w_date(&x.end_date));
    w.o("organization", x.organization.as_ref().map(w_org));
    Ok(w.done())
}

fn w_fundings(path: &str, x: &Fundings) -> Result<Value> {
    let mut w = w_container(path, &x.path, &x.last_modified_date)?;
    let mut groups = Vec::new();
    for (i, g) in x.groups.iter().enumerate() {
        let gp = format!("{path}/group[{}]", i + 1);
        let mut gw = w_group(&gp, &g.last_modified_date, &g.external_ids)?;
        let items = g
            .funding_summaries
            .iter()
            .map(|s| w_funding_summary(&format!("{gp}/funding-summary"), s))
            .collect::<Result<Vec<_>>>()?;
        gw.a("funding-summary", items);
        groups.push(gw.done());
    }
    w.a("group", groups);
    Ok(w.done())
}

fn w_affiliation(path: &str, x: &Affiliation) -> Result<Value> {
    let mut w = W::new(path);
    w_meta(&mut w, &x.meta)?;
    w.s("department-name", &x.department_name);
    w.s("role-title", &x.role_title);
    w.o("start-date", w_date(&x.start_date));
    w.o("end-date", w_date(&x.end_date));
    w.o("organization", x.organization.as_ref().map(w_org));
    w.v("url", &x.url);
    opt_ext_ids(&mut w, "external-ids", &x.external_ids)?;
    Ok(w.done())
}

fn w_affiliations(path: &str, x: &Affiliations) -> Result<Value> {
    let mut w = w_container(path, &x.path, &x.last_modified_date)?;
    let mut groups = Vec::new();
    for (i, g) in x.groups.iter().enumerate() {
        let gp = format!("{path}/affiliation-group[{}]", i + 1);
        let mut gw = w_group(&gp, &g.last_modified_date, &g.external_ids)?;
        let mut summaries = Vec::new();
        for s in &g.summaries {
            let mut sw = W::new("");
            let v = w_affiliation(&format!("{gp}/{}", s.kind.summary_element()), s)?;
            sw.o(s.kind.summary_element(), Some(v));
            summaries.push(sw.done());
        }
        gw.a("summaries", summaries);
        groups.push(gw.done());
    }
    w.a("affiliation-group", groups);
    Ok(w.done())
}

fn w_peer_review_summary(path: &str, x: &PeerReviewSummary) -> Result<Value> {
    let mut w = W::new(path);
    w_meta(&mut w, &x.meta)?;
    w.e("reviewer-role", &x.reviewer_role);
    opt_ext_ids(&mut w, "external-ids", &x.external_ids)?;
    w.v("review-url", &x.review_url);
    w.e("review-type", &x.review_type);
    w.o("completion-date", w_date(&x.completion_date));
    w.s("review-group-id", &x.review_group_id);
    w.o(
        "convening-organization",
        x.convening_organization.as_ref().map(w_org),
    );
    Ok(w.done())
}

fn w_peer_review(path: &str, x: &PeerReview) -> Result<Value> {
    let mut w = W::new(path);
    w_meta(&mut w, &x.meta)?;
    w.e("reviewer-role", &x.reviewer_role);
    req_ext_ids(&mut w, "review-identifiers", &x.review_identifiers)?;
    w.v("review-url", &x.review_url);
    w.e("review-type", &x.review_type);
    w.o("review-completion-date", w_date(&x.review_completion_date));
    w.s("review-group-id", &x.review_group_id);
    if let Some(s) = &x.subject_external_identifier {
        let v = w_ext_id(&w.at("subject-external-identifier"), s)?;
        w.o("subject-external-identifier", Some(v));
    }
    w.s("subject-container-name", &x.subject_container_name);
    w.s("subject-type", &x.subject_type);
    w.o("subject-name", w_title(&x.subject_name));
    w.v("subject-url", &x.subject_url);
    w.o(
        "convening-organization",
        x.convening_organization.as_ref().map(w_org),
    );
    Ok(w.done())
}

fn w_peer_reviews(path: &str, x: &PeerReviews) -> Result<Value> {
    let mut w = w_container(path, &x.path, &x.last_modified_date)?;
    let mut groups = Vec::new();
    for (i, g) in x.groups.iter().enumerate() {
        let gp = format!("{path}/group[{}]", i + 1);
        let mut gw = w_group(&gp, &g.last_modified_date, &g.external_ids)?;
        let mut dups = Vec::new();
        for (j, d) in g.duplicates.iter().enumerate() {
            let dp = format!("{gp}/peer-review-group[{}]", j + 1);
            let mut dw = w_group(&dp, &d.last_modified_date, &d.external_ids)?;
            let items = d
                .summaries
                .iter()
                .map(|s| w_peer_review_summary(&format!("{dp}/peer-review-summary"), s))
                .collect::<Result<Vec<_>>>()?;
            dw.a("peer-review-summary", items);
            dups.push(dw.done());
        }
        gw.a("peer-review-group", dups);
        groups.push(gw.done());
    }
    w.a("group", groups);
    Ok(w.done())
}

fn w_hosts(orgs: &[Organization]) -> Value {
    let mut w = W::new("");
    w.a("organization", orgs.iter().map(w_org).collect());
    w.done()
}

fn w_proposal(path: &str, p: &Proposal) -> Result<Value> {
    let mut w = W::new(path);
    w.o("title", w_title(&p.title));
    w.o("hosts", Some(w_hosts(&p.hosts)));
    req_ext_ids(&mut w, "external-ids", &p.external_ids)?;
    w.o("start-date", w_date(&p.start_date));
    w.o("end-date", w_date(&p.end_date));
    w.v("url", &p.url);
    Ok(w.done())
}

fn w_research_resource(path: &str, x: &ResearchResource) -> Result<Value> {
    let mut w = W::new(path);
    w_meta(&mut w, &x.meta)?;
    if let Some(p) = &x.proposal {
        let v = w_proposal(&w.at("proposal"), p)?;
        w.o("proposal", Some(v));
    }
    let mut items = Vec::new();
    for (i, it) in x.resource_items.iter().enumerate() {
        let ip = format!("{path}/resource-item[{}]", i + 1);
        let mut iw = W::new(&ip);
        iw.s("resource-name", &it.resource_name);
        iw.e("resource-type", &it.resource_type);
        iw.o("hosts", Some(w_hosts(&it.hosts)));
        req_ext_ids(&mut iw, "external-ids", &it.external_ids)?;
        iw.v("url", &it.url);
        items.push(iw.done());
    }
    w.a("resource-item", items);
    Ok(w.done())
}

fn w_research_resource_summary(path: &str, x: &ResearchResourceSummary) -> Result<Value> {
    let mut w = W::new(path);
    w_meta(&mut w, &x.meta)?;
    if let Some(p) = &x.proposal {
        let v = w_proposal(&w.at("proposal"), p)?;
        w.o("proposal", Some(v));
    }
    Ok(w.done())
}

fn w_research_resources(path: &str, x: &ResearchResources) -> Result<Value> {
    let mut w = w_container(path, &x.path, &x.last_modified_date)?;
    let mut groups = Vec::new();
    for (i, g) in x.groups.iter().enumerate() {
        let gp = format!("{path}/group[{}]", i + 1);
        let mut gw = w_group(&gp, &g.last_modified_date, &g.external_ids)?;
        let items = g
            .summaries
            .iter()
            .map(|s| w_research_resource_summary(&format!("{gp}/research-resource-summary"), s))
            .collect::<Result<Vec<_>>>()?;
        gw.a("research-resource-summary", items);
        groups.push(gw.done());
    }
    w.a("group", groups);
    Ok(w.done())
}

fn w_activities(path: &str, x: &ActivitiesSummary) -> Result<Value> {
    let mut w = w_container(path, &x.path, &x.last_modified_date)?;
    for (k, a) in [
        (AffiliationKind::Distinction, &x.distinctions),
        (AffiliationKind::Education, &x.educations),
        (AffiliationKind::Employment, &x.employments),
        (AffiliationKind::InvitedPosition, &x.invited_positions),
        (AffiliationKind::Membership, &x.memberships),
        (AffiliationKind::Qualification, &x.qualifications),
        (AffiliationKind::Service, &x.services),
    ] {
        if let Some(a) = a {
            let v = w_affiliations(&w.at(k.container_element()), a)?;
            w.o(k.container_element(), Some(v));
        }
    }
    if let Some(f) = &x.fundings {
        let v = w_fundings(&w.at("fundings"), f)?;
        w.o("fundings", Some(v));
    }
    if let Some(p) = &x.peer_reviews {
        let v = w_peer_reviews(&w.at("peer-reviews"), p)?;
        w.o("peer-reviews", Some(v));
    }
    if let Some(r) = &x.research_resources {
        let v = w_research_resources(&w.at("research-resources"), r)?;
        w.o("research-resources", Some(v));
    }
    if let Some(r) = &x.works {
        let v = w_works(&w.at("works"), r)?;
        w.o("works", Some(v));
    }
    Ok(w.done())
}

fn w_name(path: &str, x: &Name) -> Result<Value> {
    let mut w = W::new(path);
    w.e("visibility", &x.visibility);
    w.s("path", &x.path);
    w.t("created-date", &x.created_date)?;
    w.t("last-modified-date", &x.last_modified_date)?;
    w.v("given-names", &x.given_names);
    w.v("family-name", &x.family_name);
    w.v("credit-name", &x.credit_name);
    Ok(w.done())
}

fn w_biography(path: &str, x: &Biography) -> Result<Value> {
    let mut w = W::new(path);
    w.e("visibility", &x.visibility);
    w.s("path", &x.path);
    w.t("created-date", &x.created_date)?;
    w.t("last-modified-date", &x.last_modified_date)?;
    w.s("content", &x.content);
    Ok(w.done())
}

fn w_other_name(path: &str, x: &OtherName) -> Result<Value> {
    let mut w = W::new(path);
    w_meta(&mut w, &x.meta)?;
    w.s("content", &x.content);
    Ok(w.done())
}

fn w_researcher_url(path: &str, x: &ResearcherUrl) -> Result<Value> {
    let mut w = W::new(path);
    w_meta(&mut w, &x.meta)?;
    w.s("url-name", &x.url_name);
    w.v("url", &x.url);
    Ok(w.done())
}

fn w_email(path: &str, x: &Email) -> Result<Value> {
    let mut w = W::new(path);
    w_meta(&mut w, &x.meta)?;
    w.s("email", &x.email);
    w.b("verified", x.verified);
    w.b("primary", x.primary);
    Ok(w.done())
}

fn w_address(path: &str, x: &Address) -> Result<Value> {
    let mut w = W::new(path);
    w_meta(&mut w, &x.meta)?;
    w.v("country", &x.country);
    Ok(w.done())
}

fn w_keyword(path: &str, x: &Keyword) -> Result<Value> {
    let mut w = W::new(path);
    w_meta(&mut w, &x.meta)?;
    w.s("content", &x.content);
    Ok(w.done())
}

fn w_list<T>(
    path: &str,
    l: &ItemList<T>,
    item: &str,
    f: fn(&str, &T) -> Result<Value>,
) -> Result<Value> {
    let mut w = w_container(path, &l.path, &l.last_modified_date)?;
    let items = l
        .items
        .iter()
        .enumerate()
        .map(|(i, x)| f(&format!("{path}/{item}[{}]", i + 1), x))
        .collect::<Result<Vec<_>>>()?;
    w.a(item, items);
    Ok(w.done())
}

fn w_person(path: &str, x: &Person) -> Result<Value> {
    let mut w = w_container(path, &x.path, &x.last_modified_date)?;
    if let Some(n) = &x.name {
        let v = w_name(&w.at("name"), n)?;
        w.o("name", Some(v));
    }
    if let Some(l) = &x.other_names {
        let v = w_list(&w.at("other-names"), l, "other-name", w_other_name)?;
        w.o("other-names", Some(v));
    }
    if let Some(b) = &x.biography {
        let v = w_biography(&w.at("biography"), b)?;
        w.o("biography", Some(v));
    }
    if let Some(l) = &x.researcher_urls {
        let v = w_list(
            &w.at("researcher-urls"),
            l,
            "researcher-url",
            w_researcher_url,
        )?;
        w.o("researcher-urls", Some(v));
    }
    if let Some(l) = &x.emails {
        let v = w_list(&w.at("emails"), l, "email", w_email)?;
        w.o("emails", Some(v));
    }
    if let Some(l) = &x.addresses {
        let v = w_list(&w.at("addresses"), l, "address", w_address)?;
        w.o("addresses", Some(v));
    }
    if let Some(l) = &x.keywords {
        let v = w_list(&w.at("keywords"), l, "keyword", w_keyword)?;
        w.o("keywords", Some(v));
    }
    if let Some(l) = &x.external_identifiers {
        let v = w_list(
            &w.at("external-identifiers"),
            l,
            "external-identifier",
            w_ext_id,
        )?;
        w.o("external-identifiers", Some(v));
    }
    Ok(w.done())
}

fn w_personal_details(path: &str, x: &PersonalDetails) -> Result<Value> {
    let mut w = w_container(path, &x.path, &x.last_modified_date)?;
    if let Some(n) = &x.name {
        let v = w_name(&w.at("name"), n)?;
        w.o("name", Some(v));
    }
    if let Some(l) = &x.other_names {
        let v = w_list(&w.at("other-names"), l, "other-name", w_other_name)?;
        w.o("other-names", Some(v));
    }
    if let Some(b) = &x.biography {
        let v = w_biography(&w.at("biography"), b)?;
        w.o("biography", Some(v));
    }
    Ok(w.done())
}

fn w_history(path: &str, x: &History) -> Result<Value> {
    let mut w = W::new(path);
    // no "visibility": history/@visibility is XML-only (module docs)
    w.s(
        "creation-method",
        &x.creation_method.as_ref().map(creation_method_to_json),
    );
    w.t("completion-date", &x.completion_date)?;
    w.t("submission-date", &x.submission_date)?;
    w.t("last-modified-date", &x.last_modified_date)?;
    w.b("claimed", x.claimed);
    if let Some(s) = &x.source {
        let v = w_source(&w.at("source"), s);
        w.o("source", Some(v));
    }
    w.t("deactivation-date", &x.deactivation_date)?;
    w.b("verified-email", x.verified_email);
    w.b("verified-primary-email", x.verified_primary_email);
    Ok(w.done())
}

fn w_record(path: &str, x: &Record) -> Result<Value> {
    let mut w = W::new(path);
    w.s("path", &x.path);
    w.o("orcid-identifier", x.orcid_identifier.as_ref().map(w_ident));
    if let Some(p) = &x.preferences {
        let mut pw = W::new("");
        pw.s("locale", &p.locale);
        w.o("preferences", Some(pw.done()));
    }
    if let Some(h) = &x.history {
        let v = w_history(&w.at("history"), h)?;
        w.o("history", Some(v));
    }
    if let Some(p) = &x.person {
        let v = w_person(&w.at("person"), p)?;
        w.o("person", Some(v));
    }
    if let Some(a) = &x.activities_summary {
        let v = w_activities(&w.at("activities-summary"), a)?;
        w.o("activities-summary", Some(v));
    }
    Ok(w.done())
}

fn w_error(x: &OrcidError) -> Value {
    let mut w = W::new("");
    w.n("response-code", x.response_code);
    w.s("developer-message", &x.developer_message);
    w.s("user-message", &x.user_message);
    w.n("error-code", x.error_code);
    w.s("more-info", &x.more_info);
    w.done()
}

fn w_bulk(path: &str, items: &[BulkItem]) -> Result<Value> {
    let mut out = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let mut e = W::new("");
        match it {
            BulkItem::Work(x) => {
                let v = w_work(&format!("{path}/work[{}]", i + 1), x)?;
                e.o("work", Some(v));
            }
            BulkItem::Error(x) => e.o("error", Some(w_error(x))),
        }
        out.push(e.done());
    }
    let mut w = W::new(path);
    w.a("bulk", out);
    Ok(w.done())
}

/// `group-id-record` as ORCID's JSON has it: no dates (`@JsonIgnore` in
/// ORCID's `GroupIdRecord`), `source` as an object.
fn w_group_id_record(path: &str, x: &GroupIdRecord) -> Value {
    let mut w = W::new(path);
    w.n("put-code", x.put_code);
    w.s("name", &x.name);
    w.s("group-id", &x.group_id);
    w.s("description", &x.description);
    w.e("type", &x.group_type);
    if let Some(s) = &x.source {
        let v = w_source(&w.at("source"), s);
        w.o("source", Some(v));
    }
    w.done()
}

fn w_group_id_records(path: &str, x: &GroupIdRecords) -> Result<Value> {
    let mut w = W::new(path);
    w.t("last-modified-date", &x.last_modified_date)?;
    w.n("total", x.total);
    w.n("page", x.page);
    w.n("page-size", x.page_size);
    let records = x
        .records
        .iter()
        .enumerate()
        .map(|(i, r)| w_group_id_record(&format!("{path}/group-id-record[{}]", i + 1), r))
        .collect();
    w.a("group-id-record", records);
    Ok(w.done())
}

/// The model as an ORCID JSON value.
///
/// ```
/// use yorc::model::{Preferences, Root};
///
/// let v = yorc::json::to_value(&Root::Preferences(Preferences { locale: Some("en".into()) }))?;
/// assert_eq!(v, serde_json::json!({"locale": "en"}));
/// # Ok::<(), yorc::Error>(())
/// ```
pub fn to_value(root: &Root) -> Result<Value> {
    let kind = kind_of(root).unwrap_or("unknown");
    let p = kind;
    Ok(match root {
        Root::Record(x) => w_record(p, x)?,
        Root::ActivitiesSummary(x) => w_activities(p, x)?,
        Root::Works(x) => w_works(p, x)?,
        Root::Work(x) => w_work(p, x)?,
        Root::WorkSummary(x) => w_work_summary(p, x)?,
        Root::Fundings(x) => w_fundings(p, x)?,
        Root::Funding(x) => w_funding(p, x)?,
        Root::FundingSummary(x) => w_funding_summary(p, x)?,
        Root::Affiliations(x) => w_affiliations(p, x)?,
        Root::Affiliation(x) | Root::AffiliationSummary(x) => w_affiliation(p, x)?,
        Root::PeerReviews(x) => w_peer_reviews(p, x)?,
        Root::PeerReview(x) => w_peer_review(p, x)?,
        Root::PeerReviewSummary(x) => w_peer_review_summary(p, x)?,
        Root::ResearchResources(x) => w_research_resources(p, x)?,
        Root::ResearchResource(x) => w_research_resource(p, x)?,
        Root::ResearchResourceSummary(x) => w_research_resource_summary(p, x)?,
        Root::Person(x) => w_person(p, x)?,
        Root::PersonalDetails(x) => w_personal_details(p, x)?,
        Root::Name(x) => w_name(p, x)?,
        Root::Biography(x) => w_biography(p, x)?,
        Root::CreditName(x) => {
            let mut w = W::new(p);
            w.s("value", x);
            w.done()
        }
        Root::OtherNames(l) => w_list(p, l, "other-name", w_other_name)?,
        Root::OtherName(x) => w_other_name(p, x)?,
        Root::ResearcherUrls(l) => w_list(p, l, "researcher-url", w_researcher_url)?,
        Root::ResearcherUrl(x) => w_researcher_url(p, x)?,
        Root::Emails(l) => w_list(p, l, "email", w_email)?,
        Root::Email(x) => w_email(p, x)?,
        Root::Addresses(l) => w_list(p, l, "address", w_address)?,
        Root::Address(x) => w_address(p, x)?,
        Root::Keywords(l) => w_list(p, l, "keyword", w_keyword)?,
        Root::Keyword(x) => w_keyword(p, x)?,
        Root::ExternalIdentifiers(l) => w_list(p, l, "external-identifier", w_ext_id)?,
        Root::ExternalIdentifier(x) => w_ext_id(p, x)?,
        Root::History(x) => w_history(p, x)?,
        Root::Preferences(x) => {
            let mut w = W::new(p);
            w.s("locale", &x.locale);
            w.done()
        }
        Root::Bulk(b) => w_bulk(p, &b.items)?,
        Root::Error(e) => w_error(e),
        Root::Search(s) => {
            let mut w = W::new(p);
            w.n("num-found", s.num_found);
            let results = s
                .results
                .iter()
                .map(|r| {
                    let mut rw = W::new("");
                    rw.o("orcid-identifier", Some(w_ident(r)));
                    rw.done()
                })
                .collect();
            w.a("result", results);
            w.done()
        }
        Root::ExpandedSearch(s) => {
            let mut w = W::new(p);
            w.n("num-found", s.num_found);
            let results = s
                .results
                .iter()
                .map(|r| {
                    let mut rw = W::new("");
                    rw.s("orcid-id", &r.orcid_id);
                    rw.s("given-names", &r.given_names);
                    rw.s("family-names", &r.family_names);
                    rw.s("credit-name", &r.credit_name);
                    let strs = |v: &[String]| {
                        v.iter()
                            .map(|s| Value::String(s.clone()))
                            .collect::<Vec<_>>()
                    };
                    if !r.other_names.is_empty() {
                        rw.a("other-name", strs(&r.other_names));
                    }
                    if !r.emails.is_empty() {
                        rw.a("email", strs(&r.emails));
                    }
                    if !r.institution_names.is_empty() {
                        rw.a("institution-name", strs(&r.institution_names));
                    }
                    rw.done()
                })
                .collect();
            w.a("expanded-result", results);
            w.done()
        }
        Root::GroupIdRecord(x) => w_group_id_record(p, x),
        Root::GroupIdRecords(x) => w_group_id_records(p, x)?,
        Root::Unknown(name) => {
            return Err(Error::Unwritable {
                path: name.clone(),
                reason: "unknown root element".to_string(),
            });
        }
    })
}

/// The document kind of a root (the XML root local name), `None` for [`Root::Unknown`].
///
/// ```
/// use yorc::model::{Affiliation, AffiliationKind, Root};
///
/// let a = Affiliation::new(AffiliationKind::InvitedPosition);
/// assert_eq!(yorc::json::kind_of(&Root::Affiliation(a.clone())), Some("invited-position"));
/// assert_eq!(yorc::json::kind_of(&Root::AffiliationSummary(a)), Some("invited-position-summary"));
/// ```
pub fn kind_of(root: &Root) -> Option<&'static str> {
    Some(match root {
        Root::Record(_) => "record",
        Root::ActivitiesSummary(_) => "activities-summary",
        Root::Works(_) => "works",
        Root::Work(_) => "work",
        Root::WorkSummary(_) => "work-summary",
        Root::Fundings(_) => "fundings",
        Root::Funding(_) => "funding",
        Root::FundingSummary(_) => "funding-summary",
        Root::Affiliations(a) => a.kind.container_element(),
        Root::Affiliation(a) => a.kind.as_str(),
        Root::AffiliationSummary(a) => a.kind.summary_element(),
        Root::PeerReviews(_) => "peer-reviews",
        Root::PeerReview(_) => "peer-review",
        Root::PeerReviewSummary(_) => "peer-review-summary",
        Root::ResearchResources(_) => "research-resources",
        Root::ResearchResource(_) => "research-resource",
        Root::ResearchResourceSummary(_) => "research-resource-summary",
        Root::Person(_) => "person",
        Root::PersonalDetails(_) => "personal-details",
        Root::Name(_) => "name",
        Root::Biography(_) => "biography",
        Root::CreditName(_) => "credit-name",
        Root::OtherNames(_) => "other-names",
        Root::OtherName(_) => "other-name",
        Root::ResearcherUrls(_) => "researcher-urls",
        Root::ResearcherUrl(_) => "researcher-url",
        Root::Emails(_) => "emails",
        Root::Email(_) => "email",
        Root::Addresses(_) => "addresses",
        Root::Address(_) => "address",
        Root::Keywords(_) => "keywords",
        Root::Keyword(_) => "keyword",
        Root::ExternalIdentifiers(_) => "external-identifiers",
        Root::ExternalIdentifier(_) => "external-identifier",
        Root::History(_) => "history",
        Root::Preferences(_) => "preferences",
        Root::Bulk(_) => "bulk",
        Root::Error(_) => "error",
        Root::Search(_) => "search",
        Root::ExpandedSearch(_) => "expanded-search",
        Root::GroupIdRecord(_) => "group-id-record",
        Root::GroupIdRecords(_) => "group-id-records",
        Root::Unknown(_) => return None,
    })
}

/// ORCID JSON text: pretty-printed (two-space indent), keys sorted, final LF.
///
/// ```
/// use yorc::model::{Keyword, Root};
///
/// let k = Keyword { content: Some("vents".into()), ..Keyword::default() };
/// let json = yorc::json::to_string(&Root::Keyword(k.clone()))?;
/// assert_eq!(json, "{\n  \"content\": \"vents\"\n}\n");
/// assert_eq!(yorc::json::from_str("keyword", &json)?.root, Root::Keyword(k));
/// # Ok::<(), yorc::Error>(())
/// ```
pub fn to_string(root: &Root) -> Result<String> {
    let v = to_value(root)?;
    let mut s = serde_json::to_string_pretty(&v).map_err(|e| Error::Json {
        message: e.to_string(),
    })?;
    s.push('\n');
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps() {
        assert_eq!(millis_to_datetime(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            millis_to_datetime(1_540_896_558_250),
            "2018-10-30T10:49:18.250Z"
        );
        assert_eq!(
            datetime_to_millis("2018-10-30T10:49:18.250Z"),
            Some(1_540_896_558_250)
        );
        assert_eq!(
            datetime_to_millis("2016-04-27T13:22:53.471-06:00"),
            Some(1_461_784_973_471)
        );
        assert_eq!(
            datetime_to_millis("2001-12-31T12:00:00"),
            Some(1_009_800_000_000)
        );
        assert_eq!(
            datetime_to_millis("2001-12-31T12:00:00.123456+01:00"),
            Some(1_009_796_400_123)
        );
        assert_eq!(
            datetime_to_millis("1960-01-01T00:00:00Z"),
            Some(-315_619_200_000)
        );
        // a non-ASCII zone is not a timestamp (and not a panic)
        for zone in ["+a\u{e9}00", "+\u{e9}:00", "+00:\u{e9}", "+0\u{20ac}0"] {
            assert_eq!(
                datetime_to_millis(&format!("2001-12-31T12:00:00{zone}")),
                None
            );
        }
        assert_eq!(
            millis_to_datetime(-315_619_200_000),
            "1960-01-01T00:00:00.000Z"
        );
        assert_eq!(datetime_to_millis("yesterday"), None);
        assert_eq!(datetime_to_millis("2001-13-01T00:00:00Z"), None);
        for ms in [
            0i64,
            1,
            999,
            86_399_999,
            951_782_400_000,
            4_102_444_800_000,
            -1,
        ] {
            assert_eq!(datetime_to_millis(&millis_to_datetime(ms)), Some(ms));
        }
    }

    #[test]
    fn lenient_text_and_strict_output() {
        let doc = from_str(
            "work",
            r#"{"title": {"title": {"value": " T "}, "subtitle": "S", "translated-title": null},
                "type": "book", "url": "https://bare", "put-code": "12", "display-index": -1,
                "publication-date": {"year": {"value": "2022"}, "month": {"value": "05"}},
                "unknown-key": [1, 2]}"#,
        )
        .unwrap();
        let Root::Work(w) = &doc.root else { panic!() };
        let t = w.title.as_ref().unwrap();
        assert_eq!(t.title.as_deref(), Some("T"));
        assert_eq!(t.subtitle.as_deref(), Some("S"));
        assert_eq!(w.url.as_deref(), Some("https://bare"));
        assert_eq!(w.meta.put_code, Some(12));
        assert_eq!(w.meta.display_index, None);
        assert_eq!(w.meta.display_index_text.as_deref(), Some("-1"));
        let v = to_value(&doc.root).unwrap();
        assert_eq!(v["url"], serde_json::json!({"value": "https://bare"}));
        assert_eq!(v["title"]["subtitle"], serde_json::json!({"value": "S"}));
        assert_eq!(v["put-code"], serde_json::json!(12));
        assert_eq!(v["display-index"], serde_json::json!("-1"));
        assert_eq!(
            v["publication-date"]["month"],
            serde_json::json!({"value": "05"})
        );
        assert!(v.get("translated-title").is_none());
    }

    #[test]
    fn history_visibility_is_xml_only() {
        // ORCID's JSON has no history "visibility" (its Java model refuses
        // the member): the writer leaves it out, the reader still takes it.
        let h = History {
            visibility: Some(Visibility::Private),
            creation_method: Some(CreationMethod::MemberReferred),
            claimed: Some(true),
            ..History::default()
        };
        let v = to_value(&Root::History(h)).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"creation-method": "MEMBER_REFERRED", "claimed": true})
        );
        let doc = from_str(
            "history",
            r#"{"visibility": "private", "creation-method": "DIRECT"}"#,
        )
        .unwrap();
        let Root::History(h) = &doc.root else {
            panic!()
        };
        assert_eq!(h.visibility, Some(Visibility::Private));
        assert_eq!(h.creation_method, Some(CreationMethod::Direct));
    }

    #[test]
    fn errors() {
        assert!(matches!(from_str("work", "{"), Err(Error::Json { .. })));
        assert!(matches!(from_str("work", "[]"), Err(Error::Json { .. })));
        assert!(matches!(
            from_str("nonsense", "{}"),
            Err(Error::Json { .. })
        ));
        let mut k = Keyword::default();
        k.meta.created_date = Some("not a date".into());
        assert!(matches!(
            to_value(&Root::Keyword(k)),
            Err(Error::Unwritable { ref path, .. }) if path == "keyword/created-date"
        ));
    }
}
