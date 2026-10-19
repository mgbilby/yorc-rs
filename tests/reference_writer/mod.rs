//! The writer yorc-rs 0.1.0 shipped before the streaming rewrite: it
//! builds a tree of owned nodes, collects the namespaces, then serialises.
//! Kept (test-only) as the reference the streaming writer in
//! `src/xml/write.rs` must match byte for byte, errors included; see
//! `writer_matches_reference_*` in `write_roundtrip.rs` and
//! `schema_coverage.rs`.

#![allow(dead_code)]

use std::collections::BTreeSet;

use yorc::model::*;
use yorc::xml::Ns;
use yorc::{Error, Result};

// --------------------------------------------------------------- the tree

struct Node {
    ns: Ns,
    name: &'static str,
    attrs: Vec<(&'static str, String)>,
    text: Option<String>,
    kids: Vec<Node>,
    /// Set when the model holds something the XSD has no place for here;
    /// serialising then fails with [`Error::Unwritable`].
    unwritable: Option<&'static str>,
}

impl Node {
    fn new(ns: Ns, name: &'static str) -> Node {
        Node {
            ns,
            name,
            attrs: Vec::new(),
            text: None,
            kids: Vec::new(),
            unwritable: None,
        }
    }

    fn text(ns: Ns, name: &'static str, text: impl Into<String>) -> Node {
        let mut n = Node::new(ns, name);
        n.text = Some(text.into());
        n
    }

    fn attr(&mut self, name: &'static str, value: Option<String>) {
        if let Some(v) = value {
            self.attrs.push((name, v));
        }
    }

    fn kid(&mut self, n: Node) {
        self.kids.push(n);
    }

    fn opt(&mut self, n: Option<Node>) {
        if let Some(n) = n {
            self.kids.push(n);
        }
    }

    /// A text leaf if the value is present.
    fn leaf(&mut self, ns: Ns, name: &'static str, value: &Option<String>) {
        if let Some(v) = value {
            self.kids.push(Node::text(ns, name, v.clone()));
        }
    }

    fn collect_ns(&self, out: &mut BTreeSet<Ns>) {
        out.insert(self.ns);
        for k in &self.kids {
            k.collect_ns(out);
        }
    }
}

fn bool_text(b: bool) -> String {
    (if b { "true" } else { "false" }).to_string()
}

// ------------------------------------------------------------- serialise

fn qname(n: &Node) -> String {
    format!("{}:{}", n.ns.prefix(), n.name)
}

fn check_chars(s: &str, path: &str) -> Result<()> {
    for c in s.chars() {
        let ok =
            matches!(c, '\t' | '\n' | '\r') || (c >= ' ' && c != '\u{FFFE}' && c != '\u{FFFF}');
        if !ok {
            return Err(Error::Unwritable {
                path: path.to_string(),
                reason: format!("character U+{:04X} is not allowed in XML 1.0", u32::from(c)),
            });
        }
    }
    Ok(())
}

fn escape_text(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#13;"),
            c => out.push(c),
        }
    }
}

fn escape_attr(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("&#9;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            c => out.push(c),
        }
    }
}

fn write_node(
    n: &Node,
    depth: usize,
    path: &str,
    root_ns: Option<&BTreeSet<Ns>>,
    out: &mut String,
) -> Result<()> {
    let path = if path.is_empty() {
        n.name.to_string()
    } else {
        format!("{path}/{}", n.name)
    };
    if let Some(reason) = n.unwritable {
        return Err(Error::Unwritable {
            path,
            reason: reason.to_string(),
        });
    }
    for _ in 0..depth {
        out.push_str("  ");
    }
    out.push('<');
    out.push_str(&qname(n));
    if let Some(nss) = root_ns {
        for ns in nss {
            out.push_str(" xmlns:");
            out.push_str(ns.prefix());
            out.push_str("=\"");
            out.push_str(&ns.uri());
            out.push('"');
        }
    }
    for (k, v) in &n.attrs {
        check_chars(v, &format!("{path}/@{k}"))?;
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        escape_attr(v, out);
        out.push('"');
    }
    if n.kids.is_empty() {
        match &n.text {
            Some(t) => {
                check_chars(t, &path)?;
                out.push('>');
                escape_text(t, out);
                out.push_str("</");
                out.push_str(&qname(n));
                out.push_str(">\n");
            }
            None => out.push_str("/>\n"),
        }
        return Ok(());
    }
    out.push_str(">\n");
    for k in &n.kids {
        write_node(k, depth + 1, &path, None, out)?;
    }
    for _ in 0..depth {
        out.push_str("  ");
    }
    out.push_str("</");
    out.push_str(&qname(n));
    out.push_str(">\n");
    Ok(())
}

