//! ORCID JSON (feature `json`): round-trip every JSON sample, compare the
//! written JSON with the original (normalised), and check JSON ↔ XML
//! equivalence in both directions, with xmllint validation of the XML written
//! from JSON samples when xmllint is installed.
#![cfg(all(feature = "json", feature = "xml"))]

mod support;

use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use support::*;
use yorc::json;
use yorc::xml;

fn json_samples() -> Vec<(PathBuf, &'static str)> {
    let mut out = Vec::new();
    for dir in ["read_samples", "write_samples"] {
        let d = record_xsd_dir().join("samples").join(dir);
        let mut files: Vec<_> = fs::read_dir(&d)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();
        for f in files {
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            if let Some(kind) = kind_of_sample(&name) {
                out.push((f, kind));
            }
        }
    }
    out
}

/// Document kind of a JSON sample, from its file name; `None` for the one
/// sample outside the record model.
fn kind_of_sample(name: &str) -> Option<&'static str> {
    let kinds: &[(&str, &str)] = &[
        ("record-full", "record"),
        ("address", "address"),
        ("bulk-", "bulk"),
        ("distinction", "distinction"),
        ("education", "education"),
        ("employment", "employment"),
        ("external-identifier", "external-identifier"),
        ("funding", "funding"),
        ("invited-position", "invited-position"),
        ("keywords", "keyword"), // a single keyword despite the name
        ("membership", "membership"),
        ("other-names", "other-name"), // a single other name
        ("peer-review-group-id", ""),  // group-id-3.0, not a record document
        ("peer-review", "peer-review"),
        ("qualification", "qualification"),
        ("research-resource", "research-resource"),
        ("researcher-url", "researcher-url"),
        ("service", "service"),
        ("work", "work"),
    ];
    let (_, k) = kinds
        .iter()
        .find(|(p, _)| name.starts_with(p))
        .unwrap_or_else(|| panic!("no kind for {name}"));
    (!k.is_empty()).then_some(*k)
}

/// JSON normalised for comparison: nulls, empty strings, empty arrays and
/// empty objects removed; `{"value": x}` unwrapped; scalars as trimmed text.
fn norm(v: &Value) -> Option<Value> {
    match v {
        Value::Null => None,
        Value::String(s) => {
            let t = s.trim();
            (!t.is_empty()).then(|| Value::String(t.to_string()))
        }
        Value::Number(n) => Some(Value::String(n.to_string())),
        Value::Bool(b) => Some(Value::String(b.to_string())),
        Value::Array(a) => {
            let items: Vec<Value> = a.iter().filter_map(norm).collect();
            (!items.is_empty()).then_some(Value::Array(items))
        }
        Value::Object(o) => {
            if o.len() == 1 {
                if let Some(inner) = o.get("value") {
                    return norm(inner);
                }
            }
            let m: serde_json::Map<String, Value> = o
                .iter()
                .filter_map(|(k, v)| norm(v).map(|v| (k.clone(), v)))
                .collect();
            (!m.is_empty()).then_some(Value::Object(m))
        }
    }
}

/// Keys the model does not hold, removed from the original before comparing:
/// none for the record documents. (`peer-review-group-id.json` is skipped as
/// a whole: it is a group-id-3.0 document.)
fn first_difference(a: &Value, b: &Value, path: &str) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys()) {
                match (x.get(k), y.get(k)) {
                    (Some(p), Some(q)) => {
                        if let Some(d) = first_difference(p, q, &format!("{path}/{k}")) {
                            return Some(d);
                        }
                    }
                    (p, q) => return Some(format!("{path}/{k}: {p:?} vs {q:?}")),
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            for (i, (p, q)) in x.iter().zip(y.iter()).enumerate() {
                if let Some(d) = first_difference(p, q, &format!("{path}[{i}]")) {
                    return Some(d);
                }
            }
            (x.len() != y.len()).then(|| format!("{path}: {} vs {} items", x.len(), y.len()))
        }
        _ => (a != b).then(|| format!("{path}: {a} vs {b}")),
    }
}

