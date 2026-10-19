//! Streaming access to ORCID Public Data File archives (`*.tar.gz`), and a
//! seeded, reproducible reservoir sampler ([`reservoir`]).
//!
//! The archive is decompressed and walked entry by entry; nothing is written
//! to disk. In the summaries dump each record is
//! `ORCID_YYYY_MM_summaries/<last 3 chars>/<iD>.xml`.
//!
//! [`records`] yields each `.xml` member with its bytes; [`DumpRecord::orcid`]
//! (feature `id`) and [`DumpRecord::parse`] (feature `xml`) give its iD and
//! its [`Document`](crate::model::Document). For other members, or to skip
//! reading some, walk `archive.entries()` with [`xml_entry_stem`] and
//! [`read_entry`].
//!
//! A member is held in memory whole, so every read is capped by
//! [`Limits::max_member_bytes`] (64 MiB by default, as yorc-duckdb's
//! `max_member_bytes`; the largest record seen is 5.4 MB). A member over it
//! is an [`io::Error`] (kind [`InvalidData`](io::ErrorKind::InvalidData)),
//! raised from the tar header before anything is allocated, and again while
//! reading if the header lied: a corrupt or crafted archive (a gzip bomb)
//! cannot allocate without bound. Use [`records_with`] and
//! [`read_entry_with`] for another limit.
//!
//! ```no_run
//! # #[cfg(all(feature = "id", feature = "xml"))] {
//! use std::path::Path;
//! use yorc::dump;
//!
//! let mut archive = dump::open(Path::new("ORCID_2025_10_summaries_1.tar.gz"))?;
//! for member in dump::records(&mut archive)? {
//!     let member = member?;
//!     let doc = member.parse()?;
//!     println!("{:?}: {:?}", member.orcid(), doc.orcid());
//! }
//! # }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod reservoir;

pub use reservoir::{Reservoir, Slot, SplitMix64};

use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};

use flate2::read::MultiGzDecoder;

/// An opened archive: `tar` over a multi-member gzip decoder.
pub type DumpArchive = tar::Archive<MultiGzDecoder<BufReader<File>>>;

/// Opens a Public Data File archive (`.tar.gz`) for streaming; walk it with
/// `entries()`, or with [`records`] for parsed documents.
///
/// ```no_run
/// use std::path::Path;
///
/// let mut archive = yorc::dump::open(Path::new("ORCID_2025_10_summaries_1.tar.gz"))?;
/// for entry in archive.entries()? {
///     let entry = entry?;
///     println!("{}", entry.path()?.display());
/// }
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn open(path: &Path) -> io::Result<DumpArchive> {
    let file = File::open(path)?;
    let gz = MultiGzDecoder::new(BufReader::with_capacity(1 << 20, file));
    Ok(tar::Archive::new(gz))
}

/// `true` for `*.tar.gz` / `*.tgz` (case-insensitive).
///
/// ```
/// use std::path::Path;
///
/// assert!(yorc::dump::is_dump_path(Path::new("ORCID_2025_10_summaries.tar.gz")));
/// assert!(!yorc::dump::is_dump_path(Path::new("record.xml")));
/// ```
pub fn is_dump_path(path: &Path) -> bool {
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n.to_ascii_lowercase(),
        None => return false,
    };
    name.ends_with(".tar.gz") || name.ends_with(".tgz")
}

/// The file stem of an entry path ending in `.xml` (case-insensitive).
///
/// ```
/// use std::path::Path;
///
/// let p = Path::new("ORCID_2025_10_summaries/097/0000-0002-1825-0097.xml");
/// assert_eq!(yorc::dump::xml_entry_stem(p), Some("0000-0002-1825-0097"));
/// assert_eq!(yorc::dump::xml_entry_stem(Path::new("README.txt")), None);
/// ```
pub fn xml_entry_stem(path: &Path) -> Option<&str> {
    let name = path.file_name()?.to_str()?;
    if name.len() > 4 && name.is_char_boundary(name.len() - 4) {
        let (stem, ext) = name.split_at(name.len() - 4);
        if ext.eq_ignore_ascii_case(".xml") {
            return Some(stem);
        }
    }
    None
}