fn serialise(root: &Node) -> Result<String> {
    let mut nss = BTreeSet::new();
    root.collect_ns(&mut nss);
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    write_node(root, 0, "", Some(&nss), &mut out)?;
    Ok(out)
}

// ----------------------------------------------------------------- common

fn meta_attrs(n: &mut Node, m: &ItemMeta) {
    n.attr("put-code", m.put_code.map(|v| v.to_string()));
    n.attr("path", m.path.clone());
    n.attr(
        "visibility",
        m.visibility.as_ref().map(|v| v.as_str().to_string()),
    );
    n.attr(
        "display-index",
        m.display_index_text
            .clone()
            .or_else(|| m.display_index.map(|v| v.to_string())),
    );
}

fn meta_kids(n: &mut Node, m: &ItemMeta) {
    n.leaf(Ns::Common, "created-date", &m.created_date);
    n.leaf(Ns::Common, "last-modified-date", &m.last_modified_date);
    n.opt(m.source.as_ref().map(source));
}

/// An element with the item attributes and `element-summary` children.
fn item(ns: Ns, name: &'static str, m: &ItemMeta) -> Node {
    let mut n = Node::new(ns, name);
    meta_attrs(&mut n, m);
    meta_kids(&mut n, m);
    n
}

fn identifier(ns: Ns, name: &'static str, id: &OrcidIdentifier) -> Node {
    let mut n = Node::new(ns, name);
    n.leaf(Ns::Common, "uri", &id.uri);
    n.leaf(Ns::Common, "path", &id.path);
    n.leaf(Ns::Common, "host", &id.host);
    n
}

fn source(s: &Source) -> Node {
    let mut n = Node::new(Ns::Common, "source");
    n.opt(
        s.source_orcid
            .as_ref()
            .map(|i| identifier(Ns::Common, "source-orcid", i)),
    );
    n.opt(
        s.source_client_id
            .as_ref()
            .map(|i| identifier(Ns::Common, "source-client-id", i)),
    );
    n.leaf(Ns::Common, "source-name", &s.source_name);
    n.opt(
        s.assertion_origin_orcid
            .as_ref()
            .map(|i| identifier(Ns::Common, "assertion-origin-orcid", i)),
    );
    n.opt(
        s.assertion_origin_client_id
            .as_ref()
            .map(|i| identifier(Ns::Common, "assertion-origin-client-id", i)),
    );
    n.leaf(
        Ns::Common,
        "assertion-origin-name",
        &s.assertion_origin_name,
    );
    n
}

fn date(ns: Ns, name: &'static str, d: &FuzzyDate) -> Node {
    let mut n = Node::new(ns, name);
    if let Some(y) = d.year {
        n.kid(Node::text(Ns::Common, "year", format!("{y:04}")));
    }
    if let Some(m) = d.month {
        n.kid(Node::text(Ns::Common, "month", format!("{m:02}")));
    }
    if let Some(day) = d.day {
        n.kid(Node::text(Ns::Common, "day", format!("{day:02}")));
    }
    n
}

fn opt_date(n: &mut Node, ns: Ns, name: &'static str, d: &Option<FuzzyDate>) {
    if let Some(d) = d {
        n.kid(date(ns, name, d));
    }
}

fn organization(ns: Ns, name: &'static str, o: &Organization) -> Node {
    let mut n = Node::new(ns, name);
    n.leaf(Ns::Common, "name", &o.name);
    if let Some(a) = &o.address {
        let mut an = Node::new(Ns::Common, "address");
        an.leaf(Ns::Common, "city", &a.city);
        an.leaf(Ns::Common, "region", &a.region);
        an.leaf(Ns::Common, "country", &a.country);
        n.kid(an);
    }
    if let Some(d) = &o.disambiguated_organization {
        let mut dn = Node::new(Ns::Common, "disambiguated-organization");
        dn.leaf(
            Ns::Common,
            "disambiguated-organization-identifier",
            &d.identifier,
        );
        dn.leaf(Ns::Common, "disambiguation-source", &d.source);
        n.kid(dn);
    }
    n
}

