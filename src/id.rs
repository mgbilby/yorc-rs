//! ORCID iDs: normalization, ISO 7064 MOD 11-2 validation, [`OrcidId`].
//!
//! Mirrors `orcid::NormalizeId` in yorc-duckdb (`src/core/id.cpp`) byte for
//! byte, so it agrees with the extension's `orcid_valid`/`orcid_normalize`:
//! leading ASCII whitespace is skipped, one of the prefixes
//! `https://orcid.org/`, `http://orcid.org/`, `orcid.org/` (case-sensitive) is
//! stripped, then hyphens and ASCII whitespace are ignored anywhere, the
//! remaining bytes are upper-cased and must be exactly 16: 15 digits plus a
//! check character (digit or `X`).

use std::fmt;
use std::str::FromStr;

const PREFIXES: [&[u8]; 3] = [b"https://orcid.org/", b"http://orcid.org/", b"orcid.org/"];

/// C `isspace` in the "C" locale.
#[inline]
fn is_c_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// ISO 7064 MOD 11-2 check character for the first 15 digits.
///
/// ```
/// assert_eq!(yorc::id::check_char(b"000000021825009"), Some(b'7'));
/// assert_eq!(yorc::id::check_char(b"000000021694233"), Some(b'X'));
/// assert_eq!(yorc::id::check_char(b"0000"), None);
/// ```
#[inline]
pub fn check_char(digits: &[u8]) -> Option<u8> {
    if digits.len() != 15 {
        return None;
    }
    let mut total: u32 = 0;
    for &c in digits {
        if !c.is_ascii_digit() {
            return None;
        }
        total = (total + u32::from(c - b'0')) * 2;
    }
    let digit = (12 - total % 11) % 11;
    Some(if digit == 10 {
        b'X'
    } else {
        b'0' + digit as u8
    })
}

/// Canonical `XXXX-XXXX-XXXX-XXXX`, or `None` if the input is not a valid iD.
///
/// ```
/// use yorc::id::normalize;
///
/// assert_eq!(normalize("https://orcid.org/000000021694233x").as_deref(), Some("0000-0002-1694-233X"));
/// assert_eq!(normalize(" 0000 0002 1825 0097 ").as_deref(), Some("0000-0002-1825-0097"));
/// assert_eq!(normalize("0000-0002-1825-0098"), None); // wrong check digit
/// ```
pub fn normalize(input: &str) -> Option<String> {
    let b = input.as_bytes();
    let mut pos = 0;
    while pos < b.len() && is_c_space(b[pos]) {
        pos += 1;
    }
    for p in PREFIXES {
        if b[pos..].starts_with(p) {
            pos += p.len();
            break;
        }
    }

    let mut chars = [0u8; 16];
    let mut n = 0;
    for &c in &b[pos..] {
        if c == b'-' || is_c_space(c) {
            continue;
        }
        if n == 16 {
            return None;
        }
        chars[n] = c.to_ascii_uppercase();
        n += 1;
    }
    if n != 16 {
        return None;
    }
    if check_char(&chars[..15])? != chars[15] {
        return None;
    }

    let mut out = String::with_capacity(19);
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && i % 4 == 0 {
            out.push('-');
        }
        out.push(char::from(c));
    }
    Some(out)
}

/// `true` if [`normalize`] accepts the input.
///
/// ```
/// assert!(yorc::id::is_valid("0000-0002-1825-0097"));
/// assert!(!yorc::id::is_valid("0000-0002-1825-009"));
/// ```
pub fn is_valid(input: &str) -> bool {
    normalize(input).is_some()
}

/// A valid ORCID iD in canonical `XXXX-XXXX-XXXX-XXXX` form.
///
/// ```
/// use yorc::OrcidId;
///
/// let id: OrcidId = "orcid.org/0000000218250097".parse()?;
/// assert_eq!(id.as_str(), "0000-0002-1825-0097");
/// assert_eq!(id.uri(), "https://orcid.org/0000-0002-1825-0097");
/// assert!(OrcidId::parse("0000-0002-1825-0098").is_err());
/// # Ok::<(), yorc::id::InvalidOrcidId>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OrcidId(String);