/// Size limits for reading archive members; see [`records_with`] and
/// [`read_entry_with`].
///
/// `#[non_exhaustive]`, so that more limits can be added: start from
/// [`Limits::default()`] and set what you need.
///
/// ```
/// use yorc::dump::Limits;
///
/// assert_eq!(Limits::default().max_member_bytes, 64 << 20);
/// let limits = Limits::default().with_max_member_bytes(8 << 20);
/// assert_eq!(limits.max_member_bytes, 8 << 20);
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Limits {
    /// The largest member read, in bytes (yorc-duckdb's `max_member_bytes`).
    /// A member over it is an error, not a truncated read. `u64::MAX` turns
    /// the limit off.
    pub max_member_bytes: u64,
}

impl Limits {
    /// The default [`max_member_bytes`](Limits::max_member_bytes): 64 MiB,
    /// as yorc-duckdb's.
    pub const DEFAULT_MAX_MEMBER_BYTES: u64 = 64 << 20;

    /// These limits with [`max_member_bytes`](Limits::max_member_bytes) set
    /// to `bytes`.
    pub fn with_max_member_bytes(mut self, bytes: u64) -> Limits {
        self.max_member_bytes = bytes;
        self
    }
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            max_member_bytes: Limits::DEFAULT_MAX_MEMBER_BYTES,
        }
    }
}

/// Iterates over the `.xml` file members of `archive` (any other member is
/// skipped), reading each fully, with the default [`Limits`] (64 MiB a
/// member): [`records_with`]`(archive, Limits::default())`.
pub fn records<R: Read>(archive: &mut tar::Archive<R>) -> io::Result<Records<'_, R>> {
    records_with(archive, Limits::default())
}

/// [`records`] with explicit [`Limits`]. A member over
/// [`Limits::max_member_bytes`] is yielded as an [`io::Error`] (kind
/// [`InvalidData`](io::ErrorKind::InvalidData)) naming its path, its size
/// and the limit; its bytes are not read, and the walk goes on with the
/// next member.
///
/// ```no_run
/// use std::path::Path;
/// use yorc::dump::{self, Limits};
///
/// let mut archive = dump::open(Path::new("ORCID_2025_10_summaries_1.tar.gz"))?;
/// let limits = Limits::default().with_max_member_bytes(16 << 20);
/// for member in dump::records_with(&mut archive, limits)? {
///     match member {
///         Ok(m) => println!("{}: {} bytes", m.stem, m.bytes.len()),
///         Err(e) => eprintln!("skipped: {e}"),
///     }
/// }
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn records_with<R: Read>(
    archive: &mut tar::Archive<R>,
    limits: Limits,
) -> io::Result<Records<'_, R>> {
    Ok(Records {
        entries: archive.entries()?,
        limits,
    })
}

/// The iterator of [`records`] and [`records_with`]: one [`DumpRecord`] per
/// `.xml` member, in archive order. An I/O error (a member over the limit
/// included) is yielded as an item; a corrupt gzip or tar stream usually
/// ends the walk.
pub struct Records<'a, R: Read> {
    entries: tar::Entries<'a, R>,
    limits: Limits,
}

impl<R: Read> Iterator for Records<'_, R> {
    type Item = io::Result<DumpRecord>;

    fn next(&mut self) -> Option<io::Result<DumpRecord>> {
        loop {
            let mut entry = match self.entries.next()? {
                Ok(e) => e,
                Err(e) => return Some(Err(e)),
            };
            if !entry.header().entry_type().is_file() {
                continue;
            }
            let path = match entry.path() {
                Ok(p) => p.into_owned(),
                Err(e) => return Some(Err(e)),
            };
            let Some(stem) = xml_entry_stem(&path).map(str::to_string) else {
                continue;
            };
            let size = entry.size();
            let read = read_limited(&mut entry, size, self.limits, Some(&path));
            return Some(read.map(|bytes| DumpRecord { path, stem, bytes }));
        }
    }
}