fn external_id(ns: Ns, name: &'static str, x: &ExternalId) -> Node {
    let mut n = item(ns, name, &x.meta);
    n.leaf(Ns::Common, "external-id-type", &x.id_type);
    n.leaf(Ns::Common, "external-id-value", &x.value);
    if let Some(t) = &x.normalized {
        let mut tn = Node::new(Ns::Common, "external-id-normalized");
        tn.attr("transient", t.transient.map(bool_text));
        tn.text = t.value.clone();
        n.kid(tn);
    }
    if let Some(e) = &x.normalized_error {
        let mut en = Node::new(Ns::Common, "external-id-normalized-error");
        en.attr("transient", e.transient.map(bool_text));
        en.leaf(Ns::Common, "error-code", &e.error_code);
        en.leaf(Ns::Common, "error-message", &e.error_message);
        n.kid(en);
    }
    n.leaf(Ns::Common, "external-id-url", &x.url);
    n.leaf(
        Ns::Common,
        "external-id-relationship",
        &x.relationship.as_ref().map(|r| r.as_str().to_string()),
    );
    n
}

/// `<ns:name>` holding `common:external-id` children.
fn external_ids_in(ns: Ns, name: &'static str, ids: &[ExternalId]) -> Node {
    let mut n = Node::new(ns, name);
    for x in ids {
        n.kid(external_id(Ns::Common, "external-id", x));
    }
    n
}

/// `common:external-ids`, always written (groups, proposals, resource items).
fn external_ids(ids: &[ExternalId]) -> Node {
    external_ids_in(Ns::Common, "external-ids", ids)
}

/// `common:external-ids`, written only when there is at least one id.
fn opt_external_ids(n: &mut Node, ids: &[ExternalId]) {
    if !ids.is_empty() {
        n.kid(external_ids(ids));
    }
}

fn title(ns: Ns, name: &'static str, t: &Title) -> Node {
    let mut n = Node::new(ns, name);
    n.leaf(Ns::Common, "title", &t.title);
    n.leaf(Ns::Common, "subtitle", &t.subtitle);
    if let Some(tt) = &t.translated_title {
        let mut tn = Node::new(Ns::Common, "translated-title");
        tn.attr("language-code", tt.language_code.clone());
        tn.text = tt.value.clone();
        n.kid(tn);
    }
    n
}

fn enum_text<T: ToString>(v: &Option<T>) -> Option<String> {
    v.as_ref().map(|x| x.to_string())
}

fn contributor(ns: Ns, c: &Contributor) -> Node {
    let mut n = Node::new(ns, "contributor");
    n.opt(
        c.contributor_orcid
            .as_ref()
            .map(|i| identifier(Ns::Common, "contributor-orcid", i)),
    );
    n.leaf(ns, "credit-name", &c.credit_name);
    n.leaf(ns, "contributor-email", &c.contributor_email);
    if let Some(a) = &c.attributes {
        let mut an = Node::new(ns, "contributor-attributes");
        if ns == Ns::Funding {
            if a.sequence.is_some() {
                let mut bad = Node::new(ns, "contributor-sequence");
                bad.unwritable =
                    Some("a funding contributor has no contributor-sequence (funding-3.0.xsd)");
                an.kid(bad);
            }
        } else {
            an.leaf(ns, "contributor-sequence", &enum_text(&a.sequence));
        }
        an.leaf(ns, "contributor-role", &enum_text(&a.role));
        n.kid(an);
    }
    n
}

fn opt_contributors(n: &mut Node, ns: Ns, cs: &[Contributor]) {
    if !cs.is_empty() {
        let mut cn = Node::new(ns, "contributors");
        for c in cs {
            cn.kid(contributor(ns, c));
        }
        n.kid(cn);
    }
}

/// `<ns:name path=...>` with `common:last-modified-date`.
fn container(ns: Ns, name: &'static str, path: &Option<String>, lmd: &Option<String>) -> Node {
    let mut n = Node::new(ns, name);
    n.attr("path", path.clone());
    n.leaf(Ns::Common, "last-modified-date", lmd);
    n
}

/// A group: `common:last-modified-date`, `common:external-ids` (always).
fn group(ns: Ns, name: &'static str, lmd: &Option<String>, ids: &[ExternalId]) -> Node {
    let mut n = Node::new(ns, name);
    n.leaf(Ns::Common, "last-modified-date", lmd);
    n.kid(external_ids(ids));
    n
}

// ------------------------------------------------------------------- works

fn work(w: &Work) -> Node {
    let mut n = item(Ns::Work, "work", &w.meta);
    n.opt(w.title.as_ref().map(|t| title(Ns::Work, "title", t)));
    n.leaf(Ns::Work, "journal-title", &w.journal_title);
    n.leaf(Ns::Work, "short-description", &w.short_description);
    if let Some(c) = &w.citation {
        let mut cn = Node::new(Ns::Work, "citation");
        cn.leaf(Ns::Work, "citation-type", &enum_text(&c.citation_type));
        cn.leaf(Ns::Work, "citation-value", &c.citation_value);
        n.kid(cn);
    }
    n.leaf(Ns::Work, "type", &enum_text(&w.work_type));
    opt_date(&mut n, Ns::Common, "publication-date", &w.publication_date);
    opt_external_ids(&mut n, &w.external_ids);
    n.leaf(Ns::Common, "url", &w.url);
    opt_contributors(&mut n, Ns::Work, &w.contributors);
    n.leaf(Ns::Common, "language-code", &w.language_code);
    if let Some(c) = &w.country {
        let mut cn = Node::new(Ns::Common, "country");
        cn.attr("visibility", enum_text(&c.visibility));
        cn.text = c.value.clone();
        n.kid(cn);
    }
    n
}

