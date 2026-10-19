//! ORCID v3.0 XML → [`Document`].
//!
//! The document is read with quick-xml into a small element tree, then mapped
//! onto the model. Elements are matched by local name (prefixes and namespace
//! URIs are ignored, as in yorc-duckdb); unknown elements and attributes are
//! skipped. Text follows yorc-duckdb's rules: a leaf's text is trimmed of
//! ASCII space/tab/CR/LF, empty text counts as absent, and when an element
//! that may occur once is repeated, each field keeps the first non-empty
//! value. Entities: the five predefined ones and numeric character
//! references are decoded; unknown entities and stray `&` are kept literally.
//!
//! Reads are lenient where yorc-duckdb and yorc-zig are strict: a
//! malformed attribute (no `=`, an unquoted value) is skipped, not an
//! error, and characters XML 1.0 forbids (most C0 controls, U+FFFE, U+FFFF)
//! are read as they are; writing such text back is an
//! [`Error::Unwritable`]. Invalid UTF-8 is always an [`Error::Utf8`].
//! Elements nested deeper than 256 levels are an [`Error::Xml`] (ORCID
//! documents nest about 12 deep; yorc-zig has the same limit), so a crafted
//! document cannot exhaust the stack.
//!
//! Writing: [`to_string`] (any root, as the model holds it); see
//! [`mod@write`].

pub mod write;

pub use write::{Ns, document_to_string, to_string};

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::error::{Error, Result};
use crate::model::*;

/// Parses an ORCID XML document.
///
/// ```
/// use yorc::model::Root;
///
/// let doc = yorc::xml::from_str(r#"<record:record path="/0000-0002-1825-0097" xmlns:record="http://www.orcid.org/ns/record"/>"#)?;
/// assert_eq!(doc.root_name, "record");
/// assert_eq!(doc.orcid(), Some("0000-0002-1825-0097"));
/// assert!(matches!(doc.root, Root::Record(_)));
/// # Ok::<(), yorc::Error>(())
/// ```
pub fn from_str(xml: &str) -> Result<Document> {
    let root = parse_tree(xml)?;
    Ok(document(&root))
}

/// Parses an ORCID XML document from bytes (UTF-8, optional BOM).
///
/// ```
/// let bytes = b"\xEF\xBB\xBF<keyword:keyword xmlns:keyword=\"http://www.orcid.org/ns/keyword\">\
///               <keyword:content> deep sea </keyword:content></keyword:keyword>";
/// let doc = yorc::xml::from_slice(bytes)?;
/// let yorc::model::Root::Keyword(k) = doc.root else { panic!() };
/// assert_eq!(k.content.as_deref(), Some("deep sea")); // text is trimmed
/// # Ok::<(), yorc::Error>(())
/// ```
pub fn from_slice(bytes: &[u8]) -> Result<Document> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => from_str(s),
        Err(e) => Err(Error::Utf8 {
            valid_up_to: e.valid_up_to(),
        }),
    }
}

// ---------------------------------------------------------------- the tree

/// One element: local name, attributes (local name, decoded value), child
/// elements, and its text after the last child (yorc-duckdb's text rule).
/// Only the attributes the mapping reads are kept ([`known_attr`]).
#[derive(Debug, Default)]
struct Elem {
    name: String,
    attrs: Vec<(&'static str, String)>,
    children: Vec<Elem>,
    text: String,
}

fn start_elem(e: &BytesStart<'_>) -> Elem {
    let mut el = Elem {
        name: e.local_name().into_inner().to_string(),
        ..Elem::default()
    };
    for attr in e.attributes() {
        let Ok(attr) = attr else {
            continue;
        };
        let qname: &str = attr.key.0;
        if qname == "xmlns" || qname.starts_with("xmlns:") {
            continue;
        }
        let Some(local) = known_attr(attr.key.local_name().into_inner()) else {
            continue;
        };
        let mut value = String::with_capacity(attr.value.len());
        decode_entities(&attr.value, &mut value);
        el.attrs.push((local, value));
    }
    el
}

/// The attribute names the mapping reads (by local name), as `'static`
/// strings; every other attribute is skipped without decoding its value.
fn known_attr(local: &str) -> Option<&'static str> {
    Some(match local {
        "put-code" => "put-code",
        "path" => "path",
        "visibility" => "visibility",
        "display-index" => "display-index",
        "verified" => "verified",
        "primary" => "primary",
        "transient" => "transient",
        "language-code" => "language-code",
        "currency-code" => "currency-code",
        "num-found" => "num-found",
        _ => return None,
    })
}

/// XML white space only (or empty).
fn is_xml_space(s: &str) -> bool {
    s.bytes().all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
}

/// Appends `raw` to `out`, decoding `&amp;`-style and numeric references.
fn decode_entities(raw: &str, out: &mut String) {
    let mut rest = raw;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        // a reference name is at most 11 bytes: look no further for its `;`
        // (searching the whole rest for every `&` would be quadratic)
        match after.as_bytes().iter().take(12).position(|&b| b == b';') {
            Some(semi) => {
                push_entity(&after[..semi], out);
                rest = &after[semi + 1..];
            }
            _ => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
}

/// Appends the expansion of entity `name` (without `&` and `;`).
fn push_entity(name: &str, out: &mut String) {
    let expanded = match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let code =
                if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                    u32::from_str_radix(hex, 16).ok()
                } else if let Some(dec) = name.strip_prefix('#') {
                    dec.parse::<u32>().ok()
                } else {
                    None
                };
            code.and_then(char::from_u32)
        }
    };
    match expanded {
        Some(c) => out.push(c),
        None => {
            out.push('&');
            out.push_str(name);
            out.push(';');
        }
    }
}

/// Moves a finished element into its parent (or makes it the root).
fn close(stack: &mut [Elem], root: &mut Option<Elem>, el: Elem) {
    match stack.last_mut() {
        Some(parent) => {
            parent.text.clear();
            parent.children.push(el);
        }
        None => {
            if root.is_none() {
                *root = Some(el);
            }
        }
    }
}

/// Deepest element nesting [`parse_tree`] accepts. The tree is dropped
/// recursively, so without a bound a deeply nested document (a few bytes per
/// level) would overflow the stack and abort the process.
const MAX_DEPTH: usize = 256;