/// One `.xml` member of an archive, read fully.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DumpRecord {
    /// The member's path inside the archive.
    pub path: PathBuf,
    /// Its file name without `.xml`: the iD in the summaries dump
    /// (`0000-0002-1825-0097`), `<iD>_works_<put-code>` and the like in the
    /// activities dump.
    pub stem: String,
    /// The member's bytes, exactly as stored (hash these for `source_hash`).
    pub bytes: Vec<u8>,
}

impl DumpRecord {
    /// The stem as an iD, `None` if it is not one (activities members).
    /// [`Document::orcid`](crate::model::Document::orcid) reads the iD from
    /// the document instead.
    #[cfg(feature = "id")]
    pub fn orcid(&self) -> Option<crate::id::OrcidId> {
        crate::id::OrcidId::parse(&self.stem).ok()
    }

    /// Parses the bytes as ORCID XML ([`crate::xml::from_slice`]).
    #[cfg(feature = "xml")]
    pub fn parse(&self) -> crate::Result<crate::model::Document> {
        crate::xml::from_slice(&self.bytes)
    }
}

/// Reads one entry fully, with the default [`Limits`] (64 MiB):
/// [`read_entry_with`]`(reader, size_hint, Limits::default())`.
/// `size_hint` is the tar header size.
///
/// ```
/// let mut r: &[u8] = b"<x/>";
/// assert_eq!(yorc::dump::read_entry(&mut r, 4)?, b"<x/>");
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn read_entry<R: Read>(reader: &mut R, size_hint: u64) -> io::Result<Vec<u8>> {
    read_entry_with(reader, size_hint, Limits::default())
}

/// Reads one entry fully, at most [`Limits::max_member_bytes`] bytes.
///
/// `size_hint` is the tar header size (`entry.size()`). If it is over the
/// limit, the read fails before anything is allocated or read. The reader
/// is also capped, so an entry that yields more bytes than the limit fails
/// too, whatever its header said. Both failures are an [`io::Error`] of kind
/// [`InvalidData`](io::ErrorKind::InvalidData) whose message names
/// `max_member_bytes` and the limit. The buffer starts at `size_hint`
/// bytes (at most 64 MiB) and grows only as data arrives.
///
/// ```
/// use yorc::dump::{read_entry_with, Limits};
///
/// let limits = Limits::default().with_max_member_bytes(4);
/// let mut r: &[u8] = b"<x/>";
/// assert_eq!(read_entry_with(&mut r, 4, limits)?, b"<x/>");
///
/// // the header says 5 bytes: refused before reading
/// let mut r: &[u8] = b"<xy/>";
/// let e = read_entry_with(&mut r, 5, limits).unwrap_err();
/// assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
/// assert_eq!(e.to_string(), "member is 5 bytes, over max_member_bytes (4)");
///
/// // the header says 4, the data is longer: caught while reading
/// let mut r: &[u8] = b"<xy/>";
/// let e = read_entry_with(&mut r, 4, limits).unwrap_err();
/// assert_eq!(e.to_string(), "member is over max_member_bytes (4): more than 4 bytes read");
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn read_entry_with<R: Read>(
    reader: &mut R,
    size_hint: u64,
    limits: Limits,
) -> io::Result<Vec<u8>> {
    read_limited(reader, size_hint, limits, None)
}

/// The initial buffer capacity is at most this (as before the limit existed):
/// a header below the limit can still lie.
const MAX_INITIAL_CAPACITY: u64 = 64 << 20;