fn work_summary(w: &WorkSummary) -> Node {
    let mut n = item(Ns::Work, "work-summary", &w.meta);
    n.opt(w.title.as_ref().map(|t| title(Ns::Work, "title", t)));
    opt_external_ids(&mut n, &w.external_ids);
    n.leaf(Ns::Common, "url", &w.url);
    n.leaf(Ns::Work, "type", &enum_text(&w.work_type));
    opt_date(&mut n, Ns::Common, "publication-date", &w.publication_date);
    n.leaf(Ns::Work, "journal-title", &w.journal_title);
    n
}

fn works(w: &Works) -> Node {
    let mut n = container(Ns::Activities, "works", &w.path, &w.last_modified_date);
    for g in &w.groups {
        let mut gn = group(
            Ns::Activities,
            "group",
            &g.last_modified_date,
            &g.external_ids,
        );
        for s in &g.work_summaries {
            gn.kid(work_summary(s));
        }
        n.kid(gn);
    }
    n
}

// ---------------------------------------------------------------- fundings

fn funding(f: &Funding) -> Node {
    let mut n = item(Ns::Funding, "funding", &f.meta);
    n.leaf(Ns::Funding, "type", &enum_text(&f.funding_type));
    n.leaf(
        Ns::Funding,
        "organization-defined-type",
        &f.organization_defined_type,
    );
    n.opt(f.title.as_ref().map(|t| title(Ns::Funding, "title", t)));
    n.leaf(Ns::Funding, "short-description", &f.short_description);
    if let Some(a) = &f.amount {
        let mut an = Node::new(Ns::Funding, "amount");
        an.attr("currency-code", a.currency_code.clone());
        an.text = a.value.clone();
        n.kid(an);
    }
    n.leaf(Ns::Common, "url", &f.url);
    opt_date(&mut n, Ns::Common, "start-date", &f.start_date);
    opt_date(&mut n, Ns::Common, "end-date", &f.end_date);
    opt_external_ids(&mut n, &f.external_ids);
    opt_contributors(&mut n, Ns::Funding, &f.contributors);
    n.opt(
        f.organization
            .as_ref()
            .map(|o| organization(Ns::Common, "organization", o)),
    );
    n
}

fn funding_summary(f: &FundingSummary) -> Node {
    let mut n = item(Ns::Funding, "funding-summary", &f.meta);
    n.opt(f.title.as_ref().map(|t| title(Ns::Funding, "title", t)));
    opt_external_ids(&mut n, &f.external_ids);
    n.leaf(Ns::Common, "url", &f.url);
    n.leaf(Ns::Funding, "type", &enum_text(&f.funding_type));
    opt_date(&mut n, Ns::Common, "start-date", &f.start_date);
    opt_date(&mut n, Ns::Common, "end-date", &f.end_date);
    n.opt(
        f.organization
            .as_ref()
            .map(|o| organization(Ns::Common, "organization", o)),
    );
    n
}

fn fundings(f: &Fundings) -> Node {
    let mut n = container(Ns::Activities, "fundings", &f.path, &f.last_modified_date);
    for g in &f.groups {
        let mut gn = group(
            Ns::Activities,
            "group",
            &g.last_modified_date,
            &g.external_ids,
        );
        for s in &g.funding_summaries {
            gn.kid(funding_summary(s));
        }
        n.kid(gn);
    }
    n
}

// ------------------------------------------------------------ affiliations

fn affiliation(a: &Affiliation, summary: bool) -> Node {
    let name = if summary {
        a.kind.summary_element()
    } else {
        a.kind.as_str()
    };
    let mut n = item(Ns::of_affiliation(a.kind), name, &a.meta);
    n.leaf(Ns::Common, "department-name", &a.department_name);
    n.leaf(Ns::Common, "role-title", &a.role_title);
    opt_date(&mut n, Ns::Common, "start-date", &a.start_date);
    opt_date(&mut n, Ns::Common, "end-date", &a.end_date);
    n.opt(
        a.organization
            .as_ref()
            .map(|o| organization(Ns::Common, "organization", o)),
    );
    n.leaf(Ns::Common, "url", &a.url);
    opt_external_ids(&mut n, &a.external_ids);
    n
}

