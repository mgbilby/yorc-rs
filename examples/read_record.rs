//! Reads an ORCID XML document (a saved `/record` response, a Public Data
//! File member, ...) and summarises it (feature `xml`, on by default).
//!
//! ```sh
//! cargo run --example read_record -- tests/data/0000-0002-2771-9344.xml
//! ```

use yorc::model::{AffiliationKind, Root};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: read_record FILE.xml")?;
    let bytes = std::fs::read(&path)?;
    let doc = yorc::xml::from_slice(&bytes)?;
    println!("root:  {}", doc.root_name);
    println!("iD:    {}", doc.orcid().unwrap_or("-"));

    if let Root::Record(record) = &doc.root {
        if let Some(name) = record.person.as_ref().and_then(|p| p.name.as_ref()) {
            println!(
                "name:  {} {}",
                name.given_names.as_deref().unwrap_or(""),
                name.family_name.as_deref().unwrap_or("")
            );
        }
        let keywords = record
            .person
            .as_ref()
            .and_then(|p| p.keywords.as_ref())
            .map_or(0, |k| k.items.len());
        println!("keywords: {keywords}");
    }

    if let Some(summary) = doc.activities_summary() {
        println!("works: {}", summary.work_summaries().count());
        for w in summary.work_summaries().take(5) {
            let title = w.title.as_ref().and_then(|t| t.title.as_deref());
            let year = w.publication_date.and_then(|d| d.year);
            println!(
                "  {} ({})",
                title.unwrap_or("untitled"),
                year.map_or("-".into(), |y| y.to_string())
            );
        }
        let fundings = summary.fundings.as_ref().map_or(0, |f| f.groups.len());
        println!("funding groups: {fundings}");
        for kind in AffiliationKind::ALL {
            let n: usize = summary
                .affiliations(kind)
                .map_or(0, |a| a.groups.iter().map(|g| g.summaries.len()).sum());
            if n > 0 {
                println!("{}: {n}", kind.container_element());
            }
        }
    }
    Ok(())
}
