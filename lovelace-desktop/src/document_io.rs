//! The on-disk form of one document: UTF-8 validation, the byte-order mark and the newline style.
//!
//! DOC-002 asks for a file lifecycle that never rewrites bytes the user did not change. Two
//! decisions make that possible here:
//!
//! - the decoder returns the text **exactly as stored** (`\r` characters stay in the string), so
//!   `encode` reproduces the file body for every file that decoded successfully;
//! - the loaded bytes are kept beside the text, so a clean save writes the file's own bytes back
//!   even after the editor's line model (one separator per line) has had the text.
//!
//! Nothing in this module normalises text: it does not replace CRLF with LF, it does not drop a
//! BOM, and it does not re-encode a file that reads as valid UTF-8.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// The UTF-8 byte-order mark, as it appears at the start of a file.
pub const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

// ---------------------------------------------------------------------------------------------
// DOC-006: the file's recorded state and the disk's current state.
//
// A document remembers the size, modification time and content hash of the file it was opened
// from and of every file it has successfully written. Before any write the same three values are
// read from disk again; a difference of any one of them withholds the write. The hash is what
// catches an edit that kept the size and the timestamp (a same-length rewrite restored to its old
// mtime), which a size/mtime comparison alone would miss.
// ---------------------------------------------------------------------------------------------

/// The FNV-1a 64-bit offset basis.
const HASH_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// The FNV-1a 64-bit prime.
const HASH_PRIME: u64 = 0x0000_0100_0000_01b3;

/// A stable content hash of a file's bytes (FNV-1a, 64 bits).
///
/// It is deliberately not `DefaultHasher`: the value is written into evidence and compared
/// between runs, so it must not depend on the standard library's choice of algorithm.
pub fn content_hash(bytes: &[u8]) -> u64 {
    let mut hash = HASH_OFFSET;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(HASH_PRIME);
    }
    hash
}

/// Everything DOC-006 records about one version of a file: its size, its modification time and a
/// content hash. Equality of two snapshots is what "the file has not changed" means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSnapshot {
    /// Length in bytes of the content that was hashed.
    pub size: u64,
    /// The file's modification time, when the platform reports one.
    pub modified: Option<SystemTime>,
    /// [`content_hash`] of the content.
    pub hash: u64,
}

impl FileSnapshot {
    /// A snapshot of `bytes` whose modification time is `modified`.
    pub fn of_bytes(bytes: &[u8], modified: Option<SystemTime>) -> FileSnapshot {
        FileSnapshot {
            size: bytes.len() as u64,
            modified,
            hash: content_hash(bytes),
        }
    }

    /// The hash as eight hexadecimal bytes, for the log and the conflict prompt.
    pub fn hash_label(&self) -> String {
        format!("{:016x}", self.hash)
    }

    /// The modification time as `YYYY-MM-DD HH:MM:SS UTC`, or a word when it is unknown.
    pub fn time_label(&self) -> String {
        match self.modified {
            Some(time) => format_system_time(time),
            None => "unknown time".to_string(),
        }
    }

    /// One line naming all three recorded values, used by the conflict prompt and its tests.
    pub fn summary(&self) -> String {
        format!(
            "{} bytes · {} · content {}",
            self.size,
            self.time_label(),
            self.hash_label()
        )
    }
}

/// The state of a path on disk at the moment it was probed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiskState {
    /// Nothing exists at the path (deleted or renamed away).
    Missing,
    /// The file exists and could be read.
    Present(FileSnapshot),
    /// The path exists but could not be read; the message says why.
    Unreadable {
        /// The operating system's message.
        message: String,
    },
}

impl DiskState {
    /// The snapshot, when the file was readable.
    pub fn snapshot(&self) -> Option<FileSnapshot> {
        match self {
            DiskState::Present(snapshot) => Some(*snapshot),
            _ => None,
        }
    }