fn affiliations(a: &Affiliations) -> Node {
    let mut n = container(
        Ns::Activities,
        a.kind.container_element(),
        &a.path,
        &a.last_modified_date,
    );
    for g in &a.groups {
        let mut gn = group(
            Ns::Activities,
            "affiliation-group",
            &g.last_modified_date,
            &g.external_ids,
        );
        for s in &g.summaries {
            gn.kid(affiliation(s, true));
        }
        n.kid(gn);
    }
    n
}

// ------------------------------------------------------------ peer reviews

fn peer_review_summary(p: &PeerReviewSummary) -> Node {
    let mut n = item(Ns::PeerReview, "peer-review-summary", &p.meta);
    n.leaf(
        Ns::PeerReview,
        "reviewer-role",
        &enum_text(&p.reviewer_role),
    );
    opt_external_ids(&mut n, &p.external_ids);
    n.leaf(Ns::PeerReview, "review-url", &p.review_url);
    n.leaf(Ns::PeerReview, "review-type", &enum_text(&p.review_type));
    opt_date(
        &mut n,
        Ns::PeerReview,
        "completion-date",
        &p.completion_date,
    );
    n.leaf(Ns::PeerReview, "review-group-id", &p.review_group_id);
    n.opt(
        p.convening_organization
            .as_ref()
            .map(|o| organization(Ns::PeerReview, "convening-organization", o)),
    );
    n
}

fn peer_review(p: &PeerReview) -> Node {
    let mut n = item(Ns::PeerReview, "peer-review", &p.meta);
    n.leaf(
        Ns::PeerReview,
        "reviewer-role",
        &enum_text(&p.reviewer_role),
    );
    n.kid(external_ids_in(
        Ns::PeerReview,
        "review-identifiers",
        &p.review_identifiers,
    ));
    n.leaf(Ns::PeerReview, "review-url", &p.review_url);
    n.leaf(Ns::PeerReview, "review-type", &enum_text(&p.review_type));
    opt_date(
        &mut n,
        Ns::PeerReview,
        "review-completion-date",
        &p.review_completion_date,
    );
    n.leaf(Ns::PeerReview, "review-group-id", &p.review_group_id);
    n.opt(
        p.subject_external_identifier
            .as_ref()
            .map(|x| external_id(Ns::PeerReview, "subject-external-identifier", x)),
    );
    n.leaf(
        Ns::PeerReview,
        "subject-container-name",
        &p.subject_container_name,
    );
    n.leaf(Ns::PeerReview, "subject-type", &p.subject_type);
    n.opt(
        p.subject_name
            .as_ref()
            .map(|t| title(Ns::PeerReview, "subject-name", t)),
    );
    n.leaf(Ns::PeerReview, "subject-url", &p.subject_url);
    n.opt(
        p.convening_organization
            .as_ref()
            .map(|o| organization(Ns::PeerReview, "convening-organization", o)),
    );
    n
}

fn peer_reviews(p: &PeerReviews) -> Node {
    let mut n = container(
        Ns::Activities,
        "peer-reviews",
        &p.path,
        &p.last_modified_date,
    );
    for g in &p.groups {
        let mut gn = group(
            Ns::Activities,
            "group",
            &g.last_modified_date,
            &g.external_ids,
        );
        for d in &g.duplicates {
            let mut dn = group(
                Ns::Activities,
                "peer-review-group",
                &d.last_modified_date,
                &d.external_ids,
            );
            for s in &d.summaries {
                dn.kid(peer_review_summary(s));
            }
            gn.kid(dn);
        }
        n.kid(gn);
    }
    n
}

// ------------------------------------------------------ research resources

fn hosts(orgs: &[Organization]) -> Node {
    let mut n = Node::new(Ns::ResearchResource, "hosts");
    for o in orgs {
        n.kid(organization(Ns::Common, "organization", o));
    }
    n
}

fn proposal(p: &Proposal) -> Node {
    let mut n = Node::new(Ns::ResearchResource, "proposal");
    n.opt(
        p.title
            .as_ref()
            .map(|t| title(Ns::ResearchResource, "title", t)),
    );
    n.kid(hosts(&p.hosts));
    n.kid(external_ids(&p.external_ids));
    opt_date(&mut n, Ns::Common, "start-date", &p.start_date);
    opt_date(&mut n, Ns::Common, "end-date", &p.end_date);
    n.leaf(Ns::Common, "url", &p.url);
    n
}

