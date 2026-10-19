//! Field-level differences between two snapshots of a record: yorc-duckdb's
//! `orcid_changes(old, new)` (`src/core/changes.cpp`) over this crate's rows.

use std::collections::HashMap;

use super::{Cell, ColType, Column, Row, Section, Tables, col, integer, varchar};
use crate::model::Document;

/// The sections [`changes`] compares, in output order (yorc-duckdb's
/// `DiffSections`).
pub const DIFF_SECTIONS: &[Section] = &[
    Section::Person,
    Section::PersonItems,
    Section::Works,
    Section::Fundings,
    Section::Affiliations,
    Section::PeerReviews,
    Section::ResearchResources,
    Section::History,
];

/// The columns of a [`Change`] row ([`Change::to_row`]), as yorc-duckdb's
/// `orcid_changes` returns them.
pub const CHANGE_COLUMNS: &[Column] = &[
    col("orcid", ColType::Varchar),
    col("section", ColType::Varchar),
    col("kind", ColType::Varchar),
    col("put_code", ColType::Bigint),
    col("change", ColType::Varchar),
    col("field", ColType::Varchar),
    col("old_value", ColType::Varchar),
    col("new_value", ColType::Varchar),
    col("label", ColType::Varchar),
];

/// Columns whose values never count as a change.
const IGNORED: [&str; 2] = ["source_hash", "item"];

/// Columns that label an item, in preference order (the first one the
/// section has and the row fills).
const LABELS: [&str; 8] = [
    "title",
    "role_title",
    "value",
    "subject_name",
    "org_name",
    "convening_org_name",
    "given_names",
    "creation_method",
];

/// How an item changed.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChangeKind {
    /// In the new snapshot only.
    Added,
    /// In the old snapshot only.
    Removed,
    /// In both, with this field changed.
    Modified,
}

impl ChangeKind {
    /// `added`, `removed` or `modified`.
    pub fn as_str(self) -> &'static str {
        match self {
            ChangeKind::Added => "added",
            ChangeKind::Removed => "removed",
            ChangeKind::Modified => "modified",
        }
    }
}

/// One difference between two snapshots (a row of yorc-duckdb's
/// `orcid_changes`).
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// The record's iD (the row's `orcid`).
    pub orcid: Option<String>,
    /// The section the item belongs to.
    pub section: Section,
    /// The item kind where the section mixes kinds (`affiliation_type` of
    /// affiliations, `kind` of person items).
    pub kind: Option<String>,
    /// The item's put-code (`None` for person and history).
    pub put_code: Option<i64>,
    /// Added, removed or modified.
    pub change: ChangeKind,
    /// The changed column (modified only).
    pub field: Option<&'static str>,
    /// The column's old value, as text (modified only).
    pub old_value: Option<String>,
    /// The column's new value, as text (modified only).
    pub new_value: Option<String>,
    /// Title, role, value, ... of the item (the new side's, except for a
    /// removed item).
    pub label: Option<String>,
}

impl Change {
    /// The change as a row of [`CHANGE_COLUMNS`].
    pub fn to_row(&self) -> Row {
        vec![
            varchar(self.orcid.as_deref()),
            varchar(Some(self.section.name())),
            varchar(self.kind.as_deref()),
            integer(self.put_code),
            varchar(Some(self.change.as_str())),
            varchar(self.field),
            varchar(self.old_value.as_deref()),
            varchar(self.new_value.as_deref()),
            varchar(self.label.as_deref()),
        ]
    }
}