/// [`read_entry_with`], naming the member's path in the error when known.
fn read_limited<R: Read>(
    reader: &mut R,
    size_hint: u64,
    limits: Limits,
    path: Option<&Path>,
) -> io::Result<Vec<u8>> {
    let limit = limits.max_member_bytes;
    if size_hint > limit {
        let detail = format!("is {size_hint} bytes, over max_member_bytes ({limit})");
        return Err(too_big(path, &detail));
    }
    let cap = usize::try_from(size_hint.min(MAX_INITIAL_CAPACITY)).unwrap_or(0);
    let mut buf = Vec::with_capacity(cap);
    // One byte more than the limit tells "exactly at the limit" from "over it".
    reader.take(limit.saturating_add(1)).read_to_end(&mut buf)?;
    if buf.len() as u64 > limit {
        let detail = format!("is over max_member_bytes ({limit}): more than {limit} bytes read");
        return Err(too_big(path, &detail));
    }
    Ok(buf)
}

/// The error of a member over [`Limits::max_member_bytes`]:
/// `member '<path>' <detail>`, or `member <detail>` without a path.
fn too_big(path: Option<&Path>, detail: &str) -> io::Error {
    let message = match path {
        Some(p) => format!("member '{}' {detail}", p.display()),
        None => format!("member {detail}"),
    };
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use flate2::Compression;
    use flate2::write::GzEncoder;

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "yorc-rs-test-{}-{nanos}-{name}",
            std::process::id()
        ))
    }

    fn write_tar_gz(path: &Path, entries: &[(String, Vec<u8>)]) {
        let file = File::create(path).unwrap();
        let gz = GzEncoder::new(file, Compression::fast());
        let mut builder = tar::Builder::new(gz);
        for (name, data) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_entry_type(tar::EntryType::Regular);
            header.set_cksum();
            builder
                .append_data(&mut header, name, data.as_slice())
                .unwrap();
        }
        let gz = builder.into_inner().unwrap();
        let mut file = gz.finish().unwrap();
        file.flush().unwrap();
    }

    #[test]
    fn dump_path_detection() {
        assert!(is_dump_path(Path::new("/x/ORCID_2024_10_summaries.tar.gz")));
        assert!(is_dump_path(Path::new("a.TGZ")));
        assert!(!is_dump_path(Path::new("a.xml")));
        assert!(!is_dump_path(Path::new("a.gz")));
    }

    #[test]
    fn entry_stems() {
        assert_eq!(
            xml_entry_stem(Path::new(
                "ORCID_2024_10_summaries/097/0000-0002-1825-0097.xml"
            )),
            Some("0000-0002-1825-0097")
        );
        assert_eq!(xml_entry_stem(Path::new("dir/README.txt")), None);
        assert_eq!(xml_entry_stem(Path::new(".xml")), None);
        assert_eq!(xml_entry_stem(Path::new("A.XML")), Some("A"));
    }

    #[test]
    fn streams_entries_from_tar_gz() {
        let path = temp_path("stream.tar.gz");
        let id = "0000-0002-1825-0097".to_string();
        let xml = b"<record:record path=\"/0000-0002-1825-0097\"/>".to_vec();
        let entries = vec![
            (format!("summaries/097/{id}.xml"), xml.clone()),
            ("summaries/README.txt".to_string(), b"hello".to_vec()),
        ];
        write_tar_gz(&path, &entries);
        let mut ar = open(&path).unwrap();
        let mut seen = Vec::new();
        for e in ar.entries().unwrap() {
            let mut e = e.unwrap();
            let p = e.path().unwrap().into_owned();
            if let Some(stem) = xml_entry_stem(&p) {
                let size = e.size();
                let bytes = read_entry(&mut e, size).unwrap();
                seen.push((stem.to_string(), bytes.len()));
            }
        }
        std::fs::remove_file(&path).ok();
        assert_eq!(seen, vec![(id, xml.len())]);
    }

    #[test]
    fn records_yields_xml_members() {
        let path = temp_path("records.tar.gz");
        let xml = b"<record:record path=\"/0000-0002-1825-0097\"/>".to_vec();
        let entries = vec![
            ("s/097/0000-0002-1825-0097.xml".to_string(), xml.clone()),
            ("s/README.txt".to_string(), b"hello".to_vec()),
            (
                "s/634/0000-0003-0100-6634_works_1.xml".to_string(),
                b"<x/>".to_vec(),
            ),
        ];
        write_tar_gz(&path, &entries);
        let mut ar = open(&path).unwrap();
        let got: Vec<DumpRecord> = records(&mut ar).unwrap().map(Result::unwrap).collect();
        std::fs::remove_file(&path).ok();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].stem, "0000-0002-1825-0097");
        assert_eq!(got[0].bytes, xml);
        assert_eq!(got[1].stem, "0000-0003-0100-6634_works_1");
        #[cfg(feature = "id")]
        {
            assert_eq!(got[0].orcid().unwrap().as_str(), "0000-0002-1825-0097");
            assert_eq!(got[1].orcid(), None);
        }
        #[cfg(feature = "xml")]
        assert_eq!(got[0].parse().unwrap().orcid(), Some("0000-0002-1825-0097"));
    }

    #[test]
    fn records_with_refuses_members_over_the_limit() {
        let path = temp_path("limit.tar.gz");
        let small = b"<x/>".to_vec();
        let big = vec![b' '; 100];
        let entries = vec![
            ("s/a.xml".to_string(), small.clone()),
            ("s/big.xml".to_string(), big),
            ("s/b.xml".to_string(), small.clone()),
        ];
        write_tar_gz(&path, &entries);
        let mut ar = open(&path).unwrap();
        let limits = Limits::default().with_max_member_bytes(99);
        let got: Vec<io::Result<DumpRecord>> = records_with(&mut ar, limits).unwrap().collect();
        std::fs::remove_file(&path).ok();
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].as_ref().unwrap().bytes, small);
        let e = got[1].as_ref().unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            e.to_string(),
            "member 's/big.xml' is 100 bytes, over max_member_bytes (99)"
        );
        // the walk goes on after a refused member
        assert_eq!(got[2].as_ref().unwrap().stem, "b");
    }

    #[test]
    fn member_at_the_limit_is_read() {
        let path = temp_path("at-limit.tar.gz");
        let data = vec![b'a'; 100];
        write_tar_gz(&path, &[("s/a.xml".to_string(), data.clone())]);
        let mut ar = open(&path).unwrap();
        let limits = Limits::default().with_max_member_bytes(100);
        let got: Vec<DumpRecord> = records_with(&mut ar, limits)
            .unwrap()
            .map(Result::unwrap)
            .collect();
        std::fs::remove_file(&path).ok();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].bytes, data);
    }

    #[test]
    fn read_entry_caps_a_lying_size() {
        // the size hint says 0 (unknown or lying); the reader yields more
        let data = vec![0u8; 1000];
        let limits = Limits::default().with_max_member_bytes(10);
        let e = read_entry_with(&mut data.as_slice(), 0, limits).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidData);
        assert!(e.to_string().contains("max_member_bytes (10)"), "{e}");
        // a hint over the limit fails before reading
        let mut r = data.as_slice();
        assert!(read_entry_with(&mut r, 1000, limits).is_err());
        assert_eq!(r.len(), 1000, "nothing was read");
        // no limit
        let off = Limits::default().with_max_member_bytes(u64::MAX);
        assert_eq!(read_entry_with(&mut data.as_slice(), 0, off).unwrap(), data);
        // the default limit applies to read_entry
        assert_eq!(Limits::default().max_member_bytes, 64 << 20);
        let e = read_entry(&mut data.as_slice(), (64 << 20) + 1).unwrap_err();
        assert!(
            e.to_string().contains("over max_member_bytes (67108864)"),
            "{e}"
        );
    }
}