    /// One line describing the current state for the conflict prompt.
    pub fn summary(&self) -> String {
        match self {
            DiskState::Missing => "deleted or renamed: the file is no longer there".to_string(),
            DiskState::Present(snapshot) => snapshot.summary(),
            DiskState::Unreadable { message } => format!("could not be read: {message}"),
        }
    }
}

/// The cached external-change indicator a tab shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskStatus {
    /// The document has no path yet.
    NotLocated,
    /// The path matches the recorded snapshot.
    Clean,
    /// The path exists but its size, time or content differ from the recorded snapshot.
    Changed,
    /// The path no longer exists.
    Missing,
    /// The path exists but could not be read.
    Unreadable,
}

impl DiskStatus {
    /// The tab indicator text, or `None` when there is nothing to say.
    pub fn marker(self) -> Option<&'static str> {
        match self {
            DiskStatus::NotLocated | DiskStatus::Clean => None,
            DiskStatus::Changed => Some("changed on disk"),
            DiskStatus::Missing => Some("missing on disk"),
            DiskStatus::Unreadable => Some("unreadable on disk"),
        }
    }
}

/// Reads `path` and returns its current size, modification time and content hash.
///
/// A path that does not exist is [`DiskState::Missing`], not an error: a document whose file was
/// deleted must be reported as missing rather than treated as a fresh file that a save may create.
pub fn probe(path: &Path) -> DiskState {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return DiskState::Missing,
        Err(error) => {
            return DiskState::Unreadable {
                message: error.to_string(),
            };
        }
    };
    let modified = metadata.modified().ok();
    match std::fs::read(path) {
        // The size is taken from the bytes that were hashed, so the two cannot disagree even if
        // the file is being rewritten while it is read.
        Ok(bytes) => DiskState::Present(FileSnapshot::of_bytes(&bytes, modified)),
        Err(error) => DiskState::Unreadable {
            message: error.to_string(),
        },
    }
}

/// Formats a system time as `YYYY-MM-DD HH:MM:SS UTC`.
///
/// Dates before 1970 are named rather than formatted; no document in this product can have one.
pub fn format_system_time(time: SystemTime) -> String {
    let Ok(duration) = time.duration_since(UNIX_EPOCH) else {
        return "before 1970".to_string();
    };
    let seconds = duration.as_secs();
    let days = (seconds / 86_400) as i64;
    let second_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        second_of_day / 3_600,
        (second_of_day % 3_600) / 60,
        second_of_day % 60
    )
}

/// Converts days since 1970-01-01 to a proleptic Gregorian (year, month, day).
///
/// This is Howard Hinnant's civil-from-days algorithm, used so the window can show the file's
/// modification time without pulling in a date/time dependency.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = (z - era * 146_097) as u64; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// One line ending, the separator new text is written with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnding {
    /// A single line-feed byte (`\n`).
    Lf,
    /// A carriage-return/line-feed pair (`\r\n`).
    CrLf,
}

impl LineEnding {
    /// The separator as it appears in the file.
    pub fn as_str(self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::CrLf => "\r\n",
        }
    }

    /// The name used in the window's encoding badge and in the log.
    pub fn label(self) -> &'static str {
        match self {
            LineEnding::Lf => "LF",
            LineEnding::CrLf => "CRLF",
        }
    }
}

/// The newline style observed in one file, recorded when the document is opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewlineStyle {
    /// Every line ends with `\n`.
    Lf,
    /// Every line ends with `\r\n`.
    CrLf,
    /// The file uses both endings (or a lone `\r`). `first` is the ending of the first line, which
    /// is the separator new lines are written with; an unmodified save still writes the file's own
    /// bytes, so a mixed file is only rewritten from the moment the user edits it.
    Mixed {
        /// The ending new lines are written with.
        first: LineEnding,
    },
    /// No line ending occurs (an empty or single-line file).
    None,
}

