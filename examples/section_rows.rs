//! Flattens an ORCID XML document into the yorc-duckdb section tables (the
//! eleven record sections, `search_results`, `errors` and
//! `research_resource_hosts`) and prints their
//! sizes and the first row of each (feature `rows`).
//!
//! ```sh
//! cargo run --example section_rows --features rows -- tests/data/0000-0002-2771-9344.xml
//! ```

use yorc::rows::{Cell, Section, Tables};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: section_rows FILE.xml")?;
    let tables = Tables::from_xml(&std::fs::read(&path)?)?;
    for &section in Section::ALL {
        let rows = tables.get(section);
        println!(
            "{}: {} rows, {} columns",
            section.name(),
            rows.len(),
            section.columns().len()
        );
        if let Some(row) = rows.first() {
            for (col, cell) in section.columns().iter().zip(row) {
                let text = match cell {
                    Cell::Null => continue,
                    Cell::Varchar(s) => format!("{s:?}"),
                    Cell::Integer(i) => i.to_string(),
                    Cell::Boolean(b) => b.to_string(),
                    other => format!("{other:?}"),
                };
                println!("    {} = {text}", col.name);
            }
        }
    }
    Ok(())
}
