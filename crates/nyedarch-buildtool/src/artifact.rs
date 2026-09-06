//! Reading a build artifact.
//!
//! GitHub returns artifacts as a zip. The user wants the capsule inside it, not
//! the container, so this extracts it.
//!
//! Deliberately minimal: it reads local file headers in order and handles the
//! two compression methods an Actions artifact actually uses. A general zip
//! implementation would be a much larger attack surface for no benefit, and
//! this input arrives from an untrusted build environment.
//!
//! Every length is bounded against the remaining input before it is used, so a
//! malformed or hostile archive produces `None` rather than a panic or a huge
//! allocation.

/// One file recovered from an archive.
pub struct Entry {
    pub name: String,
    pub data: Vec<u8>,
}

const LOCAL_HEADER: u32 = 0x0403_4b50;
const STORED: u16 = 0;
const DEFLATE: u16 = 8;

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}
fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

/// Extract every file an artifact contains.
///
/// Directory entries are skipped. Returns an empty vector rather than an error
/// for an archive it cannot read: the caller decides what a missing capsule
/// means, and it is never "carry on as though it worked".
pub fn read_all(bytes: &[u8]) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut i = 0usize;

    while i + 30 <= bytes.len() {
        match u32_at(bytes, i) {
            Some(LOCAL_HEADER) => {}
            // Anything else means the local-header run has ended - the central
            // directory follows, and there is nothing more to extract.
            _ => break,
        }
        let Some(flags) = u16_at(bytes, i + 6) else { break };
        let Some(method) = u16_at(bytes, i + 8) else { break };
        let Some(csize) = u32_at(bytes, i + 18) else { break };
        let Some(usize_) = u32_at(bytes, i + 22) else { break };
        let Some(name_len) = u16_at(bytes, i + 26) else { break };
        let Some(extra_len) = u16_at(bytes, i + 28) else { break };

        let name_at = i + 30;
        let data_at = name_at + name_len as usize + extra_len as usize;
        if data_at > bytes.len() {
            break;
        }
        let name = String::from_utf8_lossy(&bytes[name_at..name_at + name_len as usize]).to_string();

        // Bit 3 means the sizes live in a trailing data descriptor rather than
        // the header. Scanning for that is where zip parsers grow teeth, and an
        // Actions artifact does not use it, so this stops instead of guessing.
        if flags & 0x08 != 0 {
            break;
        }

        let end = data_at.saturating_add(csize as usize);
        if end > bytes.len() {
            break;
        }
        let raw = &bytes[data_at..end];

        if !name.ends_with('/') {
            let data = match method {
                STORED => Some(raw.to_vec()),
                DEFLATE => {
                    // Bounded by the header's stated uncompressed size, so a
                    // hostile archive cannot ask for an unbounded allocation.
                    miniz_oxide::inflate::decompress_to_vec_with_limit(
                        raw,
                        (usize_ as usize).saturating_add(1024),
                    )
                    .ok()
                }
                _ => None,
            };
            if let Some(data) = data {
                out.push(Entry { name, data });
            }
        }
        i = end;
    }
    out
}

/// The capsule inside an artifact.
///
/// An artifact holds exactly one built capsule, but the name varies by target,
/// so this picks the entry that looks like an executable rather than matching a
/// fixed name. When only one file is present, that is the answer.
pub fn capsule_from_artifact(bytes: &[u8]) -> Option<Entry> {
    let mut entries = read_all(bytes);
    if entries.is_empty() {
        return None;
    }
    if entries.len() == 1 {
        return entries.pop();
    }
    // Prefer something obviously the capsule; otherwise the largest file, which
    // a compiled binary always is next to a manifest or a log.
    if let Some(i) = entries.iter().position(|e| {
        let n = e.name.to_ascii_lowercase();
        n.contains("capsule") || n.ends_with(".nyarch") || n.ends_with(".exe")
    }) {
        return Some(entries.swap_remove(i));
    }
    entries.sort_by_key(|e| e.data.len());
    entries.pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal stored-entry zip by hand.
    fn stored_zip(name: &str, data: &[u8]) -> Vec<u8> {
        let mut z = Vec::new();
        z.extend_from_slice(&LOCAL_HEADER.to_le_bytes());
        z.extend_from_slice(&[20, 0]); // version
        z.extend_from_slice(&[0, 0]); // flags
        z.extend_from_slice(&STORED.to_le_bytes());
        z.extend_from_slice(&[0, 0, 0, 0]); // time, date
        z.extend_from_slice(&0u32.to_le_bytes()); // crc, unchecked here
        z.extend_from_slice(&(data.len() as u32).to_le_bytes());
        z.extend_from_slice(&(data.len() as u32).to_le_bytes());
        z.extend_from_slice(&(name.len() as u16).to_le_bytes());
        z.extend_from_slice(&0u16.to_le_bytes());
        z.extend_from_slice(name.as_bytes());
        z.extend_from_slice(data);
        z
    }

    #[test]
    fn a_single_stored_entry_is_recovered() {
        let z = stored_zip("nyedarch-capsule", b"ELF-ish payload");
        let e = capsule_from_artifact(&z).expect("entry");
        assert_eq!(e.name, "nyedarch-capsule");
        assert_eq!(e.data, b"ELF-ish payload");
    }

    #[test]
    fn the_capsule_is_chosen_from_several_entries() {
        let mut z = stored_zip("build-log.txt", b"noise");
        z.extend_from_slice(&stored_zip("nyedarch-capsule.exe", b"the real binary"));
        let e = capsule_from_artifact(&z).expect("entry");
        assert_eq!(e.data, b"the real binary");
    }

    /// A malformed archive must return nothing rather than panic: it arrives
    /// from an untrusted build environment.
    #[test]
    fn malformed_archives_are_refused_without_panicking() {
        for bad in [
            vec![],
            vec![0u8; 8],
            vec![0xFFu8; 512],
            b"PK\x03\x04".to_vec(),
        ] {
            assert!(capsule_from_artifact(&bad).is_none());
        }
        // A header claiming more data than exists must not read past the end.
        let mut z = stored_zip("x", b"12345");
        let n = z.len();
        z[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
        let _ = n;
        assert!(capsule_from_artifact(&z).is_none());
    }
}
