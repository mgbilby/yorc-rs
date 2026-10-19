//! XML writing: round-trip every ORCID v3.0 sample, check the written XML
//! against the original (canonicalised), validate it with xmllint against the
//! vendored XSDs when xmllint is installed.
#![cfg(feature = "xml")]

mod reference_writer;
mod support;

use std::fs;
use std::path::{Path, PathBuf};

use support::*;
use yorc::model::{BulkItem, Root};
use yorc::xml;

fn write_samples() -> Vec<PathBuf> {
    xml_files(&record_xsd_dir().join("samples/write_samples"))
}

fn read_samples() -> Vec<PathBuf> {
    xml_files(&record_xsd_dir().join("samples/read_samples"))
}

fn fixture() -> PathBuf {
    fixture_path()
}

/// Elements in ORCID's samples that the v3.0 XSDs do not allow, so the model
/// has no place for them and the writer cannot reproduce them: (file,
/// namespace, local name, reason). `samples_outside_the_schema_fail_xmllint`
/// checks that each of these originals really fails XSD validation.
const NOT_IN_SCHEMA: &[(&str, &str, &str, &str)] = &[(
    "peer-reviews-3.0.xml",
    "http://www.orcid.org/ns/peer-review",
    "summary",
    "pre-3.0 layout: peer-review:summary directly in activities:group; v3.0 has activities:peer-review-group/peer-review:peer-review-summary",
)];

/// read → write → read: the model must be identical, the second write must be
/// byte-identical to the first, and the written XML must be canonically equal
/// to the original. Returns the written XML.
fn round_trip(path: &Path) -> String {
    let original = fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let doc = xml::from_str(&original).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(
        !matches!(doc.root, Root::Unknown(_)),
        "{}: unknown root",
        path.display()
    );
    let written = xml::to_string(&doc.root).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let again = xml::from_str(&written)
        .unwrap_or_else(|e| panic!("{}: re-read: {e}\n{written}", path.display()));
    assert_eq!(
        doc,
        again,
        "{}: model changed in the round trip",
        path.display()
    );
    let rewritten = xml::to_string(&again.root).unwrap();
    assert_eq!(
        written,
        rewritten,
        "{}: writing is not deterministic",
        path.display()
    );
    let mut expected = canonical(&original);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    for (file, ns, local, _why) in NOT_IN_SCHEMA {
        if *file == name {
            prune(&mut expected, ns, local);
        }
    }
    if let Some(d) = diff(&expected, &canonical(&written), "") {
        panic!(
            "{}: written XML differs from the original: {d}\n--- written ---\n{written}",
            path.display()
        );
    }
    written
}

#[test]
fn write_samples_round_trip() {
    let files = write_samples();
    assert_eq!(files.len(), 21, "write_samples/*.xml");
    for f in &files {
        round_trip(f);
    }
}

#[test]
fn read_samples_round_trip() {
    let files = read_samples();
    assert_eq!(files.len(), 53, "read_samples/*.xml");
    let mut failed = Vec::new();
    for f in &files {
        if let Err(e) = std::panic::catch_unwind(|| round_trip(f)) {
            let msg = e.downcast_ref::<String>().cloned().unwrap_or_default();
            failed.push(msg.lines().next().unwrap_or_default().to_string());
        }
    }
    assert!(
        failed.is_empty(),
        "{} sample(s) failed:\n{}",
        failed.len(),
        failed.join("\n")
    );
}

#[test]
fn real_record_round_trips() {
    let written = round_trip(&fixture());
    assert!(written.starts_with(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<record:record xmlns:activities="
    ));
}

/// Every written sample (and the real record) must validate against its XSD.
/// Skipped (with a note) when xmllint is not installed.
#[test]
fn written_documents_validate_with_xmllint() {
    let dir = scratch_dir("xmllint");
    let mut checked = 0;
    let mut originals_invalid = Vec::new();
    for f in write_samples()
        .iter()
        .chain(read_samples().iter())
        .chain(std::iter::once(&fixture()))
    {
        let original = fs::read_to_string(f).unwrap();
        let root_ns = canonical(&original).ns;
        let xsd = xsd_for(&root_ns);
        if !xsd.exists() {
            panic!("{}: no XSD {}", f.display(), xsd.display());
        }
        match xmllint(&xsd, f) {
            None => {
                eprintln!("xmllint not installed: skipping XSD validation");
                return;
            }
            Some(Err(_)) => {
                originals_invalid.push(f.file_name().unwrap().to_string_lossy().into_owned())
            }
            Some(Ok(())) => {}
        }
        // Written documents must validate even where the original does not
        // (the originals' failures are untrimmed xs:dateTime values, an empty
        // journal-title and the pre-3.0 peer-review layout, all of which the
        // reader normalises or drops).
        let written = xml::to_string(&xml::from_str(&original).unwrap().root).unwrap();
        let out = dir.join(f.file_name().unwrap());
        fs::write(&out, &written).unwrap();
        if let Some(Err(e)) = xmllint(&xsd, &out) {
            panic!(
                "{}: written XML does not validate against {}:\n{e}\n{written}",
                f.display(),
                xsd.display()
            );
        }
        checked += 1;
    }
    let _ = fs::remove_dir_all(&dir);
    eprintln!(
        "xmllint: {checked} written documents valid, including {} whose originals do not validate: {:?}",
        originals_invalid.len(),
        originals_invalid
    );
    assert_eq!(checked, 75, "documents checked");
}