fn parse_tree(xml: &str) -> Result<Elem> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().allow_dangling_amp = true;
    let mut stack: Vec<Elem> = Vec::new();
    let mut root: Option<Elem> = None;
    loop {
        let event = match reader.read_event() {
            Ok(ev) => ev,
            Err(e) => {
                return Err(Error::Xml {
                    message: e.to_string(),
                    position: reader.error_position(),
                });
            }
        };
        match event {
            Event::Start(e) => {
                if stack.len() >= MAX_DEPTH {
                    return Err(Error::Xml {
                        message: format!("elements nested deeper than {MAX_DEPTH} levels"),
                        position: reader.buffer_position(),
                    });
                }
                let el = start_elem(&e);
                if let Some(parent) = stack.last_mut() {
                    parent.text.clear();
                }
                stack.push(el);
            }
            Event::Empty(e) => {
                let el = start_elem(&e);
                if let Some(parent) = stack.last_mut() {
                    parent.text.clear();
                }
                close(&mut stack, &mut root, el);
            }
            Event::End(_) => match stack.pop() {
                Some(el) => close(&mut stack, &mut root, el),
                None => {
                    return Err(Error::Xml {
                        message: "unbalanced end tag".to_string(),
                        position: reader.buffer_position(),
                    });
                }
            },
            Event::Text(t) => {
                if let Some(top) = stack.last_mut() {
                    let t = t.into_inner();
                    // Every text value is trimmed of leading XML white space,
                    // so white space before any other text (the indentation
                    // between children) need not be kept: containers then
                    // never allocate text.
                    if !(top.text.is_empty() && is_xml_space(&t)) {
                        top.text.push_str(&t);
                    }
                }
            }
            Event::CData(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&t.into_inner());
                }
            }
            Event::GeneralRef(r) => {
                if let Some(top) = stack.last_mut() {
                    push_entity(&r.into_inner(), &mut top.text);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if let Some(open) = stack.last() {
        return Err(Error::Xml {
            message: format!("unexpected end of document inside <{}>", open.name),
            position: reader.buffer_position(),
        });
    }
    root.ok_or(Error::NoRootElement)
}

// ----------------------------------------------------------------- helpers

fn trimmed(s: &str) -> &str {
    trim_xml_space(s)
}

/// Trimmed text, `None` if empty.
fn text(e: &Elem) -> Option<String> {
    let t = trimmed(&e.text);
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Sets a text field unless it already has a value (first non-empty wins).
fn set_text(slot: &mut Option<String>, e: &Elem) {
    if slot.is_none() {
        *slot = text(e);
    }
}

/// Raw (decoded, untrimmed) value of the attribute with this local name.
fn attr_raw<'e>(e: &'e Elem, name: &str) -> Option<&'e str> {
    e.attrs
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.as_str())
}

