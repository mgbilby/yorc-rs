//! `yorc`: Cultivate your research identity and networks. The ORCID message
//! schema v3.0 for Rust.
//!
//! yorc-rs is an independent open-source project. It is not affiliated with,
//! endorsed by or sponsored by ORCID, Inc. It does not provide access to the
//! ORCID Sandbox or the ORCID Member API: its optional client (feature
//! `blocking`) reads only ORCID's Public API (`pub.orcid.org`). Bulk data
//! should come from ORCID's Public Data File, read with [`dump`] (feature
//! `dump`).
//!
//! * [`id`]: [`id::OrcidId`], ISO 7064 MOD 11-2, the same normalisation rules
//!   as yorc-duckdb's `orcid_normalize`.
//! * [`model`]: typed structs for every data-bearing element and attribute of
//!   the v3.0 record XSDs (works, fundings, seven affiliation kinds, peer
//!   reviews, research resources, person and its items, history, record,
//!   bulk, error, search), and peer-review group ids (group-id-3.0).
//! * [`xml`]: ORCID XML → [`model::Document`] (quick-xml; lenient about unknown
//!   elements, namespaces matched by local name), and model → ORCID XML
//!   ([`xml::to_string`], exact and deterministic, with ORCID's prefixes).
//! * [`json`] (feature `json`): ORCID's JSON form over the same model, read
//!   and write.
//! * [`rows`] (feature `rows`): the model flattened into yorc-duckdb's eleven
//!   record section tables, its `search_results` and `errors` tables and
//!   `research_resource_hosts`, same names, columns, order and types; opt-in,
//!   its `timeline` and `group_ids` tables ([`rows::Extra`]) and its
//!   snapshot diff `orcid_changes` ([`rows::changes`]).
//! * [`dump`] (feature `dump`): streaming Public Data File reader, with a
//!   per-member size limit (64 MiB by default), and a seeded, reproducible
//!   reservoir sampler.
//! * [`hash`] (feature `hash`): `source_hash` (SHA-256 hex).
//! * [`blocking`] (feature `blocking`): ORCID Public API reads
//!   (`pub.orcid.org`) of any public record by iD, and search, anonymous or
//!   with an optional 2-legged `/read-public` token, through a process-wide
//!   rate limiter (one request every 45 s) with retries. No Member API, no
//!   Sandbox, no sign-in OAuth, no writes.
//!
//! Default features: `id`, `model`, `xml`. `full` turns on everything.
//!
//! ```
//! # #[cfg(feature = "xml")] {
//! // default features: id, model, xml
//! let doc = yorc::xml::from_str(
//!     r#"<record:record path="/0000-0002-1825-0097" xmlns:record="http://www.orcid.org/ns/record"/>"#,
//! )
//! .unwrap();
//! assert_eq!(doc.orcid(), Some("0000-0002-1825-0097"));
//! # }
//! ```
//!
//! Runnable programs for every module are in the repository's `examples/`
//! directory (`cargo run --example read_record -- FILE.xml`, ...).

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod error;

pub use error::{Error, Result};

#[cfg(feature = "id")]
pub mod id;

#[cfg(feature = "hash")]
pub mod hash;

#[cfg(feature = "model")]
pub mod model;

#[cfg(feature = "xml")]
pub mod xml;

#[cfg(feature = "rows")]
pub mod rows;

#[cfg(feature = "dump")]
pub mod dump;

#[cfg(feature = "json")]
pub mod json;

#[cfg(feature = "blocking")]
pub mod blocking;

#[cfg(feature = "id")]
pub use id::OrcidId;

#[cfg(feature = "model")]
pub use model::{Document, Record, Root};

// The README's Rust blocks, compiled as doctests (not part of the API).
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