fn resource_item(r: &ResourceItem) -> Node {
    let mut n = Node::new(Ns::ResearchResource, "resource-item");
    n.leaf(Ns::ResearchResource, "resource-name", &r.resource_name);
    n.leaf(
        Ns::ResearchResource,
        "resource-type",
        &enum_text(&r.resource_type),
    );
    n.kid(hosts(&r.hosts));
    n.kid(external_ids(&r.external_ids));
    n.leaf(Ns::Common, "url", &r.url);
    n
}

fn research_resource(r: &ResearchResource) -> Node {
    let mut n = item(Ns::ResearchResource, "research-resource", &r.meta);
    n.opt(r.proposal.as_ref().map(proposal));
    let mut items = Node::new(Ns::ResearchResource, "resource-items");
    for i in &r.resource_items {
        items.kid(resource_item(i));
    }
    n.kid(items);
    n
}

fn research_resource_summary(r: &ResearchResourceSummary) -> Node {
    let mut n = item(Ns::ResearchResource, "research-resource-summary", &r.meta);
    n.opt(r.proposal.as_ref().map(proposal));
    n
}

fn research_resources(r: &ResearchResources) -> Node {
    let mut n = container(
        Ns::Activities,
        "research-resources",
        &r.path,
        &r.last_modified_date,
    );
    for g in &r.groups {
        let mut gn = group(
            Ns::Activities,
            "group",
            &g.last_modified_date,
            &g.external_ids,
        );
        for s in &g.summaries {
            gn.kid(research_resource_summary(s));
        }
        n.kid(gn);
    }
    n
}

fn activities_summary(a: &ActivitiesSummary) -> Node {
    let mut n = container(
        Ns::Activities,
        "activities-summary",
        &a.path,
        &a.last_modified_date,
    );
    n.opt(a.distinctions.as_ref().map(affiliations));
    n.opt(a.educations.as_ref().map(affiliations));
    n.opt(a.employments.as_ref().map(affiliations));
    n.opt(a.fundings.as_ref().map(fundings));
    n.opt(a.invited_positions.as_ref().map(affiliations));
    n.opt(a.memberships.as_ref().map(affiliations));
    n.opt(a.peer_reviews.as_ref().map(peer_reviews));
    n.opt(a.qualifications.as_ref().map(affiliations));
    n.opt(a.research_resources.as_ref().map(research_resources));
    n.opt(a.services.as_ref().map(affiliations));
    n.opt(a.works.as_ref().map(works));
    n
}

// ------------------------------------------------------------------ person

fn name(ns: Ns, nm: &Name) -> Node {
    let mut n = Node::new(ns, "name");
    n.attr("visibility", enum_text(&nm.visibility));
    n.attr("path", nm.path.clone());
    n.leaf(Ns::Common, "created-date", &nm.created_date);
    n.leaf(Ns::Common, "last-modified-date", &nm.last_modified_date);
    n.leaf(Ns::PersonalDetails, "given-names", &nm.given_names);
    n.leaf(Ns::PersonalDetails, "family-name", &nm.family_name);
    n.leaf(Ns::PersonalDetails, "credit-name", &nm.credit_name);
    n
}

fn biography(ns: Ns, b: &Biography) -> Node {
    let mut n = Node::new(ns, "biography");
    n.attr("visibility", enum_text(&b.visibility));
    n.attr("path", b.path.clone());
    n.leaf(Ns::Common, "created-date", &b.created_date);
    n.leaf(Ns::Common, "last-modified-date", &b.last_modified_date);
    n.leaf(Ns::PersonalDetails, "content", &b.content);
    n
}

fn other_name(x: &OtherName) -> Node {
    let mut n = item(Ns::OtherName, "other-name", &x.meta);
    n.leaf(Ns::OtherName, "content", &x.content);
    n
}

fn researcher_url(x: &ResearcherUrl) -> Node {
    let mut n = item(Ns::ResearcherUrl, "researcher-url", &x.meta);
    n.leaf(Ns::ResearcherUrl, "url-name", &x.url_name);
    n.leaf(Ns::ResearcherUrl, "url", &x.url);
    n
}

fn email(x: &Email) -> Node {
    let mut n = Node::new(Ns::Email, "email");
    meta_attrs(&mut n, &x.meta);
    n.attr("verified", x.verified.map(bool_text));
    n.attr("primary", x.primary.map(bool_text));
    meta_kids(&mut n, &x.meta);
    n.leaf(Ns::Email, "email", &x.email);
    n
}

fn address(x: &Address) -> Node {
    let mut n = item(Ns::Address, "address", &x.meta);
    n.leaf(Ns::Address, "country", &x.country);
    n
}