/// The differences between two snapshots of a record, as yorc-duckdb's
/// `orcid_changes(old, new)` lists them. `None` is no snapshot: every item of
/// the other side is then added (or removed).
///
/// Items are matched on section, kind and put-code (person and history, one
/// item each, on the section alone); a key repeated within a snapshot pairs
/// up in order. Every column but `source_hash` and `item` is compared, so a
/// modified item gives one change per differing column, in column order.
/// Rows come section by section ([`DIFF_SECTIONS`]); within a section,
/// modified and added items in the new snapshot's order, then removed items
/// in the old one's.
///
/// Values are text, as yorc-duckdb's: integers in decimal, fuzzy-date parts
/// zero-padded as ORCID writes them (`*_year` to 4 digits, `*_month` and
/// `*_day` to 2), booleans `true`/`false`. Known difference (outside what
/// ORCID writes): yorc-duckdb compares the document's text, so `1` and `01`,
/// `007` and `7`, `1` and `true` differ there and are equal here, and a
/// `display-index` of `-1` is NULL here and `-1` there.
///
/// ```
/// use yorc::rows::{changes, ChangeKind, Section};
///
/// let old = yorc::xml::from_str(r#"<keyword:keywords xmlns:keyword="http://www.orcid.org/ns/keyword">
///   <keyword:keyword put-code="1"><keyword:content>vents</keyword:content></keyword:keyword>
/// </keyword:keywords>"#)?;
/// let new = yorc::xml::from_str(r#"<keyword:keywords xmlns:keyword="http://www.orcid.org/ns/keyword">
///   <keyword:keyword put-code="1"><keyword:content>deep-sea vents</keyword:content></keyword:keyword>
/// </keyword:keywords>"#)?;
/// let c = changes(Some(&old), Some(&new));
/// assert_eq!(c.len(), 1);
/// assert_eq!(c[0].section, Section::PersonItems);
/// assert_eq!(c[0].change, ChangeKind::Modified);
/// assert_eq!(c[0].field, Some("value"));
/// assert_eq!(c[0].old_value.as_deref(), Some("vents"));
/// assert_eq!(c[0].label.as_deref(), Some("deep-sea vents"));
/// assert_eq!(changes(None, Some(&new))[0].change, ChangeKind::Added);
/// # Ok::<(), yorc::Error>(())
/// ```
pub fn changes(old: Option<&Document>, new: Option<&Document>) -> Vec<Change> {
    let empty = Tables::default();
    let old_t = old.map(|d| Tables::from_document(d, None));
    let new_t = new.map(|d| Tables::from_document(d, None));
    let old_t = old_t.as_ref().unwrap_or(&empty);
    let new_t = new_t.as_ref().unwrap_or(&empty);
    let mut out = Vec::new();
    for &section in DIFF_SECTIONS {
        diff_section(section, old_t.get(section), new_t.get(section), &mut out);
    }
    out
}

/// A row's match key: kind, put-code (or `\x1e`) and its occurrence among
/// the rows with the same kind and put-code, as yorc-duckdb's `KeyRows`.
struct Keyed {
    key: String,
    kind: String,
    put_code: Option<i64>,
}

fn key_rows(rows: &[Row], kind_col: Option<usize>, put_col: Option<usize>) -> Vec<Keyed> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    rows.iter()
        .map(|row| {
            let kind = kind_col
                .and_then(|c| row[c].as_str())
                .unwrap_or_default()
                .to_string();
            let put_code = put_col.and_then(|c| row[c].as_i64());
            let mut base = kind.clone();
            base.push('\u{1f}');
            match put_code {
                Some(p) => base.push_str(&p.to_string()),
                None => base.push('\u{1e}'),
            }
            let n = seen.entry(base.clone()).or_insert(0);
            let key = format!("{base}\u{1f}{n}");
            *n += 1;
            Keyed {
                key,
                kind,
                put_code,
            }
        })
        .collect()
}

/// A cell as yorc-duckdb's change rows show it (see [`changes`]).
fn cell_text(column: &str, cell: &Cell) -> Option<String> {
    match cell {
        Cell::Null => None,
        Cell::Varchar(s) => Some(s.clone()),
        Cell::Integer(v) => Some(if column.ends_with("_year") {
            format!("{v:04}")
        } else if column.ends_with("_month") || column.ends_with("_day") {
            format!("{v:02}")
        } else {
            v.to_string()
        }),
        Cell::Boolean(b) => Some(b.to_string()),
    }
}

