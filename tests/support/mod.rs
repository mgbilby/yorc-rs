//! Shared helpers for the write tests: sample paths and a small XML
//! canonicaliser (namespace URI + local name, sorted attributes, trimmed text,
//! no comments/PIs/`xmlns`/`xsi:*`, data-less elements dropped).

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

pub fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// ORCID's schemas and samples, vendored in `reference/orcid-model`.
pub fn model_dir() -> PathBuf {
    crate_root().join("reference/orcid-model")
}

/// The real public record 0000-0003-0100-6634 (CC0, see NOTICE).
pub fn fixture_path() -> PathBuf {
    crate_root().join("tests/data/0000-0003-0100-6634.xml")
}

pub fn record_xsd_dir() -> PathBuf {
    model_dir().join("record_3.0")
}

pub fn xml_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "xml"))
        .collect();
    v.sort();
    v
}

/// Canonical element.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct C {
    pub ns: String,
    pub name: String,
    pub attrs: BTreeMap<String, String>,
    pub text: String,
    pub kids: Vec<C>,
}

fn decode(raw: &str, out: &mut String) {
    let mut rest = raw;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let Some(semi) = after.find(';') else {
            out.push('&');
            rest = after;
            continue;
        };
        entity(&after[..semi], out);
        rest = &after[semi + 1..];
    }
    out.push_str(rest);
}

fn entity(name: &str, out: &mut String) {
    let c = match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        n => {
            let code = if let Some(h) = n.strip_prefix("#x").or_else(|| n.strip_prefix("#X")) {
                u32::from_str_radix(h, 16).ok()
            } else if let Some(d) = n.strip_prefix('#') {
                d.parse().ok()
            } else {
                None
            };
            code.and_then(char::from_u32)
        }
    };
    match c {
        Some(c) => out.push(c),
        None => {
            out.push('&');
            out.push_str(name);
            out.push(';');
        }
    }
}

struct Open {
    el: C,
    prefix: String,
    scope: Vec<(String, String)>,
}

fn start(e: &BytesStart<'_>, scopes: &[Vec<(String, String)>]) -> Open {
    let qname = e.name().into_inner().to_string();
    let (prefix, local) = match qname.split_once(':') {
        Some((p, l)) => (p.to_string(), l.to_string()),
        None => (String::new(), qname.clone()),
    };
    let mut scope = Vec::new();
    let mut attrs = BTreeMap::new();
    for a in e.attributes() {
        let a = a.expect("attribute");
        let key: &str = a.key.0;
        let mut value = String::new();
        decode(&a.value, &mut value);
        if key == "xmlns" {
            scope.push((String::new(), value));
        } else if let Some(p) = key.strip_prefix("xmlns:") {
            scope.push((p.to_string(), value));
        } else if key.starts_with("xsi:") {
            continue;
        } else {
            attrs.insert(key.to_string(), value.trim().to_string());
        }
    }
    let _ = scopes;
    Open {
        el: C {
            ns: String::new(),
            name: local,
            attrs,
            text: String::new(),
            kids: Vec::new(),
        },
        prefix,
        scope,
    }
}

fn resolve(prefix: &str, own: &[(String, String)], stack: &[Open]) -> String {
    for (p, uri) in own.iter().rev() {
        if p == prefix {
            return uri.clone();
        }
    }
    for o in stack.iter().rev() {
        for (p, uri) in o.scope.iter().rev() {
            if p == prefix {
                return uri.clone();
            }
        }
    }
    format!("unbound:{prefix}")
}

fn finish(mut o: Open, stack: &mut [Open], root: &mut Option<C>) {
    o.el.ns = resolve(&o.prefix, &o.scope, stack);
    o.el.text = o.el.text.trim().to_string();
    if !o.el.kids.is_empty() {
        o.el.text.clear();
    }
    let keep = !(o.el.text.is_empty() && o.el.attrs.is_empty() && o.el.kids.is_empty());
    match stack.last_mut() {
        Some(parent) => {
            if keep {
                parent.el.kids.push(o.el);
            }
        }
        None => *root = Some(o.el),
    }
}

/// Canonical form of an XML document.
pub fn canonical(xml: &str) -> C {
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<Open> = Vec::new();
    let mut root = None;
    loop {
        match reader.read_event().expect("well-formed XML") {
            Event::Start(e) => {
                let o = start(&e, &[]);
                stack.push(o);
            }
            Event::Empty(e) => {
                let o = start(&e, &[]);
                finish(o, &mut stack, &mut root);
            }
            Event::End(_) => {
                let o = stack.pop().expect("balanced");
                finish(o, &mut stack, &mut root);
            }
            Event::Text(t) => {
                if let Some(top) = stack.last_mut() {
                    decode(&t.into_inner(), &mut top.el.text);
                }
            }
            Event::CData(t) => {
                if let Some(top) = stack.last_mut() {
                    top.el.text.push_str(&t.into_inner());
                }
            }
            Event::GeneralRef(r) => {
                if let Some(top) = stack.last_mut() {
                    entity(&r.into_inner(), &mut top.el.text);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    root.expect("root element")
}

/// First difference between two canonical trees, as a path and a message.
pub fn diff(a: &C, b: &C, path: &str) -> Option<String> {
    let here = format!("{path}/{}", a.name);
    if (a.ns.as_str(), a.name.as_str()) != (b.ns.as_str(), b.name.as_str()) {
        return Some(format!(
            "{here}: element {{{}}}{} vs {{{}}}{}",
            a.ns, a.name, b.ns, b.name
        ));
    }
    if a.attrs != b.attrs {
        return Some(format!("{here}: attributes {:?} vs {:?}", a.attrs, b.attrs));
    }
    if a.text != b.text {
        return Some(format!("{here}: text {:?} vs {:?}", a.text, b.text));
    }
    for (i, (x, y)) in a.kids.iter().zip(b.kids.iter()).enumerate() {
        if let Some(d) = diff(x, y, &format!("{here}[{i}]")) {
            return Some(d);
        }
    }
    if a.kids.len() != b.kids.len() {
        let names = |c: &C| c.kids.iter().map(|k| k.name.clone()).collect::<Vec<_>>();
        return Some(format!(
            "{here}: {} vs {} children: {:?} vs {:?}",
            a.kids.len(),
            b.kids.len(),
            names(a),
            names(b)
        ));
    }
    None
}

/// The XSD that declares a root element in namespace `ns_uri`.
pub fn xsd_for(ns_uri: &str) -> PathBuf {
    let prefix = ns_uri.rsplit('/').next().unwrap_or_default();
    let file = match prefix {
        "external-identifier" => "person-external-identifier-3.0.xsd".to_string(),
        p => format!("{p}-3.0.xsd"),
    };
    record_xsd_dir().join(file)
}

/// `xmllint --noout --schema <xsd> <file>`: `None` if xmllint is not installed.
pub fn xmllint(xsd: &Path, file: &Path) -> Option<Result<(), String>> {
    let out = std::process::Command::new("xmllint")
        .arg("--noout")
        .arg("--schema")
        .arg(xsd)
        .arg(file)
        .output()
        .ok()?;
    Some(if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    })
}

pub fn scratch_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("yorc-rs-tests-{}-{name}", std::process::id()));
    fs::create_dir_all(&d).expect("scratch dir");
    d
}

/// Removes every element `{ns}name` from a canonical tree.
pub fn prune(c: &mut C, ns: &str, name: &str) {
    c.kids.retain(|k| !(k.ns == ns && k.name == name));
    for k in &mut c.kids {
        prune(k, ns, name);
    }
}