fn keyword(x: &Keyword) -> Node {
    let mut n = item(Ns::Keyword, "keyword", &x.meta);
    n.leaf(Ns::Keyword, "content", &x.content);
    n
}

fn person_external_identifier(x: &PersonExternalIdentifier) -> Node {
    external_id(Ns::ExternalIdentifier, "external-identifier", x)
}

fn list<T>(ns: Ns, name: &'static str, l: &ItemList<T>, each: fn(&T) -> Node) -> Node {
    let mut n = container(ns, name, &l.path, &l.last_modified_date);
    for x in &l.items {
        n.kid(each(x));
    }
    n
}

fn other_names(l: &OtherNames) -> Node {
    list(Ns::OtherName, "other-names", l, other_name)
}
fn researcher_urls(l: &ResearcherUrls) -> Node {
    list(Ns::ResearcherUrl, "researcher-urls", l, researcher_url)
}
fn emails(l: &Emails) -> Node {
    list(Ns::Email, "emails", l, email)
}
fn addresses(l: &Addresses) -> Node {
    list(Ns::Address, "addresses", l, address)
}
fn keywords(l: &Keywords) -> Node {
    list(Ns::Keyword, "keywords", l, keyword)
}
fn external_identifiers(l: &ExternalIdentifiers) -> Node {
    list(
        Ns::ExternalIdentifier,
        "external-identifiers",
        l,
        person_external_identifier,
    )
}

fn person(p: &Person) -> Node {
    let mut n = container(Ns::Person, "person", &p.path, &p.last_modified_date);
    n.opt(p.name.as_ref().map(|x| name(Ns::Person, x)));
    n.opt(p.other_names.as_ref().map(other_names));
    n.opt(p.biography.as_ref().map(|x| biography(Ns::Person, x)));
    n.opt(p.researcher_urls.as_ref().map(researcher_urls));
    n.opt(p.emails.as_ref().map(emails));
    n.opt(p.addresses.as_ref().map(addresses));
    n.opt(p.keywords.as_ref().map(keywords));
    n.opt(p.external_identifiers.as_ref().map(external_identifiers));
    n
}

fn personal_details(p: &PersonalDetails) -> Node {
    let mut n = container(
        Ns::PersonalDetails,
        "personal-details",
        &p.path,
        &p.last_modified_date,
    );
    n.opt(p.name.as_ref().map(|x| name(Ns::PersonalDetails, x)));
    n.opt(p.other_names.as_ref().map(other_names));
    n.opt(
        p.biography
            .as_ref()
            .map(|x| biography(Ns::PersonalDetails, x)),
    );
    n
}

// ------------------------------------------------------------------ record

fn history(h: &History) -> Node {
    let mut n = Node::new(Ns::History, "history");
    n.attr("visibility", enum_text(&h.visibility));
    n.leaf(
        Ns::History,
        "creation-method",
        &enum_text(&h.creation_method),
    );
    n.leaf(Ns::History, "completion-date", &h.completion_date);
    n.leaf(Ns::History, "submission-date", &h.submission_date);
    n.leaf(Ns::Common, "last-modified-date", &h.last_modified_date);
    n.leaf(Ns::History, "claimed", &h.claimed.map(bool_text));
    n.opt(h.source.as_ref().map(source));
    n.leaf(Ns::History, "deactivation-date", &h.deactivation_date);
    n.leaf(
        Ns::History,
        "verified-email",
        &h.verified_email.map(bool_text),
    );
    n.leaf(
        Ns::History,
        "verified-primary-email",
        &h.verified_primary_email.map(bool_text),
    );
    n
}

fn preferences(p: &Preferences) -> Node {
    let mut n = Node::new(Ns::Preferences, "preferences");
    n.leaf(Ns::Preferences, "locale", &p.locale);
    n
}

fn record(r: &Record) -> Node {
    let mut n = Node::new(Ns::Record, "record");
    n.attr("path", r.path.clone());
    n.opt(
        r.orcid_identifier
            .as_ref()
            .map(|i| identifier(Ns::Common, "orcid-identifier", i)),
    );
    n.opt(r.preferences.as_ref().map(preferences));
    n.opt(r.history.as_ref().map(history));
    n.opt(r.person.as_ref().map(person));
    n.opt(r.activities_summary.as_ref().map(activities_summary));
    n
}

fn orcid_error(e: &OrcidError) -> Node {
    let mut n = Node::new(Ns::Error, "error");
    n.leaf(
        Ns::Error,
        "response-code",
        &e.response_code.map(|v| v.to_string()),
    );
    n.leaf(Ns::Error, "developer-message", &e.developer_message);
    n.leaf(Ns::Error, "user-message", &e.user_message);
    n.leaf(
        Ns::Error,
        "error-code",
        &e.error_code.map(|v| v.to_string()),
    );
    n.leaf(Ns::Error, "more-info", &e.more_info);
    n
}

