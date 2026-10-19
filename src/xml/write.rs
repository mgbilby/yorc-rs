//! Model → ORCID v3.0 XML.
//!
//! Output is exact and deterministic:
//! * `<?xml version="1.0" encoding="UTF-8"?>`, then the root element, LF line
//!   ends, two-space indentation, a final LF;
//! * element names carry the prefixes ORCID uses (`work:`, `common:`, ...),
//!   each prefix bound to `http://www.orcid.org/ns/<prefix>`; every namespace
//!   used anywhere in the document is declared once, on the root, in the order
//!   of [`Ns`];
//! * elements follow the XSD sequence order; absent optional values are not
//!   written; containers the XSD requires (`external-ids` in groups,
//!   `review-identifiers`, `resource-items`, ...) are always written;
//! * attributes in a fixed order per element (`put-code`, `path`,
//!   `visibility`, `display-index`, then element-specific ones);
//! * text escapes `&`, `<`, `>` and CR (attributes also `"`, TAB, LF);
//! * fuzzy-date parts: year as `{:04}`, month and day as `{:02}`.
//!
//! The document is written in one pass, straight from the model into one
//! pre-sized `String` (see the emitter below).
//!
//! [`to_string`] writes any [`Root`] as it is (no validation; this is what the
//! round-trip tests use).

use crate::error::{Error, Result};
use crate::model::*;

mod group_id;

/// The ORCID v3.0 namespaces, in declaration order.
///
/// ```
/// use yorc::xml::Ns;
///
/// assert_eq!(Ns::PeerReview.prefix(), "peer-review");
/// assert_eq!(Ns::PeerReview.uri(), "http://www.orcid.org/ns/peer-review");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ns {
    /// `activities`: `http://www.orcid.org/ns/activities`
    Activities,
    /// `address`: `http://www.orcid.org/ns/address`
    Address,
    /// `bulk`: `http://www.orcid.org/ns/bulk`
    Bulk,
    /// `common`: `http://www.orcid.org/ns/common`
    Common,
    /// `distinction`: `http://www.orcid.org/ns/distinction`
    Distinction,
    /// `education`: `http://www.orcid.org/ns/education`
    Education,
    /// `email`: `http://www.orcid.org/ns/email`
    Email,
    /// `employment`: `http://www.orcid.org/ns/employment`
    Employment,
    /// `error`: `http://www.orcid.org/ns/error`
    Error,
    /// `expanded-search`: `http://www.orcid.org/ns/expanded-search`
    ExpandedSearch,
    /// `external-identifier`: `http://www.orcid.org/ns/external-identifier`
    ExternalIdentifier,
    /// `funding`: `http://www.orcid.org/ns/funding`
    Funding,
    /// `history`: `http://www.orcid.org/ns/history`
    History,
    /// `invited-position`: `http://www.orcid.org/ns/invited-position`
    InvitedPosition,
    /// `keyword`: `http://www.orcid.org/ns/keyword`
    Keyword,
    /// `membership`: `http://www.orcid.org/ns/membership`
    Membership,
    /// `other-name`: `http://www.orcid.org/ns/other-name`
    OtherName,
    /// `peer-review`: `http://www.orcid.org/ns/peer-review`
    PeerReview,
    /// `person`: `http://www.orcid.org/ns/person`
    Person,
    /// `personal-details`: `http://www.orcid.org/ns/personal-details`
    PersonalDetails,
    /// `preferences`: `http://www.orcid.org/ns/preferences`
    Preferences,
    /// `qualification`: `http://www.orcid.org/ns/qualification`
    Qualification,
    /// `record`: `http://www.orcid.org/ns/record`
    Record,
    /// `research-resource`: `http://www.orcid.org/ns/research-resource`
    ResearchResource,
    /// `researcher-url`: `http://www.orcid.org/ns/researcher-url`
    ResearcherUrl,
    /// `search`: `http://www.orcid.org/ns/search`
    Search,
    /// `service`: `http://www.orcid.org/ns/service`
    Service,
    /// `work`: `http://www.orcid.org/ns/work`
    Work,
}

impl Ns {
    /// Every namespace, in declaration order.
    pub const ALL: [Ns; 28] = [
        Ns::Activities,
        Ns::Address,
        Ns::Bulk,
        Ns::Common,
        Ns::Distinction,
        Ns::Education,
        Ns::Email,
        Ns::Employment,
        Ns::Error,
        Ns::ExpandedSearch,
        Ns::ExternalIdentifier,
        Ns::Funding,
        Ns::History,
        Ns::InvitedPosition,
        Ns::Keyword,
        Ns::Membership,
        Ns::OtherName,
        Ns::PeerReview,
        Ns::Person,
        Ns::PersonalDetails,
        Ns::Preferences,
        Ns::Qualification,
        Ns::Record,
        Ns::ResearchResource,
        Ns::ResearcherUrl,
        Ns::Search,
        Ns::Service,
        Ns::Work,
    ];

    /// The prefix ORCID uses; also the last segment of the namespace URI.
    pub fn prefix(self) -> &'static str {
        match self {
            Ns::Activities => "activities",
            Ns::Address => "address",
            Ns::Bulk => "bulk",
            Ns::Common => "common",
            Ns::Distinction => "distinction",
            Ns::Education => "education",
            Ns::Email => "email",
            Ns::Employment => "employment",
            Ns::Error => "error",
            Ns::ExpandedSearch => "expanded-search",
            Ns::ExternalIdentifier => "external-identifier",
            Ns::Funding => "funding",
            Ns::History => "history",
            Ns::InvitedPosition => "invited-position",
            Ns::Keyword => "keyword",
            Ns::Membership => "membership",
            Ns::OtherName => "other-name",
            Ns::PeerReview => "peer-review",
            Ns::Person => "person",
            Ns::PersonalDetails => "personal-details",
            Ns::Preferences => "preferences",
            Ns::Qualification => "qualification",
            Ns::Record => "record",
            Ns::ResearchResource => "research-resource",
            Ns::ResearcherUrl => "researcher-url",
            Ns::Search => "search",
            Ns::Service => "service",
            Ns::Work => "work",
        }
    }

    /// `http://www.orcid.org/ns/<prefix>`.
    pub fn uri(self) -> String {
        format!("http://www.orcid.org/ns/{}", self.prefix())
    }

    /// The namespace of an affiliation kind's items.
    pub fn of_affiliation(kind: AffiliationKind) -> Ns {
        match kind {
            AffiliationKind::Distinction => Ns::Distinction,
            AffiliationKind::Education => Ns::Education,
            AffiliationKind::Employment => Ns::Employment,
            AffiliationKind::InvitedPosition => Ns::InvitedPosition,
            AffiliationKind::Membership => Ns::Membership,
            AffiliationKind::Qualification => Ns::Qualification,
            AffiliationKind::Service => Ns::Service,
        }
    }

    /// This namespace's bit in [`Xml::used`].
    fn bit(self) -> u32 {
        1 << (self as u32)
    }
}

// ---------------------------------------------------------------- emitter
//
// The document is written in one pass straight from the model into a single
// pre-sized `String`: no intermediate tree, no cloned model strings, no
// per-element `format!`. Start tags are left open (`pending`) until the first
// child or the end, so an element without children or text is `<x/>`. The
// namespaces in use are a bit set; their declarations are inserted after the
// root's name once the document is complete (one memmove of the output).
// The element path for `Error::Unwritable` is built only on error, from the
// stack of open elements.

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";
const URI_BASE: &str = "http://www.orcid.org/ns/";
/// Indentation for 32 levels (ORCID documents nest about 10 deep); deeper
/// levels repeat it.
const INDENT: &str = "                                                                ";
/// Room for the root's namespace declarations (all 28 take 1,490 bytes).
const DECL_ROOM: usize = 1536;

struct Xml {
    out: String,
    /// Open elements, outermost first.
    open: Vec<(Ns, &'static str)>,
    /// The innermost start tag is open (`>` not written yet).
    pending: bool,
    /// Namespaces used so far, one bit per [`Ns`].
    used: u32,
    /// Byte offset just after the root element's name.
    root_end: usize,
}

impl Xml {
    fn new(estimate: usize) -> Xml {
        let mut out = String::with_capacity(estimate + DECL_ROOM);
        out.push_str(XML_DECL);
        Xml {
            root_end: out.len(),
            out,
            open: Vec::with_capacity(16),
            pending: false,
            used: 0,
        }
    }

    /// The document, with every namespace in use declared on the root in
    /// [`Ns`] order.
    fn finish(mut self) -> String {
        let mut decls = String::with_capacity(DECL_ROOM);
        for ns in Ns::ALL {
            if self.used & ns.bit() != 0 {
                decls.push_str(" xmlns:");
                decls.push_str(ns.prefix());
                decls.push_str("=\"");
                decls.push_str(URI_BASE);
                decls.push_str(ns.prefix());
                decls.push('"');
            }
        }
        self.out.insert_str(self.root_end, &decls);
        self.out
    }