#[test]
fn samples_outside_the_schema_fail_xmllint() {
    for (file, _, _, why) in NOT_IN_SCHEMA {
        let f = record_xsd_dir().join("samples/read_samples").join(file);
        let root_ns = canonical(&fs::read_to_string(&f).unwrap()).ns;
        match xmllint(&xsd_for(&root_ns), &f) {
            None => return,
            Some(r) => assert!(
                r.is_err(),
                "{file} validates, but is listed as outside the schema ({why})"
            ),
        }
    }
}

// ------------------------------------------- the 0.1.0 writer as reference

/// The streaming writer must give exactly the bytes, or the error, of the
/// 0.1.0 tree writer (`tests/reference_writer`).
fn assert_same_as_reference(what: &str, root: &Root) {
    let new = xml::to_string(root);
    let old = reference_writer::to_string(root);
    match (&new, &old) {
        (Ok(a), Ok(b)) if a != b => {
            let at = a
                .bytes()
                .zip(b.bytes())
                .position(|(x, y)| x != y)
                .unwrap_or(a.len().min(b.len()));
            let from = at.saturating_sub(200);
            panic!(
                "{what}: output differs from the reference at byte {at} (lengths {} and {}):\n\
                 --- new ---\n{}\n--- reference ---\n{}",
                a.len(),
                b.len(),
                String::from_utf8_lossy(&a.as_bytes()[from..(at + 200).min(a.len())]),
                String::from_utf8_lossy(&b.as_bytes()[from..(at + 200).min(b.len())]),
            );
        }
        _ => assert_eq!(new, old, "{what}"),
    }
}

/// `xml` with `text` appended to every non-blank text node of a start tag and
/// `attr` to every double-quoted attribute value but namespace declarations.
fn spice(xml: &str, text: &str, attr: &str) -> String {
    let mut out = String::with_capacity(xml.len() * 2);
    let mut parts = xml.split('<');
    out.push_str(parts.next().unwrap_or_default());
    for part in parts {
        out.push('<');
        let Some(gt) = part.find('>') else {
            out.push_str(part);
            continue;
        };
        let (tag, rest) = part.split_at(gt);
        if tag.starts_with(['?', '!', '/']) {
            out.push_str(part);
            continue;
        }
        // attributes
        let mut t = tag;
        while let Some(eq) = t.find("=\"") {
            let name_start = t[..eq].rfind(char::is_whitespace).map_or(0, |i| i + 1);
            let name = &t[name_start..eq];
            let value_start = eq + 2;
            let Some(len) = t[value_start..].find('"') else {
                break;
            };
            out.push_str(&t[..value_start + len]);
            if !name.starts_with("xmlns") {
                out.push_str(attr);
            }
            t = &t[value_start + len..];
        }
        out.push_str(t);
        // text: `rest` is `>` and the text up to the next `<`
        out.push('>');
        let body = &rest[1..];
        out.push_str(body);
        if !tag.ends_with('/') && !body.trim().is_empty() {
            out.push_str(text);
        }
    }
    out
}

fn reference_documents() -> Vec<PathBuf> {
    let mut v = write_samples();
    v.extend(read_samples());
    v.push(fixture());
    v
}

#[test]
fn writer_matches_reference_on_samples_and_fixture() {
    for f in reference_documents() {
        let doc = xml::from_str(&fs::read_to_string(&f).unwrap()).unwrap();
        assert_same_as_reference(&f.display().to_string(), &doc.root);
    }
}

/// The same documents with escapes, multi-byte UTF-8, CR/TAB/LF and quotes
/// in every text and attribute value; then with a character XML 1.0 cannot
/// carry in text, and in attributes, so both writers fail at the same path.
#[test]
fn writer_matches_reference_on_spiced_documents() {
    let cases = [
        (
            " &amp; &lt;\u{e9}\u{4e2d}\u{1F600}&gt; \"q\" 'a' &#13;&#9;&#10;x\u{FFFD}&amp;",
            " &amp;&lt;&gt;&quot;&#9;&#10;&#13;\u{e9}\u{1F600}'x",
        ),
        ("12345678&amp;1234567&lt;\u{FF0C}", ""),
        (" a&#1;b", ""),
        ("", " a&#xFFFE;b"),
        ("&#xFFFF;", "&#31;"),
    ];
    for f in reference_documents() {
        let original = fs::read_to_string(&f).unwrap();
        for (i, (text, attr)) in cases.iter().enumerate() {
            let spiced = spice(&original, text, attr);
            let doc = xml::from_str(&spiced)
                .unwrap_or_else(|e| panic!("{} case {i}: spiced XML: {e}", f.display()));
            assert_same_as_reference(&format!("{} case {i}", f.display()), &doc.root);
        }
    }
}

#[test]
fn writer_matches_reference_on_bulk_works() {
    for f in write_samples() {
        let doc = xml::from_str(&fs::read_to_string(&f).unwrap()).unwrap();
        if let Root::Work(w) = &doc.root {
            let bulk = Root::Bulk(yorc::model::Bulk {
                items: vec![BulkItem::Work(w.clone()), BulkItem::Work(w.clone())],
            });
            assert_same_as_reference(&f.display().to_string(), &bulk);
        }
    }
}