/// Error from [`OrcidId::parse`]: the input (verbatim) is not a valid iD.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidOrcidId(pub String);

impl fmt::Display for InvalidOrcidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid ORCID iD: {:?}", self.0)
    }
}

impl std::error::Error for InvalidOrcidId {}

impl OrcidId {
    /// Accepts the forms [`normalize`] accepts (URL prefixes, missing
    /// hyphens, lowercase `x`, surrounding whitespace).
    pub fn parse(input: &str) -> Result<OrcidId, InvalidOrcidId> {
        match normalize(input) {
            Some(s) => Ok(OrcidId(s)),
            None => Err(InvalidOrcidId(input.to_string())),
        }
    }

    /// Canonical `XXXX-XXXX-XXXX-XXXX`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `https://orcid.org/XXXX-XXXX-XXXX-XXXX`.
    pub fn uri(&self) -> String {
        format!("https://orcid.org/{}", self.0)
    }

    /// The canonical `XXXX-XXXX-XXXX-XXXX` text, by value.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for OrcidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for OrcidId {
    type Err = InvalidOrcidId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        OrcidId::parse(s)
    }
}

impl AsRef<str> for OrcidId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_ids() {
        for id in [
            "0000-0003-0100-6634",
            "0000-0002-1694-233X",
            "0000-0002-1825-0097",
            "0000-0001-5109-3700",
        ] {
            assert_eq!(normalize(id).as_deref(), Some(id), "{id}");
        }
    }

    #[test]
    fn invalid_checksum() {
        assert_eq!(normalize("0000-0003-0100-6635"), None);
        assert!(!is_valid("0000-0002-1825-0098"));
    }

    #[test]
    fn extension_test_vectors() {
        // yorc-duckdb/test/sql/orcid.test
        assert!(is_valid("0000-0003-0100-6634"));
        assert!(!is_valid("0000-0003-0100-6635"));
        assert_eq!(
            normalize("https://orcid.org/000000021694233x").as_deref(),
            Some("0000-0002-1694-233X")
        );
    }

    #[test]
    fn prefixes_and_whitespace() {
        let want = Some("0000-0002-1825-0097");
        assert_eq!(
            normalize("http://orcid.org/0000-0002-1825-0097").as_deref(),
            want
        );
        assert_eq!(normalize("orcid.org/0000000218250097").as_deref(), want);
        assert_eq!(normalize("  0000 0002 1825 0097\n").as_deref(), want);
        assert_eq!(normalize("\t0000-0002-1825-0097").as_deref(), want);
        // prefixes are case-sensitive, as in the extension
        assert_eq!(normalize("HTTPS://ORCID.ORG/0000-0002-1825-0097"), None);
        // prefix only stripped at the start
        assert_eq!(normalize("x https://orcid.org/0000-0002-1825-0097"), None);
    }

    #[test]
    fn wrong_lengths_and_chars() {
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("0000-0002-1825-009"), None);
        assert_eq!(normalize("0000-0002-1825-00971"), None);
        assert_eq!(normalize("X000-0002-1825-0097"), None);
        assert_eq!(normalize("0000-0002-1825-009\u{e9}"), None);
        assert_eq!(normalize("https://orcid.org/"), None);
    }

    #[test]
    fn orcid_id_newtype() {
        let id: OrcidId = "https://orcid.org/000000021694233x".parse().unwrap();
        assert_eq!(id.as_str(), "0000-0002-1694-233X");
        assert_eq!(id.to_string(), "0000-0002-1694-233X");
        assert_eq!(id.uri(), "https://orcid.org/0000-0002-1694-233X");
        assert_eq!(
            OrcidId::parse("0000-0003-0100-6635"),
            Err(InvalidOrcidId("0000-0003-0100-6635".to_string()))
        );
    }

    #[test]
    fn check_char_vectors() {
        assert_eq!(check_char(b"000000021694233"), Some(b'X'));
        assert_eq!(check_char(b"000000030100663"), Some(b'4'));
        assert_eq!(check_char(b"00000003010066"), None);
    }
}
