//! group-id-3.0 documents (`group-id-record`, `group-id-records`).
//!
//! Their `group-id` namespace is not one of the record namespaces of
//! [`super::Ns`], so they are written here rather than by the record emitter, with
//! the same layout: the XML declaration, two-space indentation, `<x/>` for an
//! element with no content, every namespace used declared on the root in
//! prefix order (`common` before `group-id`) before its attributes, and the
//! same escaping. Elements follow ORCID's `GroupIdRecord` (`propOrder`):
//! `common:last-modified-date`, `common:created-date`, `name`, `group-id`,
//! `description`, `type`, `common:source`; the page is
//! `common:last-modified-date`, `total`, `page`, `page-size`, then the
//! records.

use super::{INDENT, URI_BASE, XML_DECL, bad_char, escape_text, push_int};
use crate::error::Result;
use crate::model::*;

const COMMON: &str = "common";
const GROUP_ID: &str = "group-id";

struct G {
    out: String,
    /// Open elements (prefix, name), outermost first.
    open: Vec<(&'static str, &'static str)>,
    /// The innermost start tag is open (`>` not written yet).
    pending: bool,
    /// A `common:` element was written.
    common: bool,
    /// Byte offset just after the root element's name.
    root_end: usize,
}

impl G {
    fn new() -> G {
        let mut out = String::with_capacity(1024);
        out.push_str(XML_DECL);
        G {
            root_end: out.len(),
            out,
            open: Vec::with_capacity(8),
            pending: false,
            common: false,
        }
    }

    fn finish(mut self) -> String {
        let mut decls = String::with_capacity(128);
        let mut declare = |prefix: &str| {
            decls.push_str(" xmlns:");
            decls.push_str(prefix);
            decls.push_str("=\"");
            decls.push_str(URI_BASE);
            decls.push_str(prefix);
            decls.push('"');
        };
        if self.common {
            declare(COMMON);
        }
        declare(GROUP_ID);
        self.out.insert_str(self.root_end, &decls);
        self.out
    }

    fn path(&self, last: &str) -> String {
        let mut p = String::new();
        for (_, name) in &self.open {
            p.push_str(name);
            p.push('/');
        }
        p.push_str(last);
        p
    }

    fn indent(&mut self) {
        let mut n = 2 * self.open.len();
        while n > INDENT.len() {
            self.out.push_str(INDENT);
            n -= INDENT.len();
        }
        self.out.push_str(&INDENT[..n]);
    }

    fn qname(&mut self, prefix: &str, name: &str) {
        if prefix == COMMON {
            self.common = true;
        }
        self.out.push_str(prefix);
        self.out.push(':');
        self.out.push_str(name);
    }

    fn close_pending(&mut self) {
        if self.pending {
            self.out.push_str(">\n");
            self.pending = false;
        }
    }

    fn start(&mut self, prefix: &'static str, name: &'static str) {
        self.close_pending();
        self.indent();
        self.out.push('<');
        self.qname(prefix, name);
        if self.open.is_empty() {
            self.root_end = self.out.len();
        }
        self.open.push((prefix, name));
        self.pending = true;
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

    fn end(&mut self) {
        let Some((prefix, name)) = self.open.pop() else {
            return;
        };
        if self.pending {
            self.pending = false;
            self.out.push_str("/>\n");
            return;
        }
        self.indent();
        self.out.push_str("</");
        self.qname(prefix, name);
        self.out.push_str(">\n");
    }

    fn open_leaf(&mut self, prefix: &'static str, name: &'static str) {
        self.close_pending();
        self.indent();
        self.out.push('<');
        self.qname(prefix, name);
        self.out.push('>');
    }

    fn close_leaf(&mut self, prefix: &'static str, name: &'static str) {
        self.out.push_str("</");
        self.qname(prefix, name);
        self.out.push_str(">\n");
    }

    fn leaf(
        &mut self,
        prefix: &'static str,
        name: &'static str,
        value: Option<&str>,
    ) -> Result<()> {
        let Some(v) = value else {
            return Ok(());
        };
        self.open_leaf(prefix, name);
        if let Err(c) = escape_text(v, &mut self.out) {
            return Err(bad_char(self.path(name), c));
        }
        self.close_leaf(prefix, name);
        Ok(())
    }

    fn leaf_int(&mut self, prefix: &'static str, name: &'static str, value: Option<i64>) {
        if let Some(v) = value {
            self.open_leaf(prefix, name);
            push_int(&mut self.out, v, 0);
            self.close_leaf(prefix, name);
        }
    }

    fn identifier(&mut self, name: &'static str, id: &Option<OrcidIdentifier>) -> Result<()> {
        let Some(id) = id else {
            return Ok(());
        };
        self.start(COMMON, name);
        self.leaf(COMMON, "uri", id.uri.as_deref())?;
        self.leaf(COMMON, "path", id.path.as_deref())?;
        self.leaf(COMMON, "host", id.host.as_deref())?;
        self.end();
        Ok(())
    }

    /// `common:source`, as the record writer writes it.
    fn source(&mut self, s: &Source) -> Result<()> {
        self.start(COMMON, "source");
        self.identifier("source-orcid", &s.source_orcid)?;
        self.identifier("source-client-id", &s.source_client_id)?;
        self.leaf(COMMON, "source-name", s.source_name.as_deref())?;
        self.identifier("assertion-origin-orcid", &s.assertion_origin_orcid)?;
        self.identifier("assertion-origin-client-id", &s.assertion_origin_client_id)?;
        self.leaf(
            COMMON,
            "assertion-origin-name",
            s.assertion_origin_name.as_deref(),
        )?;
        self.end();
        Ok(())
    }

    fn record(&mut self, r: &GroupIdRecord) -> Result<()> {
        self.start(GROUP_ID, "group-id-record");
        self.attr_int("put-code", r.put_code);
        self.leaf(
            COMMON,
            "last-modified-date",
            r.last_modified_date.as_deref(),
        )?;
        self.leaf(COMMON, "created-date", r.created_date.as_deref())?;
        self.leaf(GROUP_ID, "name", r.name.as_deref())?;
        self.leaf(GROUP_ID, "group-id", r.group_id.as_deref())?;
        self.leaf(GROUP_ID, "description", r.description.as_deref())?;
        self.leaf(GROUP_ID, "type", r.group_type.as_ref().map(|t| t.as_str()))?;
        if let Some(s) = &r.source {
            self.source(s)?;
        }
        self.end();
        Ok(())
    }
}

/// A `group-id-record` document.
pub(super) fn record_document(r: &GroupIdRecord) -> Result<String> {
    let mut g = G::new();
    g.record(r)?;
    Ok(g.finish())
}

/// A `group-id-records` document.
pub(super) fn records_document(p: &GroupIdRecords) -> Result<String> {
    let mut g = G::new();
    g.start(GROUP_ID, "group-id-records");
    g.leaf(
        COMMON,
        "last-modified-date",
        p.last_modified_date.as_deref(),
    )?;
    g.leaf_int(GROUP_ID, "total", p.total);
    g.leaf_int(GROUP_ID, "page", p.page);
    g.leaf_int(GROUP_ID, "page-size", p.page_size);
    for r in &p.records {
        g.record(r)?;
    }
    g.end();
    Ok(g.finish())
}
