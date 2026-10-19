//! Pins WALKTHROUGH.md to `examples/walkthrough.rs`: every cell but the
//! network one (9) is run and its output compared with the cell's
//! "Expected output" block, each cell's code block must be the body of its
//! function, and the cell list must match.
//!
//! Cells 0 and 8 read the dump fixture (`tests/data/dump/`); if it is
//! missing (a checkout without it), they are skipped with a message.
#![cfg(feature = "full")]

#[allow(dead_code)]
#[path = "../examples/walkthrough.rs"]
mod walkthrough;

/// One `## Cell N: Title` section of WALKTHROUGH.md.
struct DocCell {
    number: u32,
    title: String,
    code: String,
    expected: String,
}

fn read_lf(rel: &str) -> String {
    walkthrough::read_text(rel).unwrap().replace("\r\n", "\n")
}

/// The text of the first block fenced with `fence` (e.g. "```text\n") in `s`,
/// with a final line end.
fn fenced(s: &str, fence: &str, what: &str) -> String {
    let start = s
        .find(fence)
        .unwrap_or_else(|| panic!("{what}: no {fence:?} block"))
        + fence.len();
    let len = s[start..]
        .find("\n```")
        .unwrap_or_else(|| panic!("{what}: unterminated block"));
    format!("{}\n", &s[start..start + len])
}

fn doc_cells() -> Vec<DocCell> {
    let md = read_lf("WALKTHROUGH.md");
    md.split("\n## Cell ")
        .skip(1)
        .map(|part| {
            let (head, rest) = part.split_once('\n').unwrap();
            let (number, title) = head
                .split_once(": ")
                .unwrap_or_else(|| panic!("heading {head:?}"));
            let number: u32 = number.parse().unwrap();
            let what = format!("cell {number}");
            // the code (`rust`), then its expected output (`text`)
            let code = fenced(rest, "```rust\n", &what);
            let expected = fenced(rest, "```text\n", &what);
            DocCell {
                number,
                title: title.to_string(),
                code,
                expected,
            }
        })
        .collect()
}

fn doc_cell(n: u32) -> DocCell {
    doc_cells()
        .into_iter()
        .find(|c| c.number == n)
        .unwrap_or_else(|| panic!("WALKTHROUGH.md has no cell {n}"))
}

/// The body of `pub fn cell_N(...)` in the example, unindented by one level.
fn function_body(src: &str, n: u32) -> String {
    let sig = format!("pub fn cell_{n}(");
    let at = src.find(&sig).unwrap_or_else(|| panic!("no fn cell_{n}"));
    let start = at + src[at..].find("{\n").unwrap() + 2;
    let len = src[start..].find("\n}\n").unwrap() + 1;
    src[start..start + len]
        .lines()
        .map(|l| l.strip_prefix("    ").unwrap_or(l))
        .map(|l| format!("{l}\n"))
        .collect()
}

fn dump_fixture_present() -> bool {
    walkthrough::path(walkthrough::DUMP).is_file()
}

/// Runs cell `n` offline and compares its output with WALKTHROUGH.md.
fn check(n: u32) {
    if (n == 0 || n == 8) && !dump_fixture_present() {
        eprintln!("cell {n} skipped: {} is not there", walkthrough::DUMP);
        return;
    }
    let got = walkthrough::run(n, false).unwrap_or_else(|e| panic!("cell {n}: {e}"));
    let want = doc_cell(n).expected;
    assert_eq!(
        got, want,
        "cell {n}: output differs from WALKTHROUGH.md\n--- got ---\n{got}--- expected ---\n{want}"
    );
}

#[test]
fn the_document_lists_every_cell_in_order() {
    let doc: Vec<(u32, String)> = doc_cells()
        .into_iter()
        .map(|c| (c.number, c.title))
        .collect();
    let code: Vec<(u32, String)> = walkthrough::CELLS
        .iter()
        .map(|&(n, t)| (n, t.to_string()))
        .collect();
    assert_eq!(doc, code);
}

#[test]
fn code_blocks_are_the_cell_bodies() {
    let src = read_lf("examples/walkthrough.rs");
    for cell in doc_cells() {
        assert_eq!(
            cell.code,
            function_body(&src, cell.number),
            "cell {}: the code block is not the body of cell_{}",
            cell.number,
            cell.number
        );
    }
}

#[test]
fn cell_0_setup() {
    check(0);
}

#[test]
fn cell_1_ids() {
    check(1);
}

#[test]
fn cell_2_record_summary() {
    check(2);
}

#[test]
fn cell_3_newest_works() {
    check(3);
}

#[test]
fn cell_4_json() {
    check(4);
}

#[test]
fn cell_5_section_tables() {
    check(5);
}

#[test]
fn cell_6_contributors() {
    check(6);
}

#[test]
fn cell_7_affiliation_timeline() {
    check(7);
}

#[test]
fn cell_8_dump() {
    check(8);
}

// Cell 9 reaches ORCID (one request, paced at 45 s) and is not run here.
