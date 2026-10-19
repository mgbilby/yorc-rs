//! Normalises and checks ORCID iDs (feature `id`).
//!
//! ```sh
//! cargo run --example orcid_id -- https://orcid.org/000000021694233x 0000-0002-1825-0098
//! ```

use yorc::OrcidId;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let inputs = if args.is_empty() {
        vec!["0000-0002-1825-0097".to_string()]
    } else {
        args
    };
    for input in inputs {
        match OrcidId::parse(&input) {
            Ok(id) => println!("{input:?}: valid, {id} ({})", id.uri()),
            Err(e) => println!("{input:?}: {e}"),
        }
    }
}
