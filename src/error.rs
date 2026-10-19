//! [`Error`]: what reading and writing ORCID documents can fail with.

use std::fmt;

/// Errors from reading and writing ORCID documents (XML or JSON).
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Malformed XML. `position` is the byte offset where the reader stopped.
    Xml {
        /// What the XML reader reported.
        message: String,
        /// Byte offset in the input.
        position: u64,
    },
    /// The input is not UTF-8; `valid_up_to` bytes were.
    Utf8 {
        /// Length of the valid UTF-8 prefix, in bytes.
        valid_up_to: usize,
    },
    /// The document has no root element.
    NoRootElement,
    /// The model cannot be written: a character XML 1.0 cannot carry, a root
    /// this crate has no element for, content the XSD forbids at that place,
    /// or (JSON) a timestamp that is not an `xs:dateTime`. `path` is the
    /// element path.
    Unwritable {
        /// Element path of the value (e.g. `funding/contributors/contributor`).
        path: String,
        /// Why it cannot be written.
        reason: String,
    },
    /// Malformed JSON, or JSON that is not the expected ORCID shape (feature
    /// `json`).
    Json {
        /// What went wrong, with the JSON path where known.
        message: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Xml { message, position } => {
                write!(f, "malformed XML at byte {position}: {message}")
            }
            Error::Utf8 { valid_up_to } => {
                write!(f, "input is not UTF-8 (valid up to byte {valid_up_to})")
            }
            Error::NoRootElement => f.write_str("document has no root element"),
            Error::Unwritable { path, reason } => write!(f, "cannot write {path}: {reason}"),
            Error::Json { message } => write!(f, "JSON: {message}"),
        }
    }
}

impl std::error::Error for Error {}

/// `Result` with this crate's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