    // ------------------------------------------------------------ errors

    /// The open elements' path, plus `last` if given (`a/b/last`).
    fn path(&self, last: Option<&str>) -> String {
        let mut p = String::new();
        for (i, (_, name)) in self.open.iter().enumerate() {
            if i > 0 {
                p.push('/');
            }
            p.push_str(name);
        }
        if let Some(l) = last {
            if !p.is_empty() {
                p.push('/');
            }
            p.push_str(l);
        }
        p
    }

    /// Content the XSD has no place for at `path/name`.
    fn forbidden(&self, name: &str, reason: &str) -> Error {
        Error::Unwritable {
            path: self.path(Some(name)),
            reason: reason.to_string(),
        }
    }

    // ----------------------------------------------------------- markup

    fn indent(&mut self) {
        let mut n = 2 * self.open.len();
        while n > INDENT.len() {
            self.out.push_str(INDENT);
            n -= INDENT.len();
        }
        self.out.push_str(&INDENT[..n]);
    }

    fn qname(&mut self, ns: Ns, name: &str) {
        self.out.push_str(ns.prefix());
        self.out.push(':');
        self.out.push_str(name);
    }

    /// Closes the parent's start tag before a child.
    fn close_pending(&mut self) {
        if self.pending {
            self.out.push_str(">\n");
            self.pending = false;
        }
    }

    /// `<ns:name` (attributes may follow); end it with [`Xml::end`] or
    /// [`Xml::end_text`].
    fn start(&mut self, ns: Ns, name: &'static str) {
        self.used |= ns.bit();
        self.close_pending();
        self.indent();
        self.out.push('<');
        self.qname(ns, name);
        if self.open.is_empty() {
            self.root_end = self.out.len();
        }
        self.open.push((ns, name));
        self.pending = true;
    }

    /// ` name="value"` (escaped) if the value is present.
    fn attr(&mut self, name: &'static str, value: Option<&str>) -> Result<()> {
        let Some(v) = value else {
            return Ok(());
        };
        self.out.push(' ');
        self.out.push_str(name);
        self.out.push_str("=\"");
        if let Err(c) = escape_attr(v, &mut self.out) {
            let mut path = self.path(None);
            path.push_str("/@");
            path.push_str(name);
            return Err(bad_char(path, c));
        }
        self.out.push('"');
        Ok(())
    }

    fn attr_int(&mut self, name: &'static str, value: Option<i64>) {
        if let Some(v) = value {
            self.out.push(' ');
            self.out.push_str(name);
            self.out.push_str("=\"");
            push_int(&mut self.out, v, 0);
            self.out.push('"');
        }
    }

    fn attr_bool(&mut self, name: &'static str, value: Option<bool>) {
        if let Some(v) = value {
            self.out.push(' ');
            self.out.push_str(name);
            self.out
                .push_str(if v { "=\"true\"" } else { "=\"false\"" });
        }
    }

    /// Ends the innermost element: `/>` if it has no children.
    fn end(&mut self) {
        let Some((ns, name)) = self.open.pop() else {
            return;
        };
        if self.pending {
            self.pending = false;
            self.out.push_str("/>\n");
            return;
        }
        self.indent();
        self.out.push_str("</");
        self.qname(ns, name);
        self.out.push_str(">\n");
    }

    /// Ends the innermost element (which has no children) with `text`, or
    /// as `/>` if there is none.
    fn end_text(&mut self, text: Option<&str>) -> Result<()> {
        match text {
            Some(t) if self.pending => {
                self.pending = false;
                self.out.push('>');
                if let Err(c) = escape_text(t, &mut self.out) {
                    return Err(bad_char(self.path(None), c));
                }
                if let Some((ns, name)) = self.open.pop() {
                    self.out.push_str("</");
                    self.qname(ns, name);
                    self.out.push_str(">\n");
                }
            }
            _ => self.end(),
        }
        Ok(())
    }

    /// `<ns:name>`, on its own line.
    fn open_leaf(&mut self, ns: Ns, name: &str) {
        self.used |= ns.bit();
        self.close_pending();
        self.indent();
        self.out.push('<');
        self.qname(ns, name);
        self.out.push('>');
    }

    /// `</ns:name>` and the line end.
    fn close_leaf(&mut self, ns: Ns, name: &str) {
        self.out.push_str("</");
        self.qname(ns, name);
        self.out.push_str(">\n");
    }

    /// `<ns:name>text</ns:name>` (escaped).
    fn text(&mut self, ns: Ns, name: &'static str, text: &str) -> Result<()> {
        self.open_leaf(ns, name);
        if let Err(c) = escape_text(text, &mut self.out) {
            return Err(bad_char(self.path(Some(name)), c));
        }
        self.close_leaf(ns, name);
        Ok(())
    }

    /// A text leaf if the value is present.
    fn leaf(&mut self, ns: Ns, name: &'static str, value: Option<&str>) -> Result<()> {
        match value {
            Some(v) => self.text(ns, name, v),
            None => Ok(()),
        }
    }

    /// An integer leaf, zero-padded to `width` as `format!("{v:0width$}")`
    /// does (the sign counts towards the width).
    fn leaf_int(&mut self, ns: Ns, name: &'static str, value: Option<i64>, width: usize) {
        if let Some(v) = value {
            self.open_leaf(ns, name);
            push_int(&mut self.out, v, width);
            self.close_leaf(ns, name);
        }
    }

