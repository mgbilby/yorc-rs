//! Streams an ORCID Public Data File archive (`.tar.gz`) without unpacking
//! it and counts its records, works and parse errors (features `dump`, and
//! `xml`, on by default).
//!
//! ```sh
//! cargo run --release --example dump_count --features dump -- ORCID_2025_10_summaries_1.tar.gz
//! ```

use std::path::Path;

use yorc::dump;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: dump_count ARCHIVE.tar.gz")?;
    let mut archive = dump::open(Path::new(&path))?;
    let (mut members, mut records, mut works, mut errors) = (0u64, 0u64, 0u64, 0u64);
    for member in dump::records(&mut archive)? {
        let member = member?;
        members += 1;
        match member.parse() {
            Ok(doc) => {
                if doc.record().is_some() {
                    records += 1;
                }
                if let Some(a) = doc.activities_summary() {
                    works += a.work_summaries().count() as u64;
                }
            }
            Err(e) => {
                errors += 1;
                eprintln!("{}: {e}", member.path.display());
            }
        }
    }
    println!("{members} XML members, {records} records, {works} work summaries, {errors} errors");
    Ok(())
}
