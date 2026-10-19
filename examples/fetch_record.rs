//! Reads one record by iD and summarises it (feature `blocking`).
//!
//! ```sh
//! cargo run --example fetch_record --features blocking -- 0000-0002-1825-0097
//! ```
//!
//! Reads ORCID's Public API (`pub.orcid.org`), the only API this crate
//! reads (no Member API, no Sandbox). Anonymous by default; set
//! `ORCID_TOKEN` to send a `/read-public` bearer token. Requests to ORCID
//! are paced at one every 45 s: this is for occasional single-record
//! retrieval; for bulk data use the Public Data File (`dump`).

use yorc::blocking::{Access, Client, Config, SCOPE_READ_PUBLIC, Token};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut id = "0000-0002-1825-0097".to_string();
    for a in std::env::args().skip(1) {
        if a.starts_with("--") {
            return Err(format!("unknown option {a}").into());
        }
        id = a;
    }
    let access = Access::Public;
    let token = std::env::var("ORCID_TOKEN")
        .ok()
        .map(|t| Token::new(access.realm(), t, SCOPE_READ_PUBLIC));
    let client = Client::new(Config::new(access));

    let record = client.record(&id, token.as_ref())?;
    if let Some(name) = record.person.as_ref().and_then(|p| p.name.as_ref()) {
        println!(
            "{} {}",
            name.given_names.as_deref().unwrap_or(""),
            name.family_name.as_deref().unwrap_or("")
        );
    }
    let summary = record.activities_summary.unwrap_or_default();
    println!("{} works", summary.work_summaries().count());
    for w in summary.work_summaries().take(10) {
        let title = w.title.as_ref().and_then(|t| t.title.as_deref());
        println!(
            "  [{}] {}",
            w.meta.put_code.unwrap_or_default(),
            title.unwrap_or("untitled")
        );
    }
    Ok(())
}