impl NewlineStyle {
    /// Classifies the newlines in `text`.
    pub fn detect(text: &str) -> NewlineStyle {
        let bytes = text.as_bytes();
        let mut crlf = 0usize;
        let mut lf = 0usize;
        let mut cr = 0usize;
        let mut first = None;
        let mut index = 0;
        while index < bytes.len() {
            match bytes[index] {
                b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                    crlf += 1;
                    first.get_or_insert(LineEnding::CrLf);
                    index += 2;
                    continue;
                }
                b'\r' => cr += 1,
                b'\n' => {
                    lf += 1;
                    first.get_or_insert(LineEnding::Lf);
                }
                _ => {}
            }
            index += 1;
        }
        match (crlf, lf, cr) {
            (0, 0, 0) => NewlineStyle::None,
            (_, 0, 0) => NewlineStyle::CrLf,
            (0, _, 0) => NewlineStyle::Lf,
            _ => NewlineStyle::Mixed {
                first: first.unwrap_or(LineEnding::Lf),
            },
        }
    }

    /// The separator new text is written with.
    pub fn ending(self) -> LineEnding {
        match self {
            NewlineStyle::Lf | NewlineStyle::None => LineEnding::Lf,
            NewlineStyle::CrLf => LineEnding::CrLf,
            NewlineStyle::Mixed { first } => first,
        }
    }

    /// The name used in the window's encoding badge and in the log.
    pub fn label(self) -> &'static str {
        match self {
            NewlineStyle::Lf => LineEnding::Lf.label(),
            NewlineStyle::CrLf => LineEnding::CrLf.label(),
            NewlineStyle::Mixed { .. } => "mixed newlines",
            NewlineStyle::None => "no newline",
        }
    }
}

/// Why a file could not be read as a Lovelace document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The file starts with a UTF-16 byte-order mark, the usual sign of a Windows editor that
    /// saved as "Unicode" instead of UTF-8.
    Utf16Bom {
        /// True for a big-endian byte-order mark.
        big_endian: bool,
    },
    /// The bytes are not a UTF-8 sequence.
    InvalidUtf8 {
        /// Byte offset of the first byte that does not belong to a valid sequence.
        offset: usize,
        /// That byte.
        byte: u8,
    },
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Utf16Bom { big_endian } => write!(
                formatter,
                "the file starts with a UTF-16 {} byte-order mark, not UTF-8; re-save it as UTF-8 \
                 (a Lovelace script is UTF-8 text)",
                if *big_endian {
                    "big-endian"
                } else {
                    "little-endian"
                }
            ),
            DecodeError::InvalidUtf8 { offset, byte } => write!(
                formatter,
                "the file is not UTF-8 text: byte 0x{byte:02X} at offset {offset} is not valid \
                 UTF-8; re-save it as UTF-8 (a Lovelace script is UTF-8 text)"
            ),
        }
    }
}

/// A file that decoded, with everything an unmodified save needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedFile {
    /// The text exactly as stored: no BOM, and every `\r` kept.
    pub text: String,
    /// True when the file began with a UTF-8 byte-order mark.
    pub bom: bool,
    /// The newline style the file used.
    pub newline: NewlineStyle,
    /// The file's own bytes, so a save of an unmodified buffer is byte-identical.
    pub bytes: Vec<u8>,
}

/// Decodes file bytes as a Lovelace document.
///
/// A UTF-8 byte-order mark is detected and removed from the text; a UTF-16 byte-order mark and any
/// invalid sequence are refused with a message naming the problem, so a non-UTF-8 file is never
/// written back through the decoder. No normalisation of any kind happens here.
pub fn decode(bytes: &[u8]) -> Result<LoadedFile, DecodeError> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Err(DecodeError::Utf16Bom { big_endian: false });
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return Err(DecodeError::Utf16Bom { big_endian: true });
    }
    let (bom, body) = match bytes.strip_prefix(&UTF8_BOM) {
        Some(body) => (true, body),
        None => (false, bytes),
    };
    let text = match std::str::from_utf8(body) {
        Ok(text) => text.to_string(),
        Err(error) => {
            // Offsets are reported against the file the user chose, so the skipped byte-order
            // mark is added back.
            let offset = error.valid_up_to() + (bytes.len() - body.len());
            return Err(DecodeError::InvalidUtf8 {
                offset,
                byte: bytes.get(offset).copied().unwrap_or(0),
            });
        }
    };
    let newline = NewlineStyle::detect(&text);
    Ok(LoadedFile {
        text,
        bom,
        newline,
        bytes: bytes.to_vec(),
    })
}