    fn leaf_bool(&mut self, ns: Ns, name: &'static str, value: Option<bool>) {
        if let Some(v) = value {
            self.open_leaf(ns, name);
            self.out.push_str(if v { "true" } else { "false" });
            self.close_leaf(ns, name);
        }
    }
}

fn bad_char(path: String, c: char) -> Error {
    Error::Unwritable {
        path,
        reason: format!("character U+{:04X} is not allowed in XML 1.0", u32::from(c)),
    }
}

/// Appends `v` in decimal, zero-padded after the sign to `width` characters.
fn push_int(out: &mut String, v: i64, width: usize) {
    let mut buf = [0u8; 20];
    let mut n = v.unsigned_abs();
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    let len = buf.len() - i + usize::from(v < 0);
    if v < 0 {
        out.push('-');
    }
    for _ in len..width {
        out.push('0');
    }
    for &d in &buf[i..] {
        out.push(char::from(d));
    }
}

// --------------------------------------------------------------- escaping
//
// One pass both escapes and checks for characters XML 1.0 cannot carry.
// Eight bytes at a time (SWAR, std only), it looks for the next byte that may
// need attention: a control byte (< 0x20), `&`, `<`, `>`, `"` (attributes),
// or 0xEF, the lead byte of U+FFFE and U+FFFF. Runs of plain text between
// such bytes are copied as `&str` slices; the slice ends are always at ASCII
// bytes or at the ends of the string, so they are char boundaries.

const LO: u64 = 0x0101_0101_0101_0101;
const HI: u64 = 0x8080_8080_8080_8080;

/// The high bit of each byte of `x` that is below `n` (`n` <= 0x80). Bits
/// above the lowest one may be spurious (borrows), the lowest is exact.
#[inline]
fn bytes_below(x: u64, n: u8) -> u64 {
    x.wrapping_sub(LO * u64::from(n)) & !x & HI
}

/// The high bit of each byte of `x` equal to `b` (lowest set bit exact).
#[inline]
fn bytes_equal(x: u64, b: u8) -> u64 {
    bytes_below(x ^ (LO * u64::from(b)), 1)
}

#[inline]
fn is_special<const ATTR: bool>(b: u8) -> bool {
    b < 0x20 || matches!(b, b'&' | b'<' | b'>' | 0xEF) || (ATTR && b == b'"')
}

/// Index of the first byte at or after `i` that [`is_special`], or `b.len()`.
#[inline]
fn next_special<const ATTR: bool>(b: &[u8], mut i: usize) -> usize {
    while let Some(chunk) = b[i..].first_chunk::<8>() {
        let x = u64::from_le_bytes(*chunk);
        let mut m = bytes_below(x, 0x20)
            | bytes_equal(x, b'&')
            | bytes_equal(x, b'<')
            | bytes_equal(x, b'>')
            | bytes_equal(x, 0xEF);
        if ATTR {
            m |= bytes_equal(x, b'"');
        }
        if m != 0 {
            // little-endian load: the lowest set bit is the first byte
            return i + (m.trailing_zeros() / 8) as usize;
        }
        i += 8;
    }
    while i < b.len() {
        if is_special::<ATTR>(b[i]) {
            return i;
        }
        i += 1;
    }
    b.len()
}

/// Appends `s` escaped: `&` `<` `>` and CR always; in attributes (`ATTR`)
/// also `"`, TAB and LF. `Err` is the first character XML 1.0 cannot carry
/// (`out` then holds a partial value).
fn escape<const ATTR: bool>(s: &str, out: &mut String) -> std::result::Result<(), char> {
    let b = s.as_bytes();
    let mut run = 0;
    let mut i = 0;
    loop {
        i = next_special::<ATTR>(b, i);
        let Some(&c) = b.get(i) else {
            break;
        };
        let rep = match c {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' if ATTR => "&quot;",
            b'\r' => "&#13;",
            b'\t' if ATTR => "&#9;",
            b'\n' if ATTR => "&#10;",
            b'\t' | b'\n' | b'"' => {
                i += 1;
                continue;
            }
            0xEF => {
                // U+FFFE is EF BF BE, U+FFFF is EF BF BF
                if b.get(i + 1).copied() == Some(0xBF) {
                    match b.get(i + 2).copied() {
                        Some(0xBE) => return Err('\u{FFFE}'),
                        Some(0xBF) => return Err('\u{FFFF}'),
                        _ => {}
                    }
                }
                i += 1;
                continue;
            }
            // the other control characters
            _ => return Err(char::from(c)),
        };
        out.push_str(&s[run..i]);
        out.push_str(rep);
        i += 1;
        run = i;
    }
    out.push_str(&s[run..]);
    Ok(())
}

fn escape_text(s: &str, out: &mut String) -> std::result::Result<(), char> {
    escape::<false>(s, out)
}

fn escape_attr(s: &str, out: &mut String) -> std::result::Result<(), char> {
    escape::<true>(s, out)
}

// ----------------------------------------------------------- size estimate
//
// Output capacity from the shape of the model (counts only, no strings
// read): an under-estimate costs a reallocation and a copy, an over-estimate
// only untouched address space. Calibrated on the real record in
// tests/data (a work summary writes about 1.2 KiB besides its ids, an
// external id about 480 bytes, indentation included), rounded up.

const PER_ITEM: usize = 1600;
const PER_ID: usize = 560;

fn ids_size(ids: &[ExternalId]) -> usize {
    ids.len() * PER_ID
}

fn works_size(w: &Works) -> usize {
    w.groups
        .iter()
        .map(|g| {
            ids_size(&g.external_ids)
                + g.work_summaries
                    .iter()
                    .map(|s| PER_ITEM + ids_size(&s.external_ids))
                    .sum::<usize>()
        })
        .sum()
}

fn fundings_size(f: &Fundings) -> usize {
    f.groups
        .iter()
        .map(|g| {
            ids_size(&g.external_ids)
                + g.funding_summaries
                    .iter()
                    .map(|s| PER_ITEM + ids_size(&s.external_ids))
                    .sum::<usize>()
        })
        .sum()
}

fn affiliations_size(a: &Affiliations) -> usize {
    a.groups
        .iter()
        .map(|g| {
            ids_size(&g.external_ids)
                + g.summaries
                    .iter()
                    .map(|s| PER_ITEM + ids_size(&s.external_ids))
                    .sum::<usize>()
        })
        .sum()
}

fn peer_reviews_size(p: &PeerReviews) -> usize {
    p.groups
        .iter()
        .map(|g| {
            ids_size(&g.external_ids)
                + g.duplicates
                    .iter()
                    .map(|d| {
                        ids_size(&d.external_ids)
                            + d.summaries
                                .iter()
                                .map(|s| PER_ITEM + ids_size(&s.external_ids))
                                .sum::<usize>()
                    })
                    .sum::<usize>()
        })
        .sum()
}

fn research_resources_size(r: &ResearchResources) -> usize {
    r.groups
        .iter()
        .map(|g| ids_size(&g.external_ids) + g.summaries.len() * PER_ITEM)
        .sum()
}

fn activities_size(a: &ActivitiesSummary) -> usize {
    let kinds = [
        &a.distinctions,
        &a.educations,
        &a.employments,
        &a.invited_positions,
        &a.memberships,
        &a.qualifications,
        &a.services,
    ];
    kinds
        .into_iter()
        .flatten()
        .map(affiliations_size)
        .sum::<usize>()
        + a.works.as_ref().map_or(0, works_size)
        + a.fundings.as_ref().map_or(0, fundings_size)
        + a.peer_reviews.as_ref().map_or(0, peer_reviews_size)
        + a.research_resources
            .as_ref()
            .map_or(0, research_resources_size)
}

fn person_size(p: &Person) -> usize {
    let items = p.other_names.as_ref().map_or(0, |l| l.items.len())
        + p.researcher_urls.as_ref().map_or(0, |l| l.items.len())
        + p.emails.as_ref().map_or(0, |l| l.items.len())
        + p.addresses.as_ref().map_or(0, |l| l.items.len())
        + p.keywords.as_ref().map_or(0, |l| l.items.len())
        + p.external_identifiers.as_ref().map_or(0, |l| l.items.len());
    2048 + p
        .biography
        .as_ref()
        .map_or(0, |b| b.content.as_ref().map_or(0, String::len))
        + items * PER_ITEM
}

fn estimate(root: &Root) -> usize {
    4096 + match root {
        Root::Record(r) => {
            r.person.as_ref().map_or(0, person_size)
                + r.activities_summary.as_ref().map_or(0, activities_size)
        }
        Root::ActivitiesSummary(a) => activities_size(a),
        Root::Works(w) => works_size(w),
        Root::Fundings(f) => fundings_size(f),
        Root::Affiliations(a) => affiliations_size(a),
        Root::PeerReviews(p) => peer_reviews_size(p),
        Root::ResearchResources(r) => research_resources_size(r),
        Root::Person(p) => person_size(p),
        Root::Bulk(b) => b.items.len() * 4 * PER_ITEM,
        _ => 0,
    }
}

// ----------------------------------------------------------------- common

fn meta_attrs(x: &mut Xml, m: &ItemMeta) -> Result<()> {
    x.attr_int("put-code", m.put_code);
    x.attr("path", m.path.as_deref())?;
    x.attr("visibility", m.visibility.as_ref().map(|v| v.as_str()))?;
    match &m.display_index_text {
        Some(t) => x.attr("display-index", Some(t.as_str()))?,
        None => x.attr_int("display-index", m.display_index.map(i64::from)),
    }
    Ok(())
}

fn meta_kids(x: &mut Xml, m: &ItemMeta) -> Result<()> {
    x.leaf(Ns::Common, "created-date", m.created_date.as_deref())?;
    x.leaf(
        Ns::Common,
        "last-modified-date",
        m.last_modified_date.as_deref(),
    )?;
    if let Some(s) = &m.source {
        source(x, s)?;
    }
    Ok(())
}

/// Starts an element with the item attributes and `element-summary`
/// children; the caller writes the rest and ends it.
fn item(x: &mut Xml, ns: Ns, name: &'static str, m: &ItemMeta) -> Result<()> {
    x.start(ns, name);
    meta_attrs(x, m)?;
    meta_kids(x, m)
}

fn identifier(x: &mut Xml, ns: Ns, name: &'static str, id: &OrcidIdentifier) -> Result<()> {
    x.start(ns, name);
    x.leaf(Ns::Common, "uri", id.uri.as_deref())?;
    x.leaf(Ns::Common, "path", id.path.as_deref())?;
    x.leaf(Ns::Common, "host", id.host.as_deref())?;
    x.end();
    Ok(())
}

fn opt_identifier(
    x: &mut Xml,
    ns: Ns,
    name: &'static str,
    id: &Option<OrcidIdentifier>,
) -> Result<()> {
    match id {
        Some(id) => identifier(x, ns, name, id),
        None => Ok(()),
    }
}

fn source(x: &mut Xml, s: &Source) -> Result<()> {
    x.start(Ns::Common, "source");
    opt_identifier(x, Ns::Common, "source-orcid", &s.source_orcid)?;
    opt_identifier(x, Ns::Common, "source-client-id", &s.source_client_id)?;
    x.leaf(Ns::Common, "source-name", s.source_name.as_deref())?;
    opt_identifier(
        x,
        Ns::Common,
        "assertion-origin-orcid",
        &s.assertion_origin_orcid,
    )?;
    opt_identifier(
        x,
        Ns::Common,
        "assertion-origin-client-id",
        &s.assertion_origin_client_id,
    )?;
    x.leaf(
        Ns::Common,
        "assertion-origin-name",
        s.assertion_origin_name.as_deref(),
    )?;
    x.end();
    Ok(())
}

fn opt_date(x: &mut Xml, ns: Ns, name: &'static str, d: &Option<FuzzyDate>) {
    if let Some(d) = d {
        x.start(ns, name);
        x.leaf_int(Ns::Common, "year", d.year.map(i64::from), 4);
        x.leaf_int(Ns::Common, "month", d.month.map(i64::from), 2);
        x.leaf_int(Ns::Common, "day", d.day.map(i64::from), 2);
        x.end();
    }
}

fn organization(x: &mut Xml, ns: Ns, name: &'static str, o: &Organization) -> Result<()> {
    x.start(ns, name);
    x.leaf(Ns::Common, "name", o.name.as_deref())?;
    if let Some(a) = &o.address {
        x.start(Ns::Common, "address");
        x.leaf(Ns::Common, "city", a.city.as_deref())?;
        x.leaf(Ns::Common, "region", a.region.as_deref())?;
        x.leaf(Ns::Common, "country", a.country.as_deref())?;
        x.end();
    }
    if let Some(d) = &o.disambiguated_organization {
        x.start(Ns::Common, "disambiguated-organization");
        x.leaf(
            Ns::Common,
            "disambiguated-organization-identifier",
            d.identifier.as_deref(),
        )?;
        x.leaf(Ns::Common, "disambiguation-source", d.source.as_deref())?;
        x.end();
    }
    x.end();
    Ok(())
}

fn opt_organization(
    x: &mut Xml,
    ns: Ns,
    name: &'static str,
    o: &Option<Organization>,
) -> Result<()> {
    match o {
        Some(o) => organization(x, ns, name, o),
        None => Ok(()),
    }
}

fn external_id(x: &mut Xml, ns: Ns, name: &'static str, e: &ExternalId) -> Result<()> {
    item(x, ns, name, &e.meta)?;
    x.leaf(Ns::Common, "external-id-type", e.id_type.as_deref())?;
    x.leaf(Ns::Common, "external-id-value", e.value.as_deref())?;
    if let Some(t) = &e.normalized {
        x.start(Ns::Common, "external-id-normalized");
        x.attr_bool("transient", t.transient);
        x.end_text(t.value.as_deref())?;
    }
    if let Some(n) = &e.normalized_error {
        x.start(Ns::Common, "external-id-normalized-error");
        x.attr_bool("transient", n.transient);
        x.leaf(Ns::Common, "error-code", n.error_code.as_deref())?;
        x.leaf(Ns::Common, "error-message", n.error_message.as_deref())?;
        x.end();
    }
    x.leaf(Ns::Common, "external-id-url", e.url.as_deref())?;
    x.leaf(
        Ns::Common,
        "external-id-relationship",
        e.relationship.as_ref().map(|r| r.as_str()),
    )?;
    x.end();
    Ok(())
}

/// `<ns:name>` holding `common:external-id` children (always written).
fn external_ids_in(x: &mut Xml, ns: Ns, name: &'static str, ids: &[ExternalId]) -> Result<()> {
    x.start(ns, name);
    for e in ids {
        external_id(x, Ns::Common, "external-id", e)?;
    }
    x.end();
    Ok(())
}

/// `common:external-ids`, always written (groups, proposals, resource items).
fn external_ids(x: &mut Xml, ids: &[ExternalId]) -> Result<()> {
    external_ids_in(x, Ns::Common, "external-ids", ids)
}

/// `common:external-ids`, written only when there is at least one id.
fn opt_external_ids(x: &mut Xml, ids: &[ExternalId]) -> Result<()> {
    if ids.is_empty() {
        Ok(())
    } else {
        external_ids(x, ids)
    }
}

fn opt_title(x: &mut Xml, ns: Ns, name: &'static str, t: &Option<Title>) -> Result<()> {
    let Some(t) = t else {
        return Ok(());
    };
    x.start(ns, name);
    x.leaf(Ns::Common, "title", t.title.as_deref())?;
    x.leaf(Ns::Common, "subtitle", t.subtitle.as_deref())?;
    if let Some(tt) = &t.translated_title {
        x.start(Ns::Common, "translated-title");
        x.attr("language-code", tt.language_code.as_deref())?;
        x.end_text(tt.value.as_deref())?;
    }
    x.end();
    Ok(())
}

fn contributor(x: &mut Xml, ns: Ns, c: &Contributor) -> Result<()> {
    x.start(ns, "contributor");
    opt_identifier(x, Ns::Common, "contributor-orcid", &c.contributor_orcid)?;
    x.leaf(ns, "credit-name", c.credit_name.as_deref())?;
    x.leaf(ns, "contributor-email", c.contributor_email.as_deref())?;
    if let Some(a) = &c.attributes {
        x.start(ns, "contributor-attributes");
        if ns == Ns::Funding {
            if a.sequence.is_some() {
                return Err(x.forbidden(
                    "contributor-sequence",
                    "a funding contributor has no contributor-sequence (funding-3.0.xsd)",
                ));
            }
        } else {
            x.leaf(
                ns,
                "contributor-sequence",
                a.sequence.as_ref().map(|v| v.as_str()),
            )?;
        }
        x.leaf(ns, "contributor-role", a.role.as_ref().map(|v| v.as_str()))?;
        x.end();
    }
    x.end();
    Ok(())
}

fn opt_contributors(x: &mut Xml, ns: Ns, cs: &[Contributor]) -> Result<()> {
    if !cs.is_empty() {
        x.start(ns, "contributors");
        for c in cs {
            contributor(x, ns, c)?;
        }
        x.end();
    }
    Ok(())
}

/// Starts `<ns:name path=...>` with `common:last-modified-date`.
fn container(
    x: &mut Xml,
    ns: Ns,
    name: &'static str,
    path: &Option<String>,
    lmd: &Option<String>,
) -> Result<()> {
    x.start(ns, name);
    x.attr("path", path.as_deref())?;
    x.leaf(Ns::Common, "last-modified-date", lmd.as_deref())
}

/// Starts a group: `common:last-modified-date`, `common:external-ids`
/// (always).
fn group(
    x: &mut Xml,
    ns: Ns,
    name: &'static str,
    lmd: &Option<String>,
    ids: &[ExternalId],
) -> Result<()> {
    x.start(ns, name);
    x.leaf(Ns::Common, "last-modified-date", lmd.as_deref())?;
    external_ids(x, ids)
}

// ------------------------------------------------------------------- works

fn work(x: &mut Xml, w: &Work) -> Result<()> {
    item(x, Ns::Work, "work", &w.meta)?;
    opt_title(x, Ns::Work, "title", &w.title)?;
    x.leaf(Ns::Work, "journal-title", w.journal_title.as_deref())?;
    x.leaf(
        Ns::Work,
        "short-description",
        w.short_description.as_deref(),
    )?;
    if let Some(c) = &w.citation {
        x.start(Ns::Work, "citation");
        x.leaf(
            Ns::Work,
            "citation-type",
            c.citation_type.as_ref().map(|v| v.as_str()),
        )?;
        x.leaf(Ns::Work, "citation-value", c.citation_value.as_deref())?;
        x.end();
    }
    x.leaf(Ns::Work, "type", w.work_type.as_ref().map(|v| v.as_str()))?;
    opt_date(x, Ns::Common, "publication-date", &w.publication_date);
    opt_external_ids(x, &w.external_ids)?;
    x.leaf(Ns::Common, "url", w.url.as_deref())?;
    opt_contributors(x, Ns::Work, &w.contributors)?;
    x.leaf(Ns::Common, "language-code", w.language_code.as_deref())?;
    if let Some(c) = &w.country {
        x.start(Ns::Common, "country");
        x.attr("visibility", c.visibility.as_ref().map(|v| v.as_str()))?;
        x.end_text(c.value.as_deref())?;
    }
    x.end();
    Ok(())
}

fn work_summary(x: &mut Xml, w: &WorkSummary) -> Result<()> {
    item(x, Ns::Work, "work-summary", &w.meta)?;
    opt_title(x, Ns::Work, "title", &w.title)?;
    opt_external_ids(x, &w.external_ids)?;
    x.leaf(Ns::Common, "url", w.url.as_deref())?;
    x.leaf(Ns::Work, "type", w.work_type.as_ref().map(|v| v.as_str()))?;
    opt_date(x, Ns::Common, "publication-date", &w.publication_date);
    x.leaf(Ns::Work, "journal-title", w.journal_title.as_deref())?;
    x.end();
    Ok(())
}

fn works(x: &mut Xml, w: &Works) -> Result<()> {
    container(x, Ns::Activities, "works", &w.path, &w.last_modified_date)?;
    for g in &w.groups {
        group(
            x,
            Ns::Activities,
            "group",
            &g.last_modified_date,
            &g.external_ids,
        )?;
        for s in &g.work_summaries {
            work_summary(x, s)?;
        }
        x.end();
    }
    x.end();
    Ok(())
}

// ---------------------------------------------------------------- fundings

fn funding(x: &mut Xml, f: &Funding) -> Result<()> {
    item(x, Ns::Funding, "funding", &f.meta)?;
    x.leaf(
        Ns::Funding,
        "type",
        f.funding_type.as_ref().map(|v| v.as_str()),
    )?;
    x.leaf(
        Ns::Funding,
        "organization-defined-type",
        f.organization_defined_type.as_deref(),
    )?;
    opt_title(x, Ns::Funding, "title", &f.title)?;
    x.leaf(
        Ns::Funding,
        "short-description",
        f.short_description.as_deref(),
    )?;
    if let Some(a) = &f.amount {
        x.start(Ns::Funding, "amount");
        x.attr("currency-code", a.currency_code.as_deref())?;
        x.end_text(a.value.as_deref())?;
    }
    x.leaf(Ns::Common, "url", f.url.as_deref())?;
    opt_date(x, Ns::Common, "start-date", &f.start_date);
    opt_date(x, Ns::Common, "end-date", &f.end_date);
    opt_external_ids(x, &f.external_ids)?;
    opt_contributors(x, Ns::Funding, &f.contributors)?;
    opt_organization(x, Ns::Common, "organization", &f.organization)?;
    x.end();
    Ok(())
}

fn funding_summary(x: &mut Xml, f: &FundingSummary) -> Result<()> {
    item(x, Ns::Funding, "funding-summary", &f.meta)?;
    opt_title(x, Ns::Funding, "title", &f.title)?;
    opt_external_ids(x, &f.external_ids)?;
    x.leaf(Ns::Common, "url", f.url.as_deref())?;
    x.leaf(
        Ns::Funding,
        "type",
        f.funding_type.as_ref().map(|v| v.as_str()),
    )?;
    opt_date(x, Ns::Common, "start-date", &f.start_date);
    opt_date(x, Ns::Common, "end-date", &f.end_date);
    opt_organization(x, Ns::Common, "organization", &f.organization)?;
    x.end();
    Ok(())
}

fn fundings(x: &mut Xml, f: &Fundings) -> Result<()> {
    container(
        x,
        Ns::Activities,
        "fundings",
        &f.path,
        &f.last_modified_date,
    )?;
    for g in &f.groups {
        group(
            x,
            Ns::Activities,
            "group",
            &g.last_modified_date,
            &g.external_ids,
        )?;
        for s in &g.funding_summaries {
            funding_summary(x, s)?;
        }
        x.end();
    }
    x.end();
    Ok(())
}

// ------------------------------------------------------------ affiliations

fn affiliation(x: &mut Xml, a: &Affiliation, summary: bool) -> Result<()> {
    let name = if summary {
        a.kind.summary_element()
    } else {
        a.kind.as_str()
    };
    item(x, Ns::of_affiliation(a.kind), name, &a.meta)?;
    x.leaf(Ns::Common, "department-name", a.department_name.as_deref())?;
    x.leaf(Ns::Common, "role-title", a.role_title.as_deref())?;
    opt_date(x, Ns::Common, "start-date", &a.start_date);
    opt_date(x, Ns::Common, "end-date", &a.end_date);
    opt_organization(x, Ns::Common, "organization", &a.organization)?;
    x.leaf(Ns::Common, "url", a.url.as_deref())?;
    opt_external_ids(x, &a.external_ids)?;
    x.end();
    Ok(())
}

fn affiliations(x: &mut Xml, a: &Affiliations) -> Result<()> {
    container(
        x,
        Ns::Activities,
        a.kind.container_element(),
        &a.path,
        &a.last_modified_date,
    )?;
    for g in &a.groups {
        group(
            x,
            Ns::Activities,
            "affiliation-group",
            &g.last_modified_date,
            &g.external_ids,
        )?;
        for s in &g.summaries {
            affiliation(x, s, true)?;
        }
        x.end();
    }
    x.end();
    Ok(())
}

fn opt_affiliations(x: &mut Xml, a: &Option<Affiliations>) -> Result<()> {
    match a {
        Some(a) => affiliations(x, a),
        None => Ok(()),
    }
}

// ------------------------------------------------------------ peer reviews

fn peer_review_summary(x: &mut Xml, p: &PeerReviewSummary) -> Result<()> {
    item(x, Ns::PeerReview, "peer-review-summary", &p.meta)?;
    x.leaf(
        Ns::PeerReview,
        "reviewer-role",
        p.reviewer_role.as_ref().map(|v| v.as_str()),
    )?;
    opt_external_ids(x, &p.external_ids)?;
    x.leaf(Ns::PeerReview, "review-url", p.review_url.as_deref())?;
    x.leaf(
        Ns::PeerReview,
        "review-type",
        p.review_type.as_ref().map(|v| v.as_str()),
    )?;
    opt_date(x, Ns::PeerReview, "completion-date", &p.completion_date);
    x.leaf(
        Ns::PeerReview,
        "review-group-id",
        p.review_group_id.as_deref(),
    )?;
    opt_organization(
        x,
        Ns::PeerReview,
        "convening-organization",
        &p.convening_organization,
    )?;
    x.end();
    Ok(())
}

fn peer_review(x: &mut Xml, p: &PeerReview) -> Result<()> {
    item(x, Ns::PeerReview, "peer-review", &p.meta)?;
    x.leaf(
        Ns::PeerReview,
        "reviewer-role",
        p.reviewer_role.as_ref().map(|v| v.as_str()),
    )?;
    external_ids_in(
        x,
        Ns::PeerReview,
        "review-identifiers",
        &p.review_identifiers,
    )?;
    x.leaf(Ns::PeerReview, "review-url", p.review_url.as_deref())?;
    x.leaf(
        Ns::PeerReview,
        "review-type",
        p.review_type.as_ref().map(|v| v.as_str()),
    )?;
    opt_date(
        x,
        Ns::PeerReview,
        "review-completion-date",
        &p.review_completion_date,
    );
    x.leaf(
        Ns::PeerReview,
        "review-group-id",
        p.review_group_id.as_deref(),
    )?;
    if let Some(e) = &p.subject_external_identifier {
        external_id(x, Ns::PeerReview, "subject-external-identifier", e)?;
    }
    x.leaf(
        Ns::PeerReview,
        "subject-container-name",
        p.subject_container_name.as_deref(),
    )?;
    x.leaf(Ns::PeerReview, "subject-type", p.subject_type.as_deref())?;
    opt_title(x, Ns::PeerReview, "subject-name", &p.subject_name)?;
    x.leaf(Ns::PeerReview, "subject-url", p.subject_url.as_deref())?;
    opt_organization(
        x,
        Ns::PeerReview,
        "convening-organization",
        &p.convening_organization,
    )?;
    x.end();
    Ok(())
}

fn peer_reviews(x: &mut Xml, p: &PeerReviews) -> Result<()> {
    container(
        x,
        Ns::Activities,
        "peer-reviews",
        &p.path,
        &p.last_modified_date,
    )?;
    for g in &p.groups {
        group(
            x,
            Ns::Activities,
            "group",
            &g.last_modified_date,
            &g.external_ids,
        )?;
        for d in &g.duplicates {
            group(
                x,
                Ns::Activities,
                "peer-review-group",
                &d.last_modified_date,
                &d.external_ids,
            )?;
            for s in &d.summaries {
                peer_review_summary(x, s)?;
            }
            x.end();
        }
        x.end();
    }
    x.end();
    Ok(())
}

// ------------------------------------------------------ research resources

fn hosts(x: &mut Xml, orgs: &[Organization]) -> Result<()> {
    x.start(Ns::ResearchResource, "hosts");
    for o in orgs {
        organization(x, Ns::Common, "organization", o)?;
    }
    x.end();
    Ok(())
}

fn opt_proposal(x: &mut Xml, p: &Option<Proposal>) -> Result<()> {
    let Some(p) = p else {
        return Ok(());
    };
    x.start(Ns::ResearchResource, "proposal");
    opt_title(x, Ns::ResearchResource, "title", &p.title)?;
    hosts(x, &p.hosts)?;
    external_ids(x, &p.external_ids)?;
    opt_date(x, Ns::Common, "start-date", &p.start_date);
    opt_date(x, Ns::Common, "end-date", &p.end_date);
    x.leaf(Ns::Common, "url", p.url.as_deref())?;
    x.end();
    Ok(())
}

fn resource_item(x: &mut Xml, r: &ResourceItem) -> Result<()> {
    x.start(Ns::ResearchResource, "resource-item");
    x.leaf(
        Ns::ResearchResource,
        "resource-name",
        r.resource_name.as_deref(),
    )?;
    x.leaf(
        Ns::ResearchResource,
        "resource-type",
        r.resource_type.as_ref().map(|v| v.as_str()),
    )?;
    hosts(x, &r.hosts)?;
    external_ids(x, &r.external_ids)?;
    x.leaf(Ns::Common, "url", r.url.as_deref())?;
    x.end();
    Ok(())
}

fn research_resource(x: &mut Xml, r: &ResearchResource) -> Result<()> {
    item(x, Ns::ResearchResource, "research-resource", &r.meta)?;
    opt_proposal(x, &r.proposal)?;
    x.start(Ns::ResearchResource, "resource-items");
    for i in &r.resource_items {
        resource_item(x, i)?;
    }
    x.end();
    x.end();
    Ok(())
}

fn research_resource_summary(x: &mut Xml, r: &ResearchResourceSummary) -> Result<()> {
    item(
        x,
        Ns::ResearchResource,
        "research-resource-summary",
        &r.meta,
    )?;
    opt_proposal(x, &r.proposal)?;
    x.end();
    Ok(())
}

fn research_resources(x: &mut Xml, r: &ResearchResources) -> Result<()> {
    container(
        x,
        Ns::Activities,
        "research-resources",
        &r.path,
        &r.last_modified_date,
    )?;
    for g in &r.groups {
        group(
            x,
            Ns::Activities,
            "group",
            &g.last_modified_date,
            &g.external_ids,
        )?;
        for s in &g.summaries {
            research_resource_summary(x, s)?;
        }
        x.end();
    }
    x.end();
    Ok(())
}

fn activities_summary(x: &mut Xml, a: &ActivitiesSummary) -> Result<()> {
    container(
        x,
        Ns::Activities,
        "activities-summary",
        &a.path,
        &a.last_modified_date,
    )?;
    opt_affiliations(x, &a.distinctions)?;
    opt_affiliations(x, &a.educations)?;
    opt_affiliations(x, &a.employments)?;
    if let Some(f) = &a.fundings {
        fundings(x, f)?;
    }
    opt_affiliations(x, &a.invited_positions)?;
    opt_affiliations(x, &a.memberships)?;
    if let Some(p) = &a.peer_reviews {
        peer_reviews(x, p)?;
    }
    opt_affiliations(x, &a.qualifications)?;
    if let Some(r) = &a.research_resources {
        research_resources(x, r)?;
    }
    opt_affiliations(x, &a.services)?;
    if let Some(w) = &a.works {
        works(x, w)?;
    }
    x.end();
    Ok(())
}

// ------------------------------------------------------------------ person

fn name(x: &mut Xml, ns: Ns, nm: &Name) -> Result<()> {
    x.start(ns, "name");
    x.attr("visibility", nm.visibility.as_ref().map(|v| v.as_str()))?;
    x.attr("path", nm.path.as_deref())?;
    x.leaf(Ns::Common, "created-date", nm.created_date.as_deref())?;
    x.leaf(
        Ns::Common,
        "last-modified-date",
        nm.last_modified_date.as_deref(),
    )?;
    x.leaf(
        Ns::PersonalDetails,
        "given-names",
        nm.given_names.as_deref(),
    )?;
    x.leaf(
        Ns::PersonalDetails,
        "family-name",
        nm.family_name.as_deref(),
    )?;
    x.leaf(
        Ns::PersonalDetails,
        "credit-name",
        nm.credit_name.as_deref(),
    )?;
    x.end();
    Ok(())
}

fn biography(x: &mut Xml, ns: Ns, b: &Biography) -> Result<()> {
    x.start(ns, "biography");
    x.attr("visibility", b.visibility.as_ref().map(|v| v.as_str()))?;
    x.attr("path", b.path.as_deref())?;
    x.leaf(Ns::Common, "created-date", b.created_date.as_deref())?;
    x.leaf(
        Ns::Common,
        "last-modified-date",
        b.last_modified_date.as_deref(),
    )?;
    x.leaf(Ns::PersonalDetails, "content", b.content.as_deref())?;
    x.end();
    Ok(())
}

fn other_name(x: &mut Xml, v: &OtherName) -> Result<()> {
    item(x, Ns::OtherName, "other-name", &v.meta)?;
    x.leaf(Ns::OtherName, "content", v.content.as_deref())?;
    x.end();
    Ok(())
}

fn researcher_url(x: &mut Xml, v: &ResearcherUrl) -> Result<()> {
    item(x, Ns::ResearcherUrl, "researcher-url", &v.meta)?;
    x.leaf(Ns::ResearcherUrl, "url-name", v.url_name.as_deref())?;
    x.leaf(Ns::ResearcherUrl, "url", v.url.as_deref())?;
    x.end();
    Ok(())
}

fn email(x: &mut Xml, v: &Email) -> Result<()> {
    x.start(Ns::Email, "email");
    meta_attrs(x, &v.meta)?;
    x.attr_bool("verified", v.verified);
    x.attr_bool("primary", v.primary);
    meta_kids(x, &v.meta)?;
    x.leaf(Ns::Email, "email", v.email.as_deref())?;
    x.end();
    Ok(())
}

fn address(x: &mut Xml, v: &Address) -> Result<()> {
    item(x, Ns::Address, "address", &v.meta)?;
    x.leaf(Ns::Address, "country", v.country.as_deref())?;
    x.end();
    Ok(())
}

fn keyword(x: &mut Xml, v: &Keyword) -> Result<()> {
    item(x, Ns::Keyword, "keyword", &v.meta)?;
    x.leaf(Ns::Keyword, "content", v.content.as_deref())?;
    x.end();
    Ok(())
}

fn person_external_identifier(x: &mut Xml, v: &PersonExternalIdentifier) -> Result<()> {
    external_id(x, Ns::ExternalIdentifier, "external-identifier", v)
}

type Each<T> = fn(&mut Xml, &T) -> Result<()>;

fn list<T>(x: &mut Xml, ns: Ns, name: &'static str, l: &ItemList<T>, each: Each<T>) -> Result<()> {
    container(x, ns, name, &l.path, &l.last_modified_date)?;
    for v in &l.items {
        each(x, v)?;
    }
    x.end();
    Ok(())
}

fn opt_list<T>(
    x: &mut Xml,
    ns: Ns,
    name: &'static str,
    l: &Option<ItemList<T>>,
    each: Each<T>,
) -> Result<()> {
    match l {
        Some(l) => list(x, ns, name, l, each),
        None => Ok(()),
    }
}

fn opt_other_names(x: &mut Xml, l: &Option<OtherNames>) -> Result<()> {
    opt_list(x, Ns::OtherName, "other-names", l, other_name)
}

fn person(x: &mut Xml, p: &Person) -> Result<()> {
    container(x, Ns::Person, "person", &p.path, &p.last_modified_date)?;
    if let Some(v) = &p.name {
        name(x, Ns::Person, v)?;
    }
    opt_other_names(x, &p.other_names)?;
    if let Some(v) = &p.biography {
        biography(x, Ns::Person, v)?;
    }
    opt_list(
        x,
        Ns::ResearcherUrl,
        "researcher-urls",
        &p.researcher_urls,
        researcher_url,
    )?;
    opt_list(x, Ns::Email, "emails", &p.emails, email)?;
    opt_list(x, Ns::Address, "addresses", &p.addresses, address)?;
    opt_list(x, Ns::Keyword, "keywords", &p.keywords, keyword)?;
    opt_list(
        x,
        Ns::ExternalIdentifier,
        "external-identifiers",
        &p.external_identifiers,
        person_external_identifier,
    )?;
    x.end();
    Ok(())
}

fn personal_details(x: &mut Xml, p: &PersonalDetails) -> Result<()> {
    container(
        x,
        Ns::PersonalDetails,
        "personal-details",
        &p.path,
        &p.last_modified_date,
    )?;
    if let Some(v) = &p.name {
        name(x, Ns::PersonalDetails, v)?;
    }
    opt_other_names(x, &p.other_names)?;
    if let Some(v) = &p.biography {
        biography(x, Ns::PersonalDetails, v)?;
    }
    x.end();
    Ok(())
}

// ------------------------------------------------------------------ record

fn history(x: &mut Xml, h: &History) -> Result<()> {
    x.start(Ns::History, "history");
    x.attr("visibility", h.visibility.as_ref().map(|v| v.as_str()))?;
    x.leaf(
        Ns::History,
        "creation-method",
        h.creation_method.as_ref().map(|v| v.as_str()),
    )?;
    x.leaf(Ns::History, "completion-date", h.completion_date.as_deref())?;
    x.leaf(Ns::History, "submission-date", h.submission_date.as_deref())?;
    x.leaf(
        Ns::Common,
        "last-modified-date",
        h.last_modified_date.as_deref(),
    )?;
    x.leaf_bool(Ns::History, "claimed", h.claimed);
    if let Some(s) = &h.source {
        source(x, s)?;
    }
    x.leaf(
        Ns::History,
        "deactivation-date",
        h.deactivation_date.as_deref(),
    )?;
    x.leaf_bool(Ns::History, "verified-email", h.verified_email);
    x.leaf_bool(
        Ns::History,
        "verified-primary-email",
        h.verified_primary_email,
    );
    x.end();
    Ok(())
}

fn preferences(x: &mut Xml, p: &Preferences) -> Result<()> {
    x.start(Ns::Preferences, "preferences");
    x.leaf(Ns::Preferences, "locale", p.locale.as_deref())?;
    x.end();
    Ok(())
}

fn record(x: &mut Xml, r: &Record) -> Result<()> {
    x.start(Ns::Record, "record");
    x.attr("path", r.path.as_deref())?;
    opt_identifier(x, Ns::Common, "orcid-identifier", &r.orcid_identifier)?;
    if let Some(p) = &r.preferences {
        preferences(x, p)?;
    }
    if let Some(h) = &r.history {
        history(x, h)?;
    }
    if let Some(p) = &r.person {
        person(x, p)?;
    }
    if let Some(a) = &r.activities_summary {
        activities_summary(x, a)?;
    }
    x.end();
    Ok(())
}

fn orcid_error(x: &mut Xml, e: &OrcidError) -> Result<()> {
    x.start(Ns::Error, "error");
    x.leaf_int(Ns::Error, "response-code", e.response_code, 0);
    x.leaf(
        Ns::Error,
        "developer-message",
        e.developer_message.as_deref(),
    )?;
    x.leaf(Ns::Error, "user-message", e.user_message.as_deref())?;
    x.leaf_int(Ns::Error, "error-code", e.error_code, 0);
    x.leaf(Ns::Error, "more-info", e.more_info.as_deref())?;
    x.end();
    Ok(())
}

fn bulk(x: &mut Xml, b: &Bulk) -> Result<()> {
    x.start(Ns::Bulk, "bulk");
    for i in &b.items {
        match i {
            BulkItem::Work(w) => work(x, w)?,
            BulkItem::Error(e) => orcid_error(x, e)?,
        }
    }
    x.end();
    Ok(())
}

fn search(x: &mut Xml, s: &Search) -> Result<()> {
    x.start(Ns::Search, "search");
    x.attr_int("num-found", s.num_found);
    for r in &s.results {
        x.start(Ns::Search, "result");
        identifier(x, Ns::Common, "orcid-identifier", r)?;
        x.end();
    }
    x.end();
    Ok(())
}

fn expanded_search(x: &mut Xml, s: &ExpandedSearch) -> Result<()> {
    let ns = Ns::ExpandedSearch;
    x.start(ns, "expanded-search");
    x.attr_int("num-found", s.num_found);
    for r in &s.results {
        x.start(ns, "expanded-result");
        x.leaf(ns, "orcid-id", r.orcid_id.as_deref())?;
        x.leaf(ns, "given-names", r.given_names.as_deref())?;
        x.leaf(ns, "family-names", r.family_names.as_deref())?;
        x.leaf(ns, "credit-name", r.credit_name.as_deref())?;
        for v in &r.other_names {
            x.text(ns, "other-name", v)?;
        }
        for v in &r.emails {
            x.text(ns, "email", v)?;
        }
        for v in &r.institution_names {
            x.text(ns, "institution-name", v)?;
        }
        x.end();
    }
    x.end();
    Ok(())
}

fn write_root(x: &mut Xml, root: &Root) -> Result<()> {
    match root {
        Root::Record(r) => record(x, r),
        Root::ActivitiesSummary(a) => activities_summary(x, a),
        Root::Works(w) => works(x, w),
        Root::Work(w) => work(x, w),
        Root::WorkSummary(w) => work_summary(x, w),
        Root::Fundings(f) => fundings(x, f),
        Root::Funding(f) => funding(x, f),
        Root::FundingSummary(f) => funding_summary(x, f),
        Root::Affiliations(a) => affiliations(x, a),
        Root::Affiliation(a) => affiliation(x, a, false),
        Root::AffiliationSummary(a) => affiliation(x, a, true),
        Root::PeerReviews(p) => peer_reviews(x, p),
        Root::PeerReview(p) => peer_review(x, p),
        Root::PeerReviewSummary(p) => peer_review_summary(x, p),
        Root::ResearchResources(r) => research_resources(x, r),
        Root::ResearchResource(r) => research_resource(x, r),
        Root::ResearchResourceSummary(r) => research_resource_summary(x, r),
        Root::Person(p) => person(x, p),
        Root::PersonalDetails(p) => personal_details(x, p),
        Root::Name(v) => name(x, Ns::PersonalDetails, v),
        Root::Biography(v) => biography(x, Ns::PersonalDetails, v),
        Root::CreditName(v) => {
            x.start(Ns::PersonalDetails, "credit-name");
            x.end_text(v.as_deref())
        }
        Root::OtherNames(l) => list(x, Ns::OtherName, "other-names", l, other_name),
        Root::OtherName(v) => other_name(x, v),
        Root::ResearcherUrls(l) => list(x, Ns::ResearcherUrl, "researcher-urls", l, researcher_url),
        Root::ResearcherUrl(v) => researcher_url(x, v),
        Root::Emails(l) => list(x, Ns::Email, "emails", l, email),
        Root::Email(v) => email(x, v),
        Root::Addresses(l) => list(x, Ns::Address, "addresses", l, address),
        Root::Address(v) => address(x, v),
        Root::Keywords(l) => list(x, Ns::Keyword, "keywords", l, keyword),
        Root::Keyword(v) => keyword(x, v),
        Root::ExternalIdentifiers(l) => list(
            x,
            Ns::ExternalIdentifier,
            "external-identifiers",
            l,
            person_external_identifier,
        ),
        Root::ExternalIdentifier(v) => person_external_identifier(x, v),
        Root::History(h) => history(x, h),
        Root::Preferences(p) => preferences(x, p),
        Root::Bulk(b) => bulk(x, b),
        Root::Error(e) => orcid_error(x, e),
        Root::Search(s) => search(x, s),
        Root::ExpandedSearch(s) => expanded_search(x, s),
        // written by `to_string` without this emitter (`group_id`)
        Root::GroupIdRecord(_) | Root::GroupIdRecords(_) => Err(Error::Unwritable {
            path: String::new(),
            reason: "group-id documents are written by to_string".to_string(),
        }),
        Root::Unknown(name) => Err(Error::Unwritable {
            path: name.clone(),
            reason: "unknown root element".to_string(),
        }),
    }
}

/// Writes any document root as ORCID v3.0 XML, exactly as the model holds it
/// (no validation). Errors only for [`Root::Unknown`] and for characters XML
/// 1.0 cannot carry.
///
/// ```
/// use yorc::model::{Keyword, Root};
///
/// let k = Keyword { content: Some("hydrothermal vents".into()), ..Keyword::default() };
/// let xml = yorc::xml::to_string(&Root::Keyword(k.clone()))?;
/// assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<keyword:keyword"));
/// assert!(xml.contains("<keyword:content>hydrothermal vents</keyword:content>"));
/// // and back
/// assert_eq!(yorc::xml::from_str(&xml)?.root, Root::Keyword(k));
/// # Ok::<(), yorc::Error>(())
/// ```
pub fn to_string(root: &Root) -> Result<String> {
    match root {
        Root::Unknown(name) => {
            return Err(Error::Unwritable {
                path: name.clone(),
                reason: "unknown root element".to_string(),
            });
        }
        Root::GroupIdRecord(r) => return group_id::record_document(r),
        Root::GroupIdRecords(p) => return group_id::records_document(p),
        _ => {}
    }
    let mut x = Xml::new(estimate(root));
    write_root(&mut x, root)?;
    Ok(x.finish())
}

/// [`to_string`] of a [`Document`]'s root (its `root_path` is not used: every
/// model element carries its own `path`).
///
/// ```
/// let doc = yorc::xml::from_str(r#"<record:record path="/0000-0002-1825-0097" xmlns:record="http://www.orcid.org/ns/record"/>"#)?;
/// let xml = yorc::xml::document_to_string(&doc)?;
/// assert!(xml.contains("<record:record"));
/// # Ok::<(), yorc::Error>(())
/// ```
pub fn document_to_string(doc: &Document) -> Result<String> {
    to_string(&doc.root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_layout_and_escaping() {
        let k = Keyword {
            meta: ItemMeta {
                put_code: Some(7),
                visibility: Some(Visibility::Public),
                ..ItemMeta::default()
            },
            content: Some("R&D <x> \"q\"".to_string()),
        };
        let xml = to_string(&Root::Keyword(k)).unwrap();
        assert_eq!(
            xml,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <keyword:keyword xmlns:keyword=\"http://www.orcid.org/ns/keyword\" put-code=\"7\" visibility=\"public\">\n  \
             <keyword:content>R&amp;D &lt;x&gt; \"q\"</keyword:content>\n\
             </keyword:keyword>\n"
        );
    }

    #[test]
    fn dates_are_padded_and_namespaces_declared_in_order() {
        let a = Address {
            country: Some("US".to_string()),
            meta: ItemMeta {
                source: Some(Source {
                    source_name: Some("x".to_string()),
                    ..Source::default()
                }),
                ..ItemMeta::default()
            },
        };
        let xml = to_string(&Root::Address(a)).unwrap();
        assert!(xml.contains(
            "<address:address xmlns:address=\"http://www.orcid.org/ns/address\" xmlns:common=\"http://www.orcid.org/ns/common\">"
        ));
        let w = Work {
            publication_date: Some(FuzzyDate {
                year: Some(2012),
                month: Some(1),
                day: Some(2),
            }),
            ..Work::default()
        };
        let xml = to_string(&Root::Work(w)).unwrap();
        assert!(xml.contains("<common:year>2012</common:year>"));
        assert!(xml.contains("<common:month>01</common:month>"));
        assert!(xml.contains("<common:day>02</common:day>"));
    }

    #[test]
    fn unwritable_input_is_an_error_not_a_panic() {
        let k = Keyword {
            content: Some("bell\u{7}".to_string()),
            ..Keyword::default()
        };
        assert!(matches!(
            to_string(&Root::Keyword(k)),
            Err(Error::Unwritable { ref path, .. }) if path == "keyword/content"
        ));
        assert!(matches!(
            to_string(&Root::Unknown("x".to_string())),
            Err(Error::Unwritable { .. })
        ));
    }

    /// The escaping rules, one character at a time (the writer before the
    /// chunked scan): the result `escape` must match.
    fn naive(s: &str, attr: bool) -> std::result::Result<String, char> {
        let mut out = String::new();
        for c in s.chars() {
            let ok =
                matches!(c, '\t' | '\n' | '\r') || (c >= ' ' && c != '\u{FFFE}' && c != '\u{FFFF}');
            if !ok {
                return Err(c);
            }
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '\r' => out.push_str("&#13;"),
                '"' if attr => out.push_str("&quot;"),
                '\t' if attr => out.push_str("&#9;"),
                '\n' if attr => out.push_str("&#10;"),
                c => out.push(c),
            }
        }
        Ok(out)
    }

    fn fast(s: &str, attr: bool) -> std::result::Result<String, char> {
        // a non-empty buffer, as in the writer
        let mut out = String::from("<");
        let r = if attr {
            escape_attr(s, &mut out)
        } else {
            escape_text(s, &mut out)
        };
        r.map(|()| out[1..].to_string())
    }

    fn same(s: &str) {
        for attr in [false, true] {
            assert_eq!(fast(s, attr), naive(s, attr), "{s:?} (attr: {attr})");
        }
    }

    #[test]
    fn escaping_plain_runs_are_copied() {
        for n in 0..40 {
            let s = "abcdefghij".repeat(4)[..n].to_string();
            assert_eq!(fast(&s, false).as_deref(), Ok(s.as_str()));
            assert_eq!(fast(&s, true).as_deref(), Ok(s.as_str()));
        }
        // bytes that are close to the special ones but plain
        same("!#$%'()*+,-./0123456789:;=?@[\\]^_`{|}~\u{7F}");
        same("\u{80}\u{9F}\u{A0}\u{FF}\u{EFFF}\u{F000}\u{FEFF}\u{FF0C}\u{FFFD}\u{10000}\u{10FFFF}");
    }

    #[test]
    fn escaping_every_special_character() {
        assert_eq!(
            fast("a&b<c>d\"e'f\rg\th\ni", false).as_deref(),
            Ok("a&amp;b&lt;c&gt;d\"e'f&#13;g\th\ni")
        );
        assert_eq!(
            fast("a&b<c>d\"e'f\rg\th\ni", true).as_deref(),
            Ok("a&amp;b&lt;c&gt;d&quot;e'f&#13;g&#9;h&#10;i")
        );
        assert_eq!(
            fast("&&<<>>", false).as_deref(),
            Ok("&amp;&amp;&lt;&lt;&gt;&gt;")
        );
        assert_eq!(fast("\"\"", true).as_deref(), Ok("&quot;&quot;"));
        same("");
        same("&");
        same("\r\n\t");
    }

    #[test]
    fn escaping_at_every_chunk_position() {
        let specials = [
            "&", "<", ">", "\"", "'", "\r", "\t", "\n", "\u{0}", "\u{1}", "\u{8}", "\u{B}",
            "\u{C}", "\u{1F}", "\u{FFFE}", "\u{FFFF}", "\u{FFFD}", "\u{FF0C}", "é", "中", "😀",
        ];
        for sp in specials {
            for before in 0..20 {
                for after in [0, 1, 7, 8, 9, 15] {
                    let s = format!("{}{sp}{}", "x".repeat(before), "y".repeat(after));
                    same(&s);
                    // two specials in one chunk, and a special after another's run
                    same(&format!("{s}{sp}"));
                    same(&format!("{sp}{s}&"));
                }
            }
        }
    }

    #[test]
    fn escaping_control_characters_are_errors() {
        for c in (0u8..0x20).map(char::from) {
            let s = format!("ok {c} more text after it");
            let want = if matches!(c, '\t' | '\n' | '\r') {
                naive(&s, false)
            } else {
                Err(c)
            };
            assert_eq!(fast(&s, false), want, "{c:?}");
            assert_eq!(fast(&s, true).is_ok(), want.is_ok(), "{c:?}");
        }
        assert_eq!(fast("1234567\u{FFFE}", false), Err('\u{FFFE}'));
        assert_eq!(fast("12345678\u{FFFF}", true), Err('\u{FFFF}'));
        // the first offending character is reported
        assert_eq!(fast("a&\u{FFFF}\u{1}", false), Err('\u{FFFF}'));
        assert_eq!(fast("a&\u{2}\u{FFFF}", true), Err('\u{2}'));
    }

    #[test]
    fn escaping_multibyte_utf8_next_to_escapes() {
        same("é&ü<中>😀\"Ω'\r\u{FFFD}&\u{EF}");
        same("&é<ü>中\"😀");
        same("ééééééé&");
        same("😀😀&😀😀<😀\u{FF0C}>\u{FFFD}");
        assert_eq!(
            fast("Ångström & Čapek <中文> \"😀\"", true).as_deref(),
            Ok("Ångström &amp; Čapek &lt;中文&gt; &quot;😀&quot;")
        );
        assert_eq!(fast("中\u{FFFE}", false), Err('\u{FFFE}'));
    }

    #[test]
    fn integers_are_padded_like_format() {
        for v in [
            0i64,
            5,
            -5,
            12,
            -12,
            123,
            2012,
            -2012,
            12345,
            i64::MAX,
            i64::MIN,
        ] {
            for w in [0usize, 2, 4] {
                let mut s = String::new();
                push_int(&mut s, v, w);
                assert_eq!(s, format!("{v:0w$}"), "{v} width {w}");
            }
        }
    }

    #[test]
    fn unwritable_paths_for_attributes_and_text_with_attributes() {
        let k = Keyword {
            meta: ItemMeta {
                path: Some("/x\u{1}".to_string()),
                ..ItemMeta::default()
            },
            content: Some("fine".to_string()),
        };
        assert_eq!(
            to_string(&Root::Keyword(k)),
            Err(Error::Unwritable {
                path: "keyword/@path".to_string(),
                reason: "character U+0001 is not allowed in XML 1.0".to_string(),
            })
        );
        let w = Work {
            country: Some(Country {
                value: Some("U\u{FFFF}S".to_string()),
                visibility: Some(Visibility::Public),
            }),
            ..Work::default()
        };
        assert_eq!(
            to_string(&Root::Work(w)),
            Err(Error::Unwritable {
                path: "work/country".to_string(),
                reason: "character U+FFFF is not allowed in XML 1.0".to_string(),
            })
        );
    }

    #[test]
    fn empty_elements_and_text_with_attributes() {
        let w = Work {
            publication_date: Some(FuzzyDate::default()),
            country: Some(Country {
                value: None,
                visibility: Some(Visibility::Public),
            }),
            title: Some(Title {
                title: Some(String::new()),
                ..Title::default()
            }),
            ..Work::default()
        };
        assert_eq!(
            to_string(&Root::Work(w)).unwrap(),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <work:work xmlns:common=\"http://www.orcid.org/ns/common\" xmlns:work=\"http://www.orcid.org/ns/work\">\n  \
             <work:title>\n    \
             <common:title></common:title>\n  \
             </work:title>\n  \
             <common:publication-date/>\n  \
             <common:country visibility=\"public\"/>\n\
             </work:work>\n"
        );
        assert_eq!(
            to_string(&Root::CreditName(None)).unwrap(),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <personal-details:credit-name xmlns:personal-details=\"http://www.orcid.org/ns/personal-details\"/>\n"
        );
    }
}