fn bulk(b: &Bulk) -> Node {
    let mut n = Node::new(Ns::Bulk, "bulk");
    for i in &b.items {
        n.kid(match i {
            BulkItem::Work(w) => work(w),
            BulkItem::Error(e) => orcid_error(e),
            _ => panic!("reference writer: a BulkItem kind it does not know"),
        });
    }
    n
}

fn search(s: &Search) -> Node {
    let mut n = Node::new(Ns::Search, "search");
    n.attr("num-found", s.num_found.map(|v| v.to_string()));
    for r in &s.results {
        let mut rn = Node::new(Ns::Search, "result");
        rn.kid(identifier(Ns::Common, "orcid-identifier", r));
        n.kid(rn);
    }
    n
}

fn expanded_search(s: &ExpandedSearch) -> Node {
    let ns = Ns::ExpandedSearch;
    let mut n = Node::new(ns, "expanded-search");
    n.attr("num-found", s.num_found.map(|v| v.to_string()));
    for r in &s.results {
        let mut rn = Node::new(ns, "expanded-result");
        rn.leaf(ns, "orcid-id", &r.orcid_id);
        rn.leaf(ns, "given-names", &r.given_names);
        rn.leaf(ns, "family-names", &r.family_names);
        rn.leaf(ns, "credit-name", &r.credit_name);
        for v in &r.other_names {
            rn.kid(Node::text(ns, "other-name", v.clone()));
        }
        for v in &r.emails {
            rn.kid(Node::text(ns, "email", v.clone()));
        }
        for v in &r.institution_names {
            rn.kid(Node::text(ns, "institution-name", v.clone()));
        }
        n.kid(rn);
    }
    n
}

fn root_node(root: &Root) -> Result<Node> {
    Ok(match root {
        Root::Record(r) => record(r),
        Root::ActivitiesSummary(a) => activities_summary(a),
        Root::Works(w) => works(w),
        Root::Work(w) => work(w),
        Root::WorkSummary(w) => work_summary(w),
        Root::Fundings(f) => fundings(f),
        Root::Funding(f) => funding(f),
        Root::FundingSummary(f) => funding_summary(f),
        Root::Affiliations(a) => affiliations(a),
        Root::Affiliation(a) => affiliation(a, false),
        Root::AffiliationSummary(a) => affiliation(a, true),
        Root::PeerReviews(p) => peer_reviews(p),
        Root::PeerReview(p) => peer_review(p),
        Root::PeerReviewSummary(p) => peer_review_summary(p),
        Root::ResearchResources(r) => research_resources(r),
        Root::ResearchResource(r) => research_resource(r),
        Root::ResearchResourceSummary(r) => research_resource_summary(r),
        Root::Person(p) => person(p),
        Root::PersonalDetails(p) => personal_details(p),
        Root::Name(x) => name(Ns::PersonalDetails, x),
        Root::Biography(x) => biography(Ns::PersonalDetails, x),
        Root::CreditName(x) => {
            let mut n = Node::new(Ns::PersonalDetails, "credit-name");
            n.text = x.clone();
            n
        }
        Root::OtherNames(l) => other_names(l),
        Root::OtherName(x) => other_name(x),
        Root::ResearcherUrls(l) => researcher_urls(l),
        Root::ResearcherUrl(x) => researcher_url(x),
        Root::Emails(l) => emails(l),
        Root::Email(x) => email(x),
        Root::Addresses(l) => addresses(l),
        Root::Address(x) => address(x),
        Root::Keywords(l) => keywords(l),
        Root::Keyword(x) => keyword(x),
        Root::ExternalIdentifiers(l) => external_identifiers(l),
        Root::ExternalIdentifier(x) => person_external_identifier(x),
        Root::History(h) => history(h),
        Root::Preferences(p) => preferences(p),
        Root::Bulk(b) => bulk(b),
        Root::Error(e) => orcid_error(e),
        Root::Search(s) => search(s),
        Root::ExpandedSearch(s) => expanded_search(s),
        Root::Unknown(name) => {
            return Err(Error::Unwritable {
                path: name.clone(),
                reason: "unknown root element".to_string(),
            });
        }
        _ => panic!("reference writer: a Root kind it does not know"),
    })
}

/// The 0.1.0 `yorc::xml::to_string`.
pub fn to_string(root: &Root) -> Result<String> {
    serialise(&root_node(root)?)
}