/// Encodes a document's text, prefixing the byte-order mark when the document had one.
///
/// The text is written exactly as it stands: a buffer loaded from a CRLF file still contains its
/// `\r` characters, so this function never rewrites line endings.
pub fn encode(text: &str, bom: bool) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(text.len() + UTF8_BOM.len());
    if bom {
        bytes.extend_from_slice(&UTF8_BOM);
    }
    bytes.extend_from_slice(text.as_bytes());
    bytes
}

/// Reads a file and decodes it, with failures phrased for the log and the prompt.
pub fn read(path: &Path) -> Result<LoadedFile, String> {
    let bytes =
        std::fs::read(path).map_err(|error| format!("Cannot open {}: {error}", path.display()))?;
    decode(&bytes).map_err(|error| format!("Cannot open {}: {error}.", path.display()))
}

/// PF-004: the suffix every temporary save file carries.
///
/// The temporary file is created in the target's own directory, so the final rename is a
/// same-volume replacement (an atomic operation on NTFS), and a failed write can always be
/// cleaned up without touching the target.
pub const TEMP_SUFFIX: &str = ".save.tmp";

/// Writes `bytes` to `path` atomically and returns the number of bytes written.
///
/// PLF-004: the bytes go to a new temporary file in the target's directory, are flushed and
/// synced to disk, and only then is the temporary file renamed over the target. The original is
/// never truncated: if the temporary write, the sync or the rename fails, the target is left
/// byte-identical and the temporary file is removed. The target's permissions are carried over
/// to the replacement (the permission bits on Unix, the file attributes on Windows) before the
/// rename, so preservation cannot turn a successful write into a reported failure.
pub fn write(path: &Path, bytes: &[u8]) -> Result<usize, String> {
    write_with(path, bytes, |file| {
        use std::io::Write;
        file.write_all(bytes)
    })
}

/// The atomic write, with the body writer injected so a test can fail it deterministically.
fn write_with<F>(path: &Path, bytes: &[u8], body: F) -> Result<usize, String>
where
    F: FnOnce(&mut std::fs::File) -> std::io::Result<()>,
{
    use std::sync::atomic::{AtomicU64, Ordering};

    let failure = |error: std::io::Error| format!("Cannot save {}: {error}", path.display());
    let directory = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    };
    let Some(file_name) = path.file_name() else {
        return Err(format!("Cannot save {}: the path names no file", path.display()));
    };
    // One sequence number per write makes the temporary name unique even when two documents are
    // saved in the same millisecond; the process id keeps two windows apart.
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
    let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let temporary = directory.join(format!(
        ".{}.{}.{sequence}{TEMP_SUFFIX}",
        file_name.to_string_lossy(),
        std::process::id()
    ));

    // The target is read before the temporary file is created; a missing target has nothing to
    // preserve. The metadata is not used to decide whether to write: that is `conflict_for`'s job.
    let existing = std::fs::metadata(path).ok();

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    // Windows file attributes (hidden, system, archive, not-content-indexed) are applied as the
    // temporary file is created, so the rename carries them to the replacement. The read-only bit
    // is deliberately not carried: a read-only target makes the rename fail (preserving the
    // original), which is the correct behaviour for a file the user protected.
    #[cfg(windows)]
    if let Some(metadata) = &existing {
        use std::os::windows::fs::MetadataExt;
        const HIDDEN_OR_SYSTEM_OR_ARCHIVE: u32 = 0x2 | 0x4 | 0x20 | 0x2000;
        let attributes = metadata.file_attributes() & HIDDEN_OR_SYSTEM_OR_ARCHIVE;
        if attributes != 0 {
            use std::os::windows::fs::OpenOptionsExt;
            options.attributes(attributes);
        }
    }
    // The permission bits (including setuid/setgid/sticky) are applied to the replacement.
    #[cfg(unix)]
    if let Some(metadata) = &existing {
        use std::os::unix::fs::PermissionsExt;
        options.mode(metadata.permissions().mode());
    }

    let mut file = match options.open(&temporary) {
        Ok(file) => file,
        Err(error) => return Err(failure(error)),
    };
    #[cfg(unix)]
    if let Some(metadata) = &existing {
        if let Err(error) = file.set_permissions(metadata.permissions()) {
            drop(file);
            let _ = std::fs::remove_file(&temporary);
            return Err(failure(error));
        }
    }

    // Everything up to the rename happens on the temporary file only. `sync_all` is what makes
    // the rename's result durable after a power loss, not just visible to other processes.
    if let Err(error) = body(&mut file).and_then(|()| {
        std::io::Write::flush(&mut file)?;
        file.sync_all()
    }) {
        drop(file);
        let _ = std::fs::remove_file(&temporary);
        return Err(failure(error));
    }
    drop(file);
    match std::fs::rename(&temporary, path) {
        Ok(()) => Ok(bytes.len()),
        Err(error) => {
            // The target was never opened for writing, so it is still the original file.
            let _ = std::fs::remove_file(&temporary);
            Err(failure(error))
        }
    }
}