fn diff_section(section: Section, old: &[Row], new: &[Row], out: &mut Vec<Change>) {
    let cols = section.columns();
    let kind_col = cols
        .iter()
        .position(|c| c.name == "affiliation_type" || c.name == "kind");
    let put_col = section.column_index("put_code");
    let orcid_col = section.column_index("orcid");
    let label_cols: Vec<usize> = LABELS
        .iter()
        .filter_map(|n| section.column_index(n))
        .collect();
    let label = |row: &Row| -> Option<String> {
        label_cols
            .iter()
            .find(|&&c| !row[c].is_null())
            .and_then(|&c| cell_text(cols[c].name, &row[c]))
    };
    let base = |k: &Keyed, row: &Row, change: ChangeKind| Change {
        orcid: orcid_col
            .and_then(|c| row[c].as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        section,
        kind: (!k.kind.is_empty()).then(|| k.kind.clone()),
        put_code: k.put_code,
        change,
        field: None,
        old_value: None,
        new_value: None,
        label: label(row),
    };

    let old_keyed = key_rows(old, kind_col, put_col);
    let new_keyed = key_rows(new, kind_col, put_col);
    let old_by_key: HashMap<&str, usize> = old_keyed
        .iter()
        .enumerate()
        .map(|(i, k)| (k.key.as_str(), i))
        .collect();
    let mut matched = vec![false; old.len()];
    for (k, n) in new_keyed.iter().zip(new) {
        let Some(&i) = old_by_key.get(k.key.as_str()) else {
            out.push(base(k, n, ChangeKind::Added));
            continue;
        };
        matched[i] = true;
        let o = &old[i];
        for (c, column) in cols.iter().enumerate() {
            if IGNORED.contains(&column.name) || o[c] == n[c] {
                continue;
            }
            let mut change = base(k, n, ChangeKind::Modified);
            change.field = Some(column.name);
            change.old_value = cell_text(column.name, &o[c]);
            change.new_value = cell_text(column.name, &n[c]);
            out.push(change);
        }
    }
    for ((k, o), m) in old_keyed.iter().zip(old).zip(&matched) {
        if !m {
            out.push(base(k, o, ChangeKind::Removed));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_text_as_orcid_writes_them() {
        assert_eq!(
            cell_text("pub_month", &Cell::Integer(2)).as_deref(),
            Some("02")
        );
        assert_eq!(
            cell_text("start_day", &Cell::Integer(31)).as_deref(),
            Some("31")
        );
        assert_eq!(
            cell_text("end_year", &Cell::Integer(987)).as_deref(),
            Some("0987")
        );
        assert_eq!(
            cell_text("put_code", &Cell::Integer(7)).as_deref(),
            Some("7")
        );
        assert_eq!(
            cell_text("claimed", &Cell::Boolean(true)).as_deref(),
            Some("true")
        );
        assert_eq!(cell_text("title", &Cell::Null), None);
    }

    #[test]
    fn repeated_keys_pair_up_in_order() {
        let rows: Vec<Row> = vec![
            vec![Cell::Varchar("keyword".into()), Cell::Integer(1)],
            vec![Cell::Varchar("keyword".into()), Cell::Integer(1)],
            vec![Cell::Null, Cell::Null],
        ];
        let k = key_rows(&rows, Some(0), Some(1));
        assert_eq!(k[0].key, "keyword\u{1f}1\u{1f}0");
        assert_eq!(k[1].key, "keyword\u{1f}1\u{1f}1");
        assert_eq!(k[2].key, "\u{1f}\u{1e}\u{1f}0");
        assert_eq!(k[2].kind, "");
        assert_eq!(k[2].put_code, None);
    }
}
