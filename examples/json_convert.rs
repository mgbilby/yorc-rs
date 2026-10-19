//! Converts an ORCID XML document to ORCID JSON, or (with `--to-xml KIND`)
//! a JSON document of that kind back to XML (features `xml`, `json`).
//!
//! ```sh
//! cargo run --example json_convert --features json -- tests/data/0000-0002-2771-9344.xml
//! cargo run --example json_convert --features json -- --to-xml record record.json
//! ```

use std::io::Write;

/// Writes `s` to stdout. A closed pipe (`| head`) ends the output quietly
/// instead of panicking as `print!` does.
fn emit(s: &str) -> std::io::Result<()> {
    match std::io::stdout().lock().write_all(s.as_bytes()) {
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        r => r,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [flag, kind, path] if flag == "--to-xml" => {
            let doc = yorc::json::from_slice(kind, &std::fs::read(path)?)?;
            emit(&yorc::xml::to_string(&doc.root)?)?;
        }
        [path] => {
            let doc = yorc::xml::from_slice(&std::fs::read(path)?)?;
            emit(&yorc::json::to_string(&doc.root)?)?;
        }
        _ => {
            return Err(format!(
                "usage: json_convert FILE.xml | json_convert --to-xml KIND FILE.json\n\
                 kinds: {}",
                yorc::json::KINDS.join(", ")
            )
            .into());
        }
    }
    Ok(())
}