#[test]
fn json_samples_round_trip() {
    let samples = json_samples();
    assert_eq!(samples.len(), 27, "JSON samples in the record model");
    for (f, kind) in &samples {
        let text = fs::read_to_string(f).unwrap();
        let doc = json::from_str(kind, &text).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let written = json::to_string(&doc.root).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let again = json::from_str(kind, &written).unwrap();
        assert_eq!(doc.root, again.root, "{}: model changed", f.display());
        assert_eq!(
            written,
            json::to_string(&again.root).unwrap(),
            "{}: not deterministic",
            f.display()
        );
        // Same content as the original, up to wrapping, nulls and number/string form.
        let original: Value = serde_json::from_str(&text).unwrap();
        let ours: Value = serde_json::from_str(&written).unwrap();
        if let Some(d) = first_difference(&norm(&original).unwrap(), &norm(&ours).unwrap(), kind) {
            panic!(
                "{}: written JSON differs from the original: {d}\n{written}",
                f.display()
            );
        }
    }
}

/// JSON sample → model → XML: the XML validates against the XSD (if xmllint
/// is installed) and reads back to the same content.
#[test]
fn json_samples_as_xml() {
    let dir = scratch_dir("json-xml");
    let mut validated = 0;
    for (f, kind) in json_samples() {
        let doc = json::from_str(kind, &fs::read_to_string(&f).unwrap()).unwrap();
        let x = xml::to_string(&doc.root).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let back = xml::from_str(&x).unwrap();
        assert_eq!(
            json::to_value(&doc.root).unwrap(),
            json::to_value(&back.root).unwrap(),
            "{}: JSON -> XML -> model differs",
            f.display()
        );
        let out = dir.join(format!("{}.xml", f.file_stem().unwrap().to_string_lossy()));
        fs::write(&out, &x).unwrap();
        let xsd = xsd_for(&canonical(&x).ns);
        match xmllint(&xsd, &out) {
            None => {
                eprintln!("xmllint not installed: skipping XSD validation");
                let _ = fs::remove_dir_all(&dir);
                return;
            }
            Some(Err(e)) => panic!(
                "{}: XML written from JSON does not validate:\n{e}\n{x}",
                f.display()
            ),
            Some(Ok(())) => validated += 1,
        }
    }
    let _ = fs::remove_dir_all(&dir);
    eprintln!("xmllint: {validated} XML documents written from JSON samples validate");
    assert_eq!(validated, 27);
}

/// XML sample → model → JSON → model: same content (timestamps compare as
/// instants, since JSON carries epoch milliseconds).
#[test]
fn xml_samples_through_json() {
    let mut n = 0;
    let mut files = xml_files(&record_xsd_dir().join("samples/read_samples"));
    files.extend(xml_files(&record_xsd_dir().join("samples/write_samples")));
    files.push(fixture_path());
    for f in files {
        let doc = xml::from_str(&fs::read_to_string(&f).unwrap()).unwrap();
        let kind = json::kind_of(&doc.root).unwrap();
        let j = json::to_string(&doc.root).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let back = json::from_str(kind, &j).unwrap();
        assert_eq!(
            json::to_value(&doc.root).unwrap(),
            json::to_value(&back.root).unwrap(),
            "{}: XML -> JSON -> model differs",
            f.display()
        );
        // and the XML written from the JSON-read model is the same document
        // except for the timestamp spelling and history/@visibility, which
        // ORCID's JSON has no member for (json.rs module docs): the JSON
        // writer leaves it out, so the round trip loses exactly that
        let x1 = strip_history_visibility(canonical(&xml::to_string(&doc.root).unwrap()));
        let x2 = canonical(&xml::to_string(&back.root).unwrap());
        if let Some(d) = diff(&strip_times(x1), &strip_times(x2), "") {
            panic!("{}: XML differs after a JSON round trip: {d}", f.display());
        }
        n += 1;
    }
    assert_eq!(n, 75);
}

const TIME_ELEMENTS: [&str; 5] = [
    "created-date",
    "last-modified-date",
    "completion-date",
    "submission-date",
    "deactivation-date",
];

/// Removes the `visibility` attribute of `history` and a work's `country`
/// (XML-only).
fn strip_history_visibility(mut c: C) -> C {
    if c.name == "history" || c.name == "country" {
        c.attrs.remove("visibility");
    }
    c.kids = c.kids.into_iter().map(strip_history_visibility).collect();
    c
}

/// Replaces the text of timestamp elements by epoch milliseconds.
fn strip_times(mut c: C) -> C {
    if TIME_ELEMENTS.contains(&c.name.as_str()) && c.kids.is_empty() {
        if let Some(ms) = json::datetime_to_millis(&c.text) {
            c.text = ms.to_string();
        }
    }
    c.kids = c.kids.into_iter().map(strip_times).collect();
    c
}