/// PLF-004 test seam: a write that fails after `written` bytes of the body.
#[cfg(test)]
pub(crate) fn write_faulted(path: &Path, bytes: &[u8], written: usize) -> Result<usize, String> {
    write_with(path, bytes, |file| {
        use std::io::Write;
        file.write_all(&bytes[..written.min(bytes.len())])?;
        Err(std::io::Error::other("injected write failure"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join("lovelace-desktop-file-io-tests");
        std::fs::create_dir_all(&directory).expect("scratch directory");
        directory.join(format!("{name}-{}.lv", std::process::id()))
    }

    #[test]
    fn a_utf8_bom_is_detected_and_removed_from_the_text() {
        let bytes = b"\xEF\xBB\xBFa = 1\n";
        let loaded = decode(bytes).expect("valid UTF-8");
        assert!(loaded.bom, "the byte-order mark must be recorded");
        assert_eq!(
            loaded.text, "a = 1\n",
            "the text must not start with U+FEFF"
        );
        assert_eq!(loaded.bytes, bytes, "the file's own bytes must be kept");
        assert_eq!(encode(&loaded.text, loaded.bom), bytes, "and written back");
    }

    #[test]
    fn a_text_without_a_bom_encodes_without_one() {
        let loaded = decode(b"a = 1\r\nb = 2\r\n").expect("valid UTF-8");
        assert!(!loaded.bom);
        assert_eq!(loaded.newline, NewlineStyle::CrLf);
        assert_eq!(encode(&loaded.text, loaded.bom), b"a = 1\r\nb = 2\r\n");
    }

    #[test]
    fn every_newline_style_is_classified() {
        assert_eq!(NewlineStyle::detect(""), NewlineStyle::None);
        assert_eq!(NewlineStyle::detect("a = 1"), NewlineStyle::None);
        assert_eq!(NewlineStyle::detect("a\nb\n"), NewlineStyle::Lf);
        assert_eq!(NewlineStyle::detect("a\r\nb\r\n"), NewlineStyle::CrLf);
        assert_eq!(
            NewlineStyle::detect("a\r\nb\n"),
            NewlineStyle::Mixed {
                first: LineEnding::CrLf
            },
            "the first ending is the one new lines are written with"
        );
        assert_eq!(
            NewlineStyle::detect("a\nb\r\n"),
            NewlineStyle::Mixed {
                first: LineEnding::Lf
            }
        );
        assert_eq!(
            NewlineStyle::detect("a\rb\n"),
            NewlineStyle::Mixed {
                first: LineEnding::Lf
            },
            "a lone carriage return is not a line ending, so the file is mixed"
        );
    }

    #[test]
    fn a_mixed_file_keeps_its_own_bytes_and_writes_new_lines_with_the_first_ending() {
        let original = b"one\r\ntwo\nthree\r\n";
        let loaded = decode(original).expect("valid UTF-8");
        assert_eq!(
            loaded.newline,
            NewlineStyle::Mixed {
                first: LineEnding::CrLf
            }
        );
        assert_eq!(loaded.newline.ending(), LineEnding::CrLf);
        // The decoded text keeps every `\r`, so even a mixed file re-encodes to its own bytes.
        assert_eq!(encode(&loaded.text, loaded.bom), original);
        // What no single separator can reproduce is the editor's line model (lines joined with
        // one separator), which is exactly why a clean save writes the loaded bytes instead.
        let editor_style = loaded
            .text
            .split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line))
            .collect::<Vec<_>>()
            .join(loaded.newline.ending().as_str());
        assert_ne!(editor_style.as_bytes(), original);
        assert_eq!(loaded.bytes, original);
    }

    #[test]
    fn invalid_utf8_is_refused_and_the_message_names_the_byte_and_offset() {
        let error = decode(b"a = 1\nb = \xE9\n").expect_err("0xE9 alone is not UTF-8");
        assert_eq!(
            error,
            DecodeError::InvalidUtf8 {
                offset: 10,
                byte: 0xE9
            }
        );
        let message = error.to_string();
        for expected in ["not UTF-8", "0xE9", "offset 10", "re-save it as UTF-8"] {
            assert!(
                message.contains(expected),
                "the message must name {expected:?}: {message}"
            );
        }
    }

    #[test]
    fn a_utf16_bom_is_refused_with_its_own_message() {
        let error = decode(b"\xFF\xFEa\x00").expect_err("UTF-16 LE is refused");
        assert_eq!(error, DecodeError::Utf16Bom { big_endian: false });
        assert!(error.to_string().contains("UTF-16 little-endian"));
        let error = decode(b"\xFE\xFF\x00a").expect_err("UTF-16 BE is refused");
        assert_eq!(error, DecodeError::Utf16Bom { big_endian: true });
        assert!(error.to_string().contains("UTF-16 big-endian"));
    }

    #[test]
    fn a_truncated_multibyte_sequence_is_refused_at_its_first_byte() {
        let error = decode(b"ok\n\xC3").expect_err("a lone continuation start is invalid");
        assert_eq!(
            error,
            DecodeError::InvalidUtf8 {
                offset: 3,
                byte: 0xC3
            }
        );
    }

    #[test]
    fn reading_a_bom_file_from_disk_round_trips_its_bytes() {
        let path = scratch("bom");
        let bytes = b"\xEF\xBB\xBFplot([1], [2], \"bom\")\r\n";
        std::fs::write(&path, bytes).expect("write the fixture");
        let loaded = read(&path).expect("decode");
        assert!(loaded.bom);
        assert_eq!(loaded.newline, NewlineStyle::CrLf);
        assert_eq!(encode(&loaded.text, loaded.bom), bytes);
        assert_eq!(
            write(&path, &encode(&loaded.text, loaded.bom)),
            Ok(bytes.len())
        );
        assert_eq!(std::fs::read(&path).expect("read back"), bytes);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_content_hash_is_stable_and_changes_with_every_edit() {
        // FNV-1a 64 of the empty input is the offset basis, and of "a" is the named value; the
        // constants are fixed so a hash printed in evidence can be checked by hand.
        assert_eq!(content_hash(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(content_hash(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_ne!(content_hash(b"a = 1\nb = 2\n"), content_hash(b"a = 1\nb = 3\n"));
        // A same-length edit is exactly the case a size-only check would miss.
        assert_eq!(b"a = 1\nb = 2\n".len(), b"a = 1\nb = 3\n".len());
    }

    #[test]
    fn a_snapshot_records_size_time_and_hash_and_summarises_all_three() {
        let bytes = b"a = 1\n";
        let modified = Some(UNIX_EPOCH + std::time::Duration::from_secs(1_763_000_000));
        let snapshot = FileSnapshot::of_bytes(bytes, modified);
        assert_eq!(snapshot.size, bytes.len() as u64);
        assert_eq!(snapshot.modified, modified);
        assert_eq!(snapshot.hash, content_hash(bytes));
        let summary = snapshot.summary();
        assert!(summary.contains("6 bytes"), "{summary}");
        assert!(summary.contains(&snapshot.hash_label()), "{summary}");
        assert!(summary.contains("02:13:20 UTC"), "{summary}: {summary}");
    }

    #[test]
    fn a_system_time_is_formatted_as_a_utc_timestamp() {
        let epoch = format_system_time(UNIX_EPOCH);
        assert_eq!(epoch, "1970-01-01 00:00:00 UTC");
        let leap = format_system_time(UNIX_EPOCH + std::time::Duration::from_secs(951_782_400));
        assert_eq!(leap, "2000-02-29 00:00:00 UTC");
        let before = format_system_time(UNIX_EPOCH - std::time::Duration::from_secs(1));
        assert_eq!(before, "before 1970");
    }

    #[test]
    fn probing_a_file_records_its_state_and_a_missing_path_is_missing() {
        let path = scratch("probe");
        std::fs::write(&path, b"a = 1\n").expect("write the fixture");
        let first = probe(&path);
        let snapshot = first.snapshot().expect("the file is readable");
        assert_eq!(snapshot.size, 6);
        assert_eq!(snapshot.hash, content_hash(b"a = 1\n"));
        assert_eq!(probe(&path), first, "probing twice is equal until it changes");

        std::fs::write(&path, b"a = 2\n").expect("rewrite the fixture");
        let second = probe(&path);
        assert_ne!(first, second, "a same-length edit changes the hash");
        std::fs::remove_file(&path).expect("remove the fixture");
        assert_eq!(probe(&path), DiskState::Missing);
    }

    #[test]
    fn a_probe_that_matches_the_recorded_snapshot_is_clean_and_any_difference_is_changed() {
        let path = scratch("status");
        std::fs::write(&path, b"b = 2\n").expect("write the fixture");
        let recorded = probe(&path).snapshot().expect("readable");
        let current = probe(&path);
        assert_eq!(current.snapshot(), Some(recorded));
        std::fs::write(&path, b"b = 3\n").expect("rewrite the fixture");
        assert_ne!(probe(&path).snapshot(), Some(recorded));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reading_a_non_utf8_file_reports_the_path_and_the_refusal() {
        let path = scratch("non-utf8");
        std::fs::write(&path, b"a = 1\nb = \xE9\n").expect("write the fixture");
        let message = read(&path).expect_err("refused");
        assert!(message.contains(&path.display().to_string()), "{message}");
        assert!(message.contains("not UTF-8"), "{message}");
        assert_eq!(
            std::fs::read(&path).expect("the file is untouched"),
            b"a = 1\nb = \xE9\n",
            "a refused file must never be rewritten"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// The temporary files a save of `path` may have left beside it. Only names belonging to
    /// this target are considered: other tests (and other processes sharing the temp directory)
    /// create their own temporary files concurrently and must not make this test flaky.
    fn temporary_files(path: &Path) -> Vec<std::path::PathBuf> {
        let directory = path.parent().expect("the fixture has a directory");
        let prefix = format!(".{}.", path.file_name().unwrap().to_string_lossy());
        std::fs::read_dir(directory)
            .expect("read the fixture directory")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|candidate| {
                candidate
                    .file_name()
                    .map(|name| {
                        let name = name.to_string_lossy();
                        name.starts_with(&prefix) && name.ends_with(TEMP_SUFFIX)
                    })
                    .unwrap_or(false)
            })
            .collect()
    }

    #[test]
    fn an_atomic_save_replaces_the_file_and_leaves_no_temporary_file() {
        let path = scratch("atomic-ok");
        std::fs::write(&path, b"a = 1\n").expect("write the fixture");
        let replaced = write(&path, b"a = 2\nb = 3\n").expect("the write succeeds");
        assert_eq!(replaced, 12);
        assert_eq!(
            std::fs::read(&path).expect("read back"),
            b"a = 2\nb = 3\n",
            "the target holds exactly the new bytes"
        );
        assert!(
            temporary_files(&path).is_empty(),
            "a successful save renames its temporary file away: {:?}",
            temporary_files(&path)
        );
        // The temporary file is created in the target's own directory, so the rename is a
        // same-volume replacement; the bytes now live under the target name and nothing else.
        assert!(path.is_file());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_failed_write_leaves_the_original_byte_identical_and_no_temporary_file() {
        let path = scratch("atomic-fault");
        let original = b"a = 1\nb = 2\n";
        std::fs::write(&path, original).expect("write the fixture");
        let before = probe(&path).snapshot().expect("readable");
        let error = write_faulted(&path, b"a = 9\nb = 9\n", 4).expect_err("the injected write fails");
        assert!(
            error.contains("injected write failure"),
            "the failure is reported: {error}"
        );
        assert_eq!(
            std::fs::read(&path).expect("read back"),
            original,
            "the original is byte-identical after a failed write"
        );
        assert_eq!(
            probe(&path).snapshot(),
            Some(before),
            "size, time and content hash of the original are unchanged"
        );
        assert!(
            temporary_files(&path).is_empty(),
            "the failed write cleans its temporary file up: {:?}",
            temporary_files(&path)
        );
        let _ = std::fs::remove_file(&path);
    }

    /// Windows refuses to rename over a read-only target; the original must survive untouched.
    #[test]
    #[cfg(windows)]
    fn a_failed_rename_keeps_the_read_only_original_byte_identical() {
        let path = scratch("atomic-readonly");
        let original = b"a = 1\n";
        std::fs::write(&path, original).expect("write the fixture");
        let before = probe(&path).snapshot().expect("readable");
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&path, permissions).expect("make the file read-only");
        let error = write(&path, b"a = 2\n").expect_err("a read-only target cannot be replaced");
        assert!(error.contains("Cannot save"), "{error}");
        assert_eq!(
            std::fs::read(&path).expect("read back"),
            original,
            "the read-only original is byte-identical after the failed rename"
        );
        assert_eq!(probe(&path).snapshot(), Some(before));
        assert!(
            temporary_files(&path).is_empty(),
            "the failed rename cleans its temporary file up: {:?}",
            temporary_files(&path)
        );
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        permissions.set_readonly(false);
        std::fs::set_permissions(&path, permissions).expect("restore writability");
        let _ = std::fs::remove_file(&path);
    }

    /// The hidden attribute is the observable Windows attribute a save must carry over.
    #[test]
    #[cfg(windows)]
    fn a_successful_save_preserves_the_targets_file_attributes() {
        use std::io::Write;
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        let path = scratch("atomic-attributes");
        let _ = std::fs::remove_file(&path);
        {
            let mut hidden = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .attributes(FILE_ATTRIBUTE_HIDDEN)
                .open(&path)
                .expect("create a hidden fixture");
            hidden.write_all(b"a = 1\n").expect("write the fixture");
        }
        assert_ne!(
            std::fs::metadata(&path)
                .expect("metadata")
                .file_attributes()
                & FILE_ATTRIBUTE_HIDDEN,
            0,
            "the fixture starts hidden"
        );
        write(&path, b"a = 2\n").expect("the write succeeds");
        assert_eq!(std::fs::read(&path).expect("read back"), b"a = 2\n");
        assert_ne!(
            std::fs::metadata(&path)
                .expect("metadata")
                .file_attributes()
                & FILE_ATTRIBUTE_HIDDEN,
            0,
            "the replacement keeps the hidden attribute"
        );
        let _ = std::fs::remove_file(&path);
    }
}
