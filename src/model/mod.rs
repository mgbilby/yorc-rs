//! Typed model of the ORCID message schema v3.0
//! (`reference/orcid-model/record_3.0` and `common_3.0`), plus the
//! peer-review group ids of `group-id-3.0` ([`GroupIdRecord`]).
//!
//! Conventions:
//! * Every data-bearing element and attribute of the record XSDs has a field.
//!   Elements with `maxOccurs="1"` are `Option`, repeated ones are `Vec`.
//! * Text is stored trimmed of ASCII space, tab, CR and LF; empty text is
//!   `None` (the same rule as yorc-duckdb).
//! * Timestamps (`xs:dateTime`) are kept as the text ORCID sent.
//! * Integers (put-codes, fuzzy-date parts, display index) are parsed with
//!   yorc-duckdb's rule: ASCII digits only, at most 18 (put-code) or 9 digits;
//!   anything else is `None`.
//! * Closed vocabularies are enums with an `Other(String)` fallback, so new
//!   ORCID values never fail a parse; `as_str()` returns the text verbatim.
//! * Summary and full forms are separate types (`WorkSummary`/`Work`, ...).

/// Defines a vocabulary enum with an `Other(String)` fallback, `as_str()`,
/// `From<&str>` and `Display`. The enum is `#[non_exhaustive]` and each
/// variant's doc is its XML text.
macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        $name:ident { $( $variant:ident => $text:literal ),+ $(,)? }
    ) => {
        $(#[$meta])*
        ///
        /// Non-exhaustive: ORCID may add values; a value this version does not
        /// know reads as `Other(..)`, and a later version may give it a variant.
        #[non_exhaustive]
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $(
                #[doc = concat!("`", $text, "`")]
                $variant,
            )+
            /// A value not in the known list, kept verbatim.
            Other(String),
        }

        impl $name {
            /// The value as written in ORCID XML.
            pub fn as_str(&self) -> &str {
                match self {
                    $( $name::$variant => $text, )+
                    $name::Other(s) => s.as_str(),
                }
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                match s {
                    $( $text => $name::$variant, )+
                    other => $name::Other(other.to_string()),
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

pub mod activities;
pub mod common;
pub mod group_id;
pub mod person;
pub mod record;

pub use activities::*;
pub use common::*;
pub use group_id::*;
pub use person::*;
pub use record::*;
