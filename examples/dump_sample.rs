//! Draws a seeded, reproducible sample of iDs from an ORCID Public Data File
//! archive (feature `dump`). Members that are not kept are never read.
//!
//! ```sh
//! cargo run --release --example dump_sample --features dump -- ORCID_2025_10_summaries_1.tar.gz 10 42
//! ```

use std::path::Path;

use yorc::dump::{self, Reservoir, Slot};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args
        .first()
        .ok_or("usage: dump_sample ARCHIVE.tar.gz [N [SEED]]")?;
    let n: usize = args.get(1).map_or(Ok(10), |s| s.parse())?;
    let seed: u64 = args.get(2).map_or(Ok(0), |s| s.parse())?;

    let mut archive = dump::open(Path::new(path))?;
    let mut reservoir = Reservoir::new(n, seed);
    for entry in archive.entries()? {
        let entry = entry?;
        let entry_path = entry.path()?.into_owned();
        let Some(stem) = dump::xml_entry_stem(&entry_path) else {
            continue;
        };
        // decide before reading: skipped members cost no parsing
        let slot = reservoir.offer();
        if slot != Slot::Skip {
            reservoir.put(slot, stem.to_string());
        }
    }
    eprintln!("{} XML members seen", reservoir.seen());
    for id in reservoir.into_items() {
        println!("{id}");
    }
    Ok(())
}
