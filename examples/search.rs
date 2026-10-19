//! One page of ORCID search results, with names (feature `blocking`). The
//! query is Solr syntax, e.g. `family-name:Carberry`.
//!
//! ```sh
//! cargo run --example search --features blocking -- 'family-name:Carberry' 0 10
//! ```
//!
//! Searches ORCID's Public API (`pub.orcid.org`; no Member API, no
//! Sandbox). Anonymous by default; set `ORCID_TOKEN` to send a
//! `/read-public` bearer token. Search is paced like every request to ORCID
//! (one every 45 s) and is not a bulk-export path: for bulk data use the
//! Public Data File (`dump`).

use yorc::blocking::{Access, Client, Config, SCOPE_READ_PUBLIC, Token};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rest: Vec<String> = std::env::args().skip(1).collect();
    let query = rest.first().ok_or("usage: search QUERY [START [ROWS]]")?;
    let start: u32 = rest.get(1).map_or(Ok(0), |s| s.parse())?;
    let rows: u32 = rest.get(2).map_or(Ok(10), |s| s.parse())?;
    let access = Access::Public;
    let token = std::env::var("ORCID_TOKEN")
        .ok()
        .map(|t| Token::new(access.realm(), t, SCOPE_READ_PUBLIC));

    let client = Client::new(Config::new(access));
    let page = client.expanded_search(query, start, rows, token.as_ref())?;
    println!("{} found", page.num_found.unwrap_or(0));
    for r in &page.results {
        println!(
            "{}  {} {}  {}",
            r.orcid_id.as_deref().unwrap_or("-"),
            r.given_names.as_deref().unwrap_or(""),
            r.family_names.as_deref().unwrap_or(""),
            r.institution_names.join("; ")
        );
    }
    Ok(())
}