/// Trimmed attribute value, `None` if absent or empty.
fn attr(e: &Elem, name: &str) -> Option<String> {
    let v = trimmed(attr_raw(e, name)?);
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

/// yorc-duckdb's integer rule: 1..=`max_digits` ASCII digits, nothing else.
pub(crate) fn parse_integer(s: &str, max_digits: usize) -> Option<i64> {
    if s.is_empty() || s.len() > max_digits {
        return None;
    }
    let mut v: i64 = 0;
    for b in s.bytes() {
        if !b.is_ascii_digit() {
            return None;
        }
        v = v * 10 + i64::from(b - b'0');
    }
    Some(v)
}

fn parse_i32(s: &str) -> Option<i32> {
    parse_integer(s, 9).and_then(|v| i32::try_from(v).ok())
}

/// yorc-duckdb's boolean rule: `true`/`1`, `false`/`0`.
pub(crate) fn parse_bool(s: &str) -> Option<bool> {
    match s {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

fn set_i32(slot: &mut Option<i32>, e: &Elem) {
    if slot.is_none() {
        *slot = text(e).and_then(|t| parse_i32(&t));
    }
}

fn set_bool(slot: &mut Option<bool>, e: &Elem) {
    if slot.is_none() {
        *slot = text(e).and_then(|t| parse_bool(&t));
    }
}

fn set_signed(slot: &mut Option<i64>, e: &Elem) {
    if slot.is_none() {
        *slot = text(e).and_then(|t| t.parse::<i64>().ok());
    }
}

fn set_enum<T>(slot: &mut Option<T>, e: &Elem)
where
    T: for<'s> From<&'s str>,
{
    if slot.is_none() {
        if let Some(t) = text(e) {
            *slot = Some(T::from(t.as_str()));
        }
    }
}

/// The value of an optional child, created on first use: repeated
/// occurrences of a single-valued element merge field by field.
fn child<T: Default>(slot: &mut Option<T>) -> &mut T {
    slot.get_or_insert_with(T::default)
}

fn visibility_attr(e: &Elem) -> Option<Visibility> {
    attr(e, "visibility").map(|v| Visibility::from(v.as_str()))
}

// ------------------------------------------------------------------ common

fn meta_attrs(m: &mut ItemMeta, e: &Elem) {
    m.put_code = attr(e, "put-code").and_then(|v| parse_integer(&v, 18));
    m.visibility = visibility_attr(e);
    let di = attr(e, "display-index");
    m.display_index = di.as_deref().and_then(parse_i32);
    m.display_index_text =
        di.filter(|t| m.display_index.map(|v| v.to_string()).as_deref() != Some(t.as_str()));
    m.path = attr(e, "path");
}

/// Handles the `element-summary` children; `false` if `c` is not one.
fn meta_child(m: &mut ItemMeta, c: &Elem) -> bool {
    match c.name.as_str() {
        "created-date" => set_text(&mut m.created_date, c),
        "last-modified-date" => set_text(&mut m.last_modified_date, c),
        "source" => fill_source(child(&mut m.source), c),
        _ => return false,
    }
    true
}

fn fill_identifier(id: &mut OrcidIdentifier, e: &Elem) {
    for c in &e.children {
        match c.name.as_str() {
            "uri" => set_text(&mut id.uri, c),
            "path" => set_text(&mut id.path, c),
            "host" => set_text(&mut id.host, c),
            _ => {}
        }
    }
}

fn fill_source(s: &mut Source, e: &Elem) {
    for c in &e.children {
        match c.name.as_str() {
            "source-orcid" => fill_identifier(child(&mut s.source_orcid), c),
            "source-client-id" => fill_identifier(child(&mut s.source_client_id), c),
            "source-name" => set_text(&mut s.source_name, c),
            "assertion-origin-orcid" => fill_identifier(child(&mut s.assertion_origin_orcid), c),
            "assertion-origin-client-id" => {
                fill_identifier(child(&mut s.assertion_origin_client_id), c)
            }
            "assertion-origin-name" => set_text(&mut s.assertion_origin_name, c),
            _ => {}
        }
    }
}

fn fill_date(d: &mut FuzzyDate, e: &Elem) {
    for c in &e.children {
        match c.name.as_str() {
            "year" => set_i32(&mut d.year, c),
            "month" => set_i32(&mut d.month, c),
            "day" => set_i32(&mut d.day, c),
            _ => {}
        }
    }
}

fn fill_organization(o: &mut Organization, e: &Elem) {
    for c in &e.children {
        match c.name.as_str() {
            "name" => set_text(&mut o.name, c),
            "address" => {
                let a = child(&mut o.address);
                for g in &c.children {
                    match g.name.as_str() {
                        "city" => set_text(&mut a.city, g),
                        "region" => set_text(&mut a.region, g),
                        "country" => set_text(&mut a.country, g),
                        _ => {}
                    }
                }
            }
            "disambiguated-organization" => {
                let d = child(&mut o.disambiguated_organization);
                for g in &c.children {
                    match g.name.as_str() {
                        "disambiguated-organization-identifier" => set_text(&mut d.identifier, g),
                        "disambiguation-source" => set_text(&mut d.source, g),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn organization(e: &Elem) -> Organization {
    let mut o = Organization::default();
    fill_organization(&mut o, e);
    o
}

fn external_id(e: &Elem) -> ExternalId {
    let mut x = ExternalId::default();
    meta_attrs(&mut x.meta, e);
    for c in &e.children {
        if meta_child(&mut x.meta, c) {
            continue;
        }
        match c.name.as_str() {
            "external-id-type" => set_text(&mut x.id_type, c),
            "external-id-value" => set_text(&mut x.value, c),
            "external-id-normalized" => {
                let n = child(&mut x.normalized);
                set_text(&mut n.value, c);
                if n.transient.is_none() {
                    n.transient = attr(c, "transient").and_then(|v| parse_bool(&v));
                }
            }
            "external-id-normalized-error" => {
                let n = child(&mut x.normalized_error);
                for g in &c.children {
                    match g.name.as_str() {
                        "error-code" => set_text(&mut n.error_code, g),
                        "error-message" => set_text(&mut n.error_message, g),
                        _ => {}
                    }
                }
                if n.transient.is_none() {
                    n.transient = attr(c, "transient").and_then(|v| parse_bool(&v));
                }
            }
            "external-id-url" => set_text(&mut x.url, c),
            "external-id-relationship" => set_enum(&mut x.relationship, c),
            _ => {}
        }
    }
    x
}

/// Appends the `external-id` children of an `external-ids` element.
fn external_ids(out: &mut Vec<ExternalId>, e: &Elem) {
    for c in &e.children {
        if c.name == "external-id" {
            out.push(external_id(c));
        }
    }
}

fn fill_title(t: &mut Title, e: &Elem) {
    for c in &e.children {
        match c.name.as_str() {
            "title" => set_text(&mut t.title, c),
            "subtitle" => set_text(&mut t.subtitle, c),
            "translated-title" => {
                let tt = child(&mut t.translated_title);
                set_text(&mut tt.value, c);
                if tt.language_code.is_none() {
                    tt.language_code = attr(c, "language-code");
                }
            }
            _ => {}
        }
    }
}

fn contributor(e: &Elem) -> Contributor {
    let mut x = Contributor::default();
    for c in &e.children {
        match c.name.as_str() {
            "contributor-orcid" => fill_identifier(child(&mut x.contributor_orcid), c),
            "credit-name" => set_text(&mut x.credit_name, c),
            "contributor-email" => set_text(&mut x.contributor_email, c),
            "contributor-attributes" => {
                let a = child(&mut x.attributes);
                for g in &c.children {
                    match g.name.as_str() {
                        "contributor-sequence" => set_enum(&mut a.sequence, g),
                        "contributor-role" => set_enum(&mut a.role, g),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    x
}

/// Appends the `contributor` children of a `contributors` element.
fn contributors(out: &mut Vec<Contributor>, e: &Elem) {
    for c in &e.children {
        if c.name == "contributor" {
            out.push(contributor(c));
        }
    }
}

// ------------------------------------------------------------------- works

fn work_summary(e: &Elem) -> WorkSummary {
    let mut w = WorkSummary::default();
    meta_attrs(&mut w.meta, e);
    for c in &e.children {
        if meta_child(&mut w.meta, c) {
            continue;
        }
        match c.name.as_str() {
            "title" => fill_title(child(&mut w.title), c),
            "external-ids" => external_ids(&mut w.external_ids, c),
            "url" => set_text(&mut w.url, c),
            "type" => set_enum(&mut w.work_type, c),
            "publication-date" => fill_date(child(&mut w.publication_date), c),
            "journal-title" => set_text(&mut w.journal_title, c),
            _ => {}
        }
    }
    w
}

fn work(e: &Elem) -> Work {
    let mut w = Work::default();
    meta_attrs(&mut w.meta, e);
    for c in &e.children {
        if meta_child(&mut w.meta, c) {
            continue;
        }
        match c.name.as_str() {
            "title" => fill_title(child(&mut w.title), c),
            "journal-title" => set_text(&mut w.journal_title, c),
            "short-description" => set_text(&mut w.short_description, c),
            "citation" => {
                let ci = child(&mut w.citation);
                for g in &c.children {
                    match g.name.as_str() {
                        "citation-type" => set_enum(&mut ci.citation_type, g),
                        "citation-value" => set_text(&mut ci.citation_value, g),
                        _ => {}
                    }
                }
            }
            "type" => set_enum(&mut w.work_type, c),
            "publication-date" => fill_date(child(&mut w.publication_date), c),
            "external-ids" => external_ids(&mut w.external_ids, c),
            "url" => set_text(&mut w.url, c),
            "contributors" => contributors(&mut w.contributors, c),
            "language-code" => set_text(&mut w.language_code, c),
            "country" => {
                let k = child(&mut w.country);
                set_text(&mut k.value, c);
                if k.visibility.is_none() {
                    k.visibility = visibility_attr(c);
                }
            }
            _ => {}
        }
    }
    w
}

fn fill_works(w: &mut Works, e: &Elem) {
    if w.path.is_none() {
        w.path = attr(e, "path");
    }
    for c in &e.children {
        match c.name.as_str() {
            "last-modified-date" => set_text(&mut w.last_modified_date, c),
            "group" => {
                let mut g = WorkGroup::default();
                for i in &c.children {
                    match i.name.as_str() {
                        "last-modified-date" => set_text(&mut g.last_modified_date, i),
                        "external-ids" => external_ids(&mut g.external_ids, i),
                        "work-summary" => g.work_summaries.push(work_summary(i)),
                        _ => {}
                    }
                }
                w.groups.push(g);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------- fundings

fn funding_summary(e: &Elem) -> FundingSummary {
    let mut f = FundingSummary::default();
    meta_attrs(&mut f.meta, e);
    for c in &e.children {
        if meta_child(&mut f.meta, c) {
            continue;
        }
        match c.name.as_str() {
            "title" => fill_title(child(&mut f.title), c),
            "external-ids" => external_ids(&mut f.external_ids, c),
            "url" => set_text(&mut f.url, c),
            "type" => set_enum(&mut f.funding_type, c),
            "start-date" => fill_date(child(&mut f.start_date), c),
            "end-date" => fill_date(child(&mut f.end_date), c),
            "organization" => fill_organization(child(&mut f.organization), c),
            _ => {}
        }
    }
    f
}

fn funding(e: &Elem) -> Funding {
    let mut f = Funding::default();
    meta_attrs(&mut f.meta, e);
    for c in &e.children {
        if meta_child(&mut f.meta, c) {
            continue;
        }
        match c.name.as_str() {
            "type" => set_enum(&mut f.funding_type, c),
            "organization-defined-type" => set_text(&mut f.organization_defined_type, c),
            "title" => fill_title(child(&mut f.title), c),
            "short-description" => set_text(&mut f.short_description, c),
            "amount" => {
                let a = child(&mut f.amount);
                set_text(&mut a.value, c);
                if a.currency_code.is_none() {
                    a.currency_code = attr(c, "currency-code");
                }
            }
            "url" => set_text(&mut f.url, c),
            "start-date" => fill_date(child(&mut f.start_date), c),
            "end-date" => fill_date(child(&mut f.end_date), c),
            "external-ids" => external_ids(&mut f.external_ids, c),
            "contributors" => contributors(&mut f.contributors, c),
            "organization" => fill_organization(child(&mut f.organization), c),
            _ => {}
        }
    }
    f
}

fn fill_fundings(f: &mut Fundings, e: &Elem) {
    if f.path.is_none() {
        f.path = attr(e, "path");
    }
    for c in &e.children {
        match c.name.as_str() {
            "last-modified-date" => set_text(&mut f.last_modified_date, c),
            "group" => {
                let mut g = FundingGroup::default();
                for i in &c.children {
                    match i.name.as_str() {
                        "last-modified-date" => set_text(&mut g.last_modified_date, i),
                        "external-ids" => external_ids(&mut g.external_ids, i),
                        "funding-summary" => g.funding_summaries.push(funding_summary(i)),
                        _ => {}
                    }
                }
                f.groups.push(g);
            }
            _ => {}
        }
    }
}

// ------------------------------------------------------------ affiliations

fn affiliation(kind: AffiliationKind, e: &Elem) -> Affiliation {
    let mut a = Affiliation::new(kind);
    meta_attrs(&mut a.meta, e);
    for c in &e.children {
        if meta_child(&mut a.meta, c) {
            continue;
        }
        match c.name.as_str() {
            "department-name" => set_text(&mut a.department_name, c),
            "role-title" => set_text(&mut a.role_title, c),
            "start-date" => fill_date(child(&mut a.start_date), c),
            "end-date" => fill_date(child(&mut a.end_date), c),
            "organization" => fill_organization(child(&mut a.organization), c),
            "url" => set_text(&mut a.url, c),
            "external-ids" => external_ids(&mut a.external_ids, c),
            _ => {}
        }
    }
    a
}

fn fill_affiliations(list: &mut Affiliations, e: &Elem) {
    if list.path.is_none() {
        list.path = attr(e, "path");
    }
    for c in &e.children {
        match c.name.as_str() {
            "last-modified-date" => set_text(&mut list.last_modified_date, c),
            "affiliation-group" => {
                let mut g = AffiliationGroup::default();
                for i in &c.children {
                    match i.name.as_str() {
                        "last-modified-date" => set_text(&mut g.last_modified_date, i),
                        "external-ids" => external_ids(&mut g.external_ids, i),
                        other => {
                            if let Some(kind) = AffiliationKind::from_summary_element(other) {
                                g.summaries.push(affiliation(kind, i));
                            }
                        }
                    }
                }
                list.groups.push(g);
            }
            _ => {}
        }
    }
}

// ------------------------------------------------------------ peer reviews

fn peer_review_summary(e: &Elem) -> PeerReviewSummary {
    let mut p = PeerReviewSummary::default();
    meta_attrs(&mut p.meta, e);
    for c in &e.children {
        if meta_child(&mut p.meta, c) {
            continue;
        }
        match c.name.as_str() {
            "reviewer-role" => set_enum(&mut p.reviewer_role, c),
            "external-ids" => external_ids(&mut p.external_ids, c),
            "review-url" => set_text(&mut p.review_url, c),
            "review-type" => set_enum(&mut p.review_type, c),
            "completion-date" => fill_date(child(&mut p.completion_date), c),
            "review-group-id" => set_text(&mut p.review_group_id, c),
            "convening-organization" => fill_organization(child(&mut p.convening_organization), c),
            _ => {}
        }
    }
    p
}

fn peer_review(e: &Elem) -> PeerReview {
    let mut p = PeerReview::default();
    meta_attrs(&mut p.meta, e);
    for c in &e.children {
        if meta_child(&mut p.meta, c) {
            continue;
        }
        match c.name.as_str() {
            "reviewer-role" => set_enum(&mut p.reviewer_role, c),
            "review-identifiers" => external_ids(&mut p.review_identifiers, c),
            "review-url" => set_text(&mut p.review_url, c),
            "review-type" => set_enum(&mut p.review_type, c),
            "review-completion-date" => fill_date(child(&mut p.review_completion_date), c),
            "review-group-id" => set_text(&mut p.review_group_id, c),
            "subject-external-identifier" => {
                if p.subject_external_identifier.is_none() {
                    p.subject_external_identifier = Some(external_id(c));
                }
            }
            "subject-container-name" => set_text(&mut p.subject_container_name, c),
            "subject-type" => set_text(&mut p.subject_type, c),
            "subject-name" => fill_title(child(&mut p.subject_name), c),
            "subject-url" => set_text(&mut p.subject_url, c),
            "convening-organization" => fill_organization(child(&mut p.convening_organization), c),
            _ => {}
        }
    }
    p
}

fn fill_peer_reviews(p: &mut PeerReviews, e: &Elem) {
    if p.path.is_none() {
        p.path = attr(e, "path");
    }
    for c in &e.children {
        match c.name.as_str() {
            "last-modified-date" => set_text(&mut p.last_modified_date, c),
            "group" => {
                let mut g = PeerReviewGroup::default();
                for i in &c.children {
                    match i.name.as_str() {
                        "last-modified-date" => set_text(&mut g.last_modified_date, i),
                        "external-ids" => external_ids(&mut g.external_ids, i),
                        "peer-review-group" => {
                            let mut d = PeerReviewDuplicates::default();
                            for j in &i.children {
                                match j.name.as_str() {
                                    "last-modified-date" => set_text(&mut d.last_modified_date, j),
                                    "external-ids" => external_ids(&mut d.external_ids, j),
                                    "peer-review-summary" => {
                                        d.summaries.push(peer_review_summary(j))
                                    }
                                    _ => {}
                                }
                            }
                            g.duplicates.push(d);
                        }
                        _ => {}
                    }
                }
                p.groups.push(g);
            }
            _ => {}
        }
    }
}

// ------------------------------------------------------ research resources

/// Appends the `organization` children of a `hosts` element.
fn hosts(out: &mut Vec<Organization>, e: &Elem) {
    for c in &e.children {
        if c.name == "organization" {
            out.push(organization(c));
        }
    }
}

fn fill_proposal(p: &mut Proposal, e: &Elem) {
    for c in &e.children {
        match c.name.as_str() {
            "title" => fill_title(child(&mut p.title), c),
            "hosts" => hosts(&mut p.hosts, c),
            "external-ids" => external_ids(&mut p.external_ids, c),
            "start-date" => fill_date(child(&mut p.start_date), c),
            "end-date" => fill_date(child(&mut p.end_date), c),
            "url" => set_text(&mut p.url, c),
            _ => {}
        }
    }
}

fn resource_item(e: &Elem) -> ResourceItem {
    let mut r = ResourceItem::default();
    for c in &e.children {
        match c.name.as_str() {
            "resource-name" => set_text(&mut r.resource_name, c),
            "resource-type" => set_enum(&mut r.resource_type, c),
            "hosts" => hosts(&mut r.hosts, c),
            "external-ids" => external_ids(&mut r.external_ids, c),
            "url" => set_text(&mut r.url, c),
            _ => {}
        }
    }
    r
}

fn research_resource_summary(e: &Elem) -> ResearchResourceSummary {
    let mut r = ResearchResourceSummary::default();
    meta_attrs(&mut r.meta, e);
    for c in &e.children {
        if meta_child(&mut r.meta, c) {
            continue;
        }
        if c.name == "proposal" {
            fill_proposal(child(&mut r.proposal), c);
        }
    }
    r
}

fn research_resource(e: &Elem) -> ResearchResource {
    let mut r = ResearchResource::default();
    meta_attrs(&mut r.meta, e);
    for c in &e.children {
        if meta_child(&mut r.meta, c) {
            continue;
        }
        match c.name.as_str() {
            "proposal" => fill_proposal(child(&mut r.proposal), c),
            "resource-items" => {
                for i in &c.children {
                    if i.name == "resource-item" {
                        r.resource_items.push(resource_item(i));
                    }
                }
            }
            _ => {}
        }
    }
    r
}

fn fill_research_resources(r: &mut ResearchResources, e: &Elem) {
    if r.path.is_none() {
        r.path = attr(e, "path");
    }
    for c in &e.children {
        match c.name.as_str() {
            "last-modified-date" => set_text(&mut r.last_modified_date, c),
            "group" => {
                let mut g = ResearchResourceGroup::default();
                for i in &c.children {
                    match i.name.as_str() {
                        "last-modified-date" => set_text(&mut g.last_modified_date, i),
                        "external-ids" => external_ids(&mut g.external_ids, i),
                        "research-resource-summary" => {
                            g.summaries.push(research_resource_summary(i))
                        }
                        _ => {}
                    }
                }
                r.groups.push(g);
            }
            _ => {}
        }
    }
}

// ------------------------------------------------------ activities summary

fn fill_activities(a: &mut ActivitiesSummary, e: &Elem) {
    if a.path.is_none() {
        a.path = attr(e, "path");
    }
    for c in &e.children {
        match c.name.as_str() {
            "last-modified-date" => set_text(&mut a.last_modified_date, c),
            "fundings" => fill_fundings(child(&mut a.fundings), c),
            "peer-reviews" => fill_peer_reviews(child(&mut a.peer_reviews), c),
            "research-resources" => fill_research_resources(child(&mut a.research_resources), c),
            "works" => fill_works(child(&mut a.works), c),
            other => {
                if let Some(kind) = AffiliationKind::from_container_element(other) {
                    let slot = a.affiliations_slot(kind);
                    let list = slot.get_or_insert_with(|| Affiliations::new(kind));
                    fill_affiliations(list, c);
                }
            }
        }
    }
}

// ------------------------------------------------------------------ person

fn fill_name(n: &mut Name, e: &Elem) {
    if n.visibility.is_none() {
        n.visibility = visibility_attr(e);
    }
    if n.path.is_none() {
        n.path = attr(e, "path");
    }
    for c in &e.children {
        match c.name.as_str() {
            "created-date" => set_text(&mut n.created_date, c),
            "last-modified-date" => set_text(&mut n.last_modified_date, c),
            "given-names" => set_text(&mut n.given_names, c),
            "family-name" => set_text(&mut n.family_name, c),
            "credit-name" => set_text(&mut n.credit_name, c),
            _ => {}
        }
    }
}

fn fill_biography(b: &mut Biography, e: &Elem) {
    if b.visibility.is_none() {
        b.visibility = visibility_attr(e);
    }
    if b.path.is_none() {
        b.path = attr(e, "path");
    }
    for c in &e.children {
        match c.name.as_str() {
            "created-date" => set_text(&mut b.created_date, c),
            "last-modified-date" => set_text(&mut b.last_modified_date, c),
            "content" => set_text(&mut b.content, c),
            _ => {}
        }
    }
}

fn other_name(e: &Elem) -> OtherName {
    let mut x = OtherName::default();
    meta_attrs(&mut x.meta, e);
    for c in &e.children {
        if !meta_child(&mut x.meta, c) && c.name == "content" {
            set_text(&mut x.content, c);
        }
    }
    x
}

fn researcher_url(e: &Elem) -> ResearcherUrl {
    let mut x = ResearcherUrl::default();
    meta_attrs(&mut x.meta, e);
    for c in &e.children {
        if meta_child(&mut x.meta, c) {
            continue;
        }
        match c.name.as_str() {
            "url-name" => set_text(&mut x.url_name, c),
            "url" => set_text(&mut x.url, c),
            _ => {}
        }
    }
    x
}

fn email(e: &Elem) -> Email {
    let mut x = Email::default();
    meta_attrs(&mut x.meta, e);
    x.verified = attr(e, "verified").and_then(|v| parse_bool(&v));
    x.primary = attr(e, "primary").and_then(|v| parse_bool(&v));
    for c in &e.children {
        if !meta_child(&mut x.meta, c) && c.name == "email" {
            set_text(&mut x.email, c);
        }
    }
    x
}

fn address(e: &Elem) -> Address {
    let mut x = Address::default();
    meta_attrs(&mut x.meta, e);
    for c in &e.children {
        if !meta_child(&mut x.meta, c) && c.name == "country" {
            set_text(&mut x.country, c);
        }
    }
    x
}

fn keyword(e: &Elem) -> Keyword {
    let mut x = Keyword::default();
    meta_attrs(&mut x.meta, e);
    for c in &e.children {
        if !meta_child(&mut x.meta, c) && c.name == "content" {
            set_text(&mut x.content, c);
        }
    }
    x
}

/// Fills a person item container (`other-names`, `emails`, ...).
fn fill_list<T>(list: &mut ItemList<T>, e: &Elem, item: &str, parse: fn(&Elem) -> T) {
    if list.path.is_none() {
        list.path = attr(e, "path");
    }
    for c in &e.children {
        if c.name == "last-modified-date" {
            set_text(&mut list.last_modified_date, c);
        } else if c.name == item {
            list.items.push(parse(c));
        }
    }
}

fn fill_person(p: &mut Person, e: &Elem) {
    if p.path.is_none() {
        p.path = attr(e, "path");
    }
    for c in &e.children {
        match c.name.as_str() {
            "last-modified-date" => set_text(&mut p.last_modified_date, c),
            "name" => fill_name(child(&mut p.name), c),
            "other-names" => fill_list(child(&mut p.other_names), c, "other-name", other_name),
            "biography" => fill_biography(child(&mut p.biography), c),
            "researcher-urls" => fill_list(
                child(&mut p.researcher_urls),
                c,
                "researcher-url",
                researcher_url,
            ),
            "emails" => fill_list(child(&mut p.emails), c, "email", email),
            "addresses" => fill_list(child(&mut p.addresses), c, "address", address),
            "keywords" => fill_list(child(&mut p.keywords), c, "keyword", keyword),
            "external-identifiers" => fill_list(
                child(&mut p.external_identifiers),
                c,
                "external-identifier",
                external_id,
            ),
            _ => {}
        }
    }
}

fn fill_personal_details(p: &mut PersonalDetails, e: &Elem) {
    if p.path.is_none() {
        p.path = attr(e, "path");
    }
    for c in &e.children {
        match c.name.as_str() {
            "last-modified-date" => set_text(&mut p.last_modified_date, c),
            "name" => fill_name(child(&mut p.name), c),
            "other-names" => fill_list(child(&mut p.other_names), c, "other-name", other_name),
            "biography" => fill_biography(child(&mut p.biography), c),
            _ => {}
        }
    }
}

// ----------------------------------------------------------------- record

fn fill_history(h: &mut History, e: &Elem) {
    if h.visibility.is_none() {
        h.visibility = visibility_attr(e);
    }
    for c in &e.children {
        match c.name.as_str() {
            "creation-method" => set_enum(&mut h.creation_method, c),
            "completion-date" => set_text(&mut h.completion_date, c),
            "submission-date" => set_text(&mut h.submission_date, c),
            "last-modified-date" => set_text(&mut h.last_modified_date, c),
            "claimed" => set_bool(&mut h.claimed, c),
            "source" => fill_source(child(&mut h.source), c),
            "deactivation-date" => set_text(&mut h.deactivation_date, c),
            "verified-email" => set_bool(&mut h.verified_email, c),
            "verified-primary-email" => set_bool(&mut h.verified_primary_email, c),
            _ => {}
        }
    }
}

fn fill_preferences(p: &mut Preferences, e: &Elem) {
    for c in &e.children {
        if c.name == "locale" {
            set_text(&mut p.locale, c);
        }
    }
}

fn fill_record(r: &mut Record, e: &Elem) {
    if r.path.is_none() {
        r.path = attr(e, "path");
    }
    for c in &e.children {
        match c.name.as_str() {
            "orcid-identifier" => fill_identifier(child(&mut r.orcid_identifier), c),
            "preferences" => fill_preferences(child(&mut r.preferences), c),
            "history" => fill_history(child(&mut r.history), c),
            "person" => fill_person(child(&mut r.person), c),
            "activities-summary" => fill_activities(child(&mut r.activities_summary), c),
            _ => {}
        }
    }
}

fn orcid_error(e: &Elem) -> OrcidError {
    let mut x = OrcidError::default();
    for c in &e.children {
        match c.name.as_str() {
            "response-code" => set_signed(&mut x.response_code, c),
            "developer-message" => set_text(&mut x.developer_message, c),
            "user-message" => set_text(&mut x.user_message, c),
            "error-code" => set_signed(&mut x.error_code, c),
            "more-info" => set_text(&mut x.more_info, c),
            _ => {}
        }
    }
    x
}

fn bulk(e: &Elem) -> Bulk {
    let mut b = Bulk::default();
    for c in &e.children {
        match c.name.as_str() {
            "work" => b.items.push(BulkItem::Work(work(c))),
            "error" => b.items.push(BulkItem::Error(orcid_error(c))),
            _ => {}
        }
    }
    b
}

fn search(e: &Elem) -> Search {
    let mut s = Search {
        num_found: attr(e, "num-found").and_then(|v| v.parse::<i64>().ok()),
        results: Vec::new(),
    };
    for c in &e.children {
        if c.name != "result" {
            continue;
        }
        for g in &c.children {
            if g.name == "orcid-identifier" {
                let mut id = OrcidIdentifier::default();
                fill_identifier(&mut id, g);
                s.results.push(id);
            }
        }
    }
    s
}

fn expanded_search(e: &Elem) -> ExpandedSearch {
    let mut s = ExpandedSearch {
        num_found: attr(e, "num-found").and_then(|v| v.parse::<i64>().ok()),
        results: Vec::new(),
    };
    for c in &e.children {
        if c.name != "expanded-result" {
            continue;
        }
        let mut r = ExpandedResult::default();
        for g in &c.children {
            match g.name.as_str() {
                "orcid-id" => set_text(&mut r.orcid_id, g),
                "given-names" => set_text(&mut r.given_names, g),
                "family-names" => set_text(&mut r.family_names, g),
                "credit-name" => set_text(&mut r.credit_name, g),
                "other-name" => r.other_names.extend(text(g)),
                "email" => r.emails.extend(text(g)),
                "institution-name" => r.institution_names.extend(text(g)),
                _ => {}
            }
        }
        s.results.push(r);
    }
    s
}

/// `group-id-record`: `common:source`, or common-2.1's inline source fields
/// (the XSD's `source-type` extension), into `source`; first non-empty value
/// per field, in document order.
fn group_id_record(e: &Elem) -> GroupIdRecord {
    let mut g = GroupIdRecord {
        put_code: attr(e, "put-code").and_then(|v| parse_integer(&v, 18)),
        ..GroupIdRecord::default()
    };
    for c in &e.children {
        match c.name.as_str() {
            "created-date" => set_text(&mut g.created_date, c),
            "last-modified-date" => set_text(&mut g.last_modified_date, c),
            "name" => set_text(&mut g.name, c),
            "group-id" => set_text(&mut g.group_id, c),
            "description" => set_text(&mut g.description, c),
            "type" => set_enum(&mut g.group_type, c),
            "source" => fill_source(child(&mut g.source), c),
            "source-orcid" => fill_identifier(child(&mut child(&mut g.source).source_orcid), c),
            "source-client-id" => {
                fill_identifier(child(&mut child(&mut g.source).source_client_id), c)
            }
            "source-name" => {
                if let Some(t) = text(c) {
                    let s = child(&mut g.source);
                    if s.source_name.is_none() {
                        s.source_name = Some(t);
                    }
                }
            }
            _ => {}
        }
    }
    g
}

/// `group-id-records`: `total`, `page` and `page-size` are the first
/// non-empty text of each, as integers (1 to 9 ASCII digits, else `None`).
fn group_id_records(e: &Elem) -> GroupIdRecords {
    let mut p = GroupIdRecords::default();
    let (mut total, mut page, mut page_size) = (None, None, None);
    for c in &e.children {
        match c.name.as_str() {
            "last-modified-date" => set_text(&mut p.last_modified_date, c),
            "total" => set_text(&mut total, c),
            "page" => set_text(&mut page, c),
            "page-size" => set_text(&mut page_size, c),
            "group-id-record" => p.records.push(group_id_record(c)),
            _ => {}
        }
    }
    let digits = |t: Option<String>| t.and_then(|t| parse_integer(&t, 9));
    p.total = digits(total);
    p.page = digits(page);
    p.page_size = digits(page_size);
    p
}

/// Fills a fresh `T` with `fill`.
fn build<T: Default>(e: &Elem, fill: fn(&mut T, &Elem)) -> T {
    let mut v = T::default();
    fill(&mut v, e);
    v
}

fn document(root: &Elem) -> Document {
    let body = match root.name.as_str() {
        "record" => Root::Record(build(root, fill_record)),
        "activities-summary" => Root::ActivitiesSummary(build(root, fill_activities)),
        "works" => Root::Works(build(root, fill_works)),
        "work" => Root::Work(work(root)),
        "work-summary" => Root::WorkSummary(work_summary(root)),
        "fundings" => Root::Fundings(build(root, fill_fundings)),
        "funding" => Root::Funding(funding(root)),
        "funding-summary" => Root::FundingSummary(funding_summary(root)),
        "peer-reviews" => Root::PeerReviews(build(root, fill_peer_reviews)),
        "peer-review" => Root::PeerReview(peer_review(root)),
        "peer-review-summary" => Root::PeerReviewSummary(peer_review_summary(root)),
        "research-resources" => Root::ResearchResources(build(root, fill_research_resources)),
        "research-resource" => Root::ResearchResource(research_resource(root)),
        "research-resource-summary" => {
            Root::ResearchResourceSummary(research_resource_summary(root))
        }
        "person" => Root::Person(build(root, fill_person)),
        "personal-details" => Root::PersonalDetails(build(root, fill_personal_details)),
        "name" => Root::Name(build(root, fill_name)),
        "biography" => Root::Biography(build(root, fill_biography)),
        "credit-name" => Root::CreditName(text(root)),
        "other-names" => {
            let mut l = OtherNames::default();
            fill_list(&mut l, root, "other-name", other_name);
            Root::OtherNames(l)
        }
        "other-name" => Root::OtherName(other_name(root)),
        "researcher-urls" => {
            let mut l = ResearcherUrls::default();
            fill_list(&mut l, root, "researcher-url", researcher_url);
            Root::ResearcherUrls(l)
        }
        "researcher-url" => Root::ResearcherUrl(researcher_url(root)),
        "emails" => {
            let mut l = Emails::default();
            fill_list(&mut l, root, "email", email);
            Root::Emails(l)
        }
        "email" => Root::Email(email(root)),
        "addresses" => {
            let mut l = Addresses::default();
            fill_list(&mut l, root, "address", address);
            Root::Addresses(l)
        }
        "address" => Root::Address(address(root)),
        "keywords" => {
            let mut l = Keywords::default();
            fill_list(&mut l, root, "keyword", keyword);
            Root::Keywords(l)
        }
        "keyword" => Root::Keyword(keyword(root)),
        "external-identifiers" => {
            let mut l = ExternalIdentifiers::default();
            fill_list(&mut l, root, "external-identifier", external_id);
            Root::ExternalIdentifiers(l)
        }
        "external-identifier" => Root::ExternalIdentifier(external_id(root)),
        "history" => Root::History(build(root, fill_history)),
        "preferences" => Root::Preferences(build(root, fill_preferences)),
        "bulk" => Root::Bulk(bulk(root)),
        "error" => Root::Error(orcid_error(root)),
        "search" => Root::Search(search(root)),
        "expanded-search" => Root::ExpandedSearch(expanded_search(root)),
        "group-id-record" => Root::GroupIdRecord(group_id_record(root)),
        "group-id-records" => Root::GroupIdRecords(group_id_records(root)),
        other => {
            if let Some(kind) = AffiliationKind::from_container_element(other) {
                let mut list = Affiliations::new(kind);
                fill_affiliations(&mut list, root);
                Root::Affiliations(list)
            } else if let Some(kind) = AffiliationKind::from_element(other) {
                Root::Affiliation(affiliation(kind, root))
            } else if let Some(kind) = AffiliationKind::from_summary_element(other) {
                Root::AffiliationSummary(affiliation(kind, root))
            } else {
                Root::Unknown(other.to_string())
            }
        }
    };
    Document {
        root_name: root.name.clone(),
        root_path: attr_raw(root, "path").map(|p| p.to_string()),
        root: body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entities_cdata_and_text_rule() {
        let doc = from_str(
            r#"<?xml version="1.0"?>
<!-- comment -->
<work:work xmlns:work="w" xmlns:common="c" put-code=" 7 " path="/0000-0002-1825-0097/work/7">
  <work:title><common:title>Fish &amp; Chips &#x2014; &#233;t&#xE9; &lt;3 &foo;</common:title>
    <common:translated-title language-code="fr"> Poisson </common:translated-title></work:title>
  <work:journal-title><![CDATA[A <raw> & journal]]></work:journal-title>
  <work:journal-title>second</work:journal-title>
  <work:type>book</work:type>
  <common:url/>
  <common:publication-date><common:year>1999</common:year><common:month>xx</common:month><common:day>007</common:day></common:publication-date>
</work:work>"#,
        )
        .unwrap();
        assert_eq!(doc.root_name, "work");
        assert_eq!(doc.orcid(), Some("0000-0002-1825-0097"));
        let Root::Work(w) = &doc.root else {
            panic!("not a work: {:?}", doc.root);
        };
        let title = w.title.as_ref().unwrap();
        assert_eq!(
            title.title.as_deref(),
            Some("Fish & Chips \u{2014} \u{e9}t\u{e9} <3 &foo;")
        );
        let tt = title.translated_title.as_ref().unwrap();
        assert_eq!(tt.value.as_deref(), Some("Poisson"));
        assert_eq!(tt.language_code.as_deref(), Some("fr"));
        assert_eq!(w.journal_title.as_deref(), Some("A <raw> & journal"));
        assert_eq!(w.work_type, Some(WorkType::Book));
        assert_eq!(w.url, None);
        assert_eq!(w.meta.put_code, Some(7));
        let d = w.publication_date.unwrap();
        assert_eq!((d.year, d.month, d.day), (Some(1999), None, Some(7)));
    }

    #[test]
    fn mixed_content_keeps_text_after_last_child() {
        let doc = from_str("<credit-name>a<x/>b</credit-name>").unwrap();
        assert_eq!(doc.root, Root::CreditName(Some("b".to_string())));
    }

    #[test]
    fn leading_white_space_is_trimmed_across_references() {
        for (xml, want) in [
            ("<credit-name>  &amp; b </credit-name>", Some("& b")),
            ("<credit-name> \n &#32;&#10;b</credit-name>", Some("b")),
            (
                "<credit-name>\n  <![CDATA[ c ]]>\n</credit-name>",
                Some("c"),
            ),
            ("<credit-name>\n  <x/>\n  </credit-name>", None),
            ("<credit-name> &#x20; </credit-name>", None),
        ] {
            let doc = from_str(xml).unwrap();
            assert_eq!(
                doc.root,
                Root::CreditName(want.map(str::to_string)),
                "{xml}"
            );
        }
    }

    #[test]
    fn unknown_values_and_elements() {
        let doc = from_str(
            "<work-summary visibility=\"friends\"><type>new-type</type><new-element>x</new-element></work-summary>",
        )
        .unwrap();
        let Root::WorkSummary(w) = &doc.root else {
            panic!("not a work summary");
        };
        assert_eq!(w.work_type, Some(WorkType::Other("new-type".to_string())));
        assert_eq!(w.work_type.as_ref().unwrap().as_str(), "new-type");
        assert_eq!(
            w.meta.visibility,
            Some(Visibility::Other("friends".to_string()))
        );
        let doc = from_str("<something-else/>").unwrap();
        assert_eq!(doc.root, Root::Unknown("something-else".to_string()));
    }

    #[test]
    fn malformed_documents_are_errors() {
        assert!(matches!(from_str("<a><b></a>"), Err(Error::Xml { .. })));
        assert!(matches!(from_str("<a><b>"), Err(Error::Xml { .. })));
        assert_eq!(from_str("  "), Err(Error::NoRootElement));
        assert_eq!(
            from_slice(b"<a>\xff</a>"),
            Err(Error::Utf8 { valid_up_to: 3 })
        );
    }

    #[test]
    fn nesting_is_bounded() {
        // at the limit: read
        let ok = "<a>".repeat(MAX_DEPTH) + &"</a>".repeat(MAX_DEPTH);
        assert!(from_str(&ok).is_ok());
        // one level deeper: an error, not a stack overflow
        let deep = "<a>".repeat(MAX_DEPTH + 1) + &"</a>".repeat(MAX_DEPTH + 1);
        assert!(matches!(from_str(&deep), Err(Error::Xml { .. })));
        // far deeper (dropping such a tree recursively would overflow the stack)
        let n = 1_000_000;
        let bomb = "<a>".repeat(n) + &"</a>".repeat(n);
        assert!(matches!(from_str(&bomb), Err(Error::Xml { .. })));
    }

    #[test]
    fn attribute_references_are_linear() {
        // every `&` scans at most 12 bytes for its `;`
        let amps = "&".repeat(1 << 20);
        let xml = format!("<work-summary put-code=\"{amps};\"/>");
        let doc = from_str(&xml).unwrap();
        assert_eq!(doc.root_name, "work-summary");
        let mut out = String::new();
        decode_entities("a &amp; b &#x41; &toolongtobeanentity; &", &mut out);
        assert_eq!(out, "a & b A &toolongtobeanentity; &");
    }

    #[test]
    fn integer_and_boolean_rules() {
        assert_eq!(parse_integer("0042", 9), Some(42));
        assert_eq!(parse_integer("4x", 9), None);
        assert_eq!(parse_integer("", 9), None);
        assert_eq!(parse_integer("1234567890", 9), None);
        assert_eq!(parse_integer("-1", 9), None);
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("0"), Some(false));
        assert_eq!(parse_bool("yes"), None);
    }
}
