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
const CENTRAL_HEADER: u32 = 0x0201_4b50;
const EOCD: u32 = 0x0605_4b50;
const STORED: u16 = 0;
const DEFLATE: u16 = 8;

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}
fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

/// Extract every file, reading the **central directory**.
///
/// This is the only reliable way to read a GitHub artifact. Actions streams the
/// zip as it is written, so each local header carries the "sizes follow the
/// data" flag with zeroes where the lengths belong. Walking local headers - the
/// obvious approach, and the first one here - stops at the first such entry and
/// finds nothing, which is exactly what happened: the artifact downloaded
/// correctly and then "no capsule could be read from it".
///
/// The central directory sits at the end and always carries real sizes and
/// offsets, whatever the local headers say.
fn read_central(bytes: &[u8]) -> Vec<Entry> {
    let mut out = Vec::new();

    // The end-of-central-directory record is last, possibly followed by a
    // comment, so scan backwards for it.
    let mut eocd = None;
    let start = bytes.len().saturating_sub(66_000);
    for i in (start..bytes.len().saturating_sub(21)).rev() {
        if u32_at(bytes, i) == Some(EOCD) {
            eocd = Some(i);
            break;
        }
    }
    let Some(eocd) = eocd else { return out };

    let Some(count) = u16_at(bytes, eocd + 10) else { return out };
    let Some(cd_off) = u32_at(bytes, eocd + 16) else { return out };
    let mut i = cd_off as usize;

    for _ in 0..count {
        if u32_at(bytes, i) != Some(CENTRAL_HEADER) {
            break;
        }
        let Some(method) = u16_at(bytes, i + 10) else { break };
        let Some(csize) = u32_at(bytes, i + 20) else { break };
        let Some(usize_) = u32_at(bytes, i + 24) else { break };
        let Some(name_len) = u16_at(bytes, i + 28) else { break };
        let Some(extra_len) = u16_at(bytes, i + 30) else { break };
        let Some(cmt_len) = u16_at(bytes, i + 32) else { break };
        let Some(local_off) = u32_at(bytes, i + 42) else { break };

        let name_at = i + 46;
        if name_at + name_len as usize > bytes.len() {
            break;
        }
        let name =
            String::from_utf8_lossy(&bytes[name_at..name_at + name_len as usize]).to_string();

        // The data begins after the *local* header, whose name and extra fields
        // may differ in length from the central one.
        let lo = local_off as usize;
        if u32_at(bytes, lo) == Some(LOCAL_HEADER) {
            if let (Some(lname), Some(lextra)) = (u16_at(bytes, lo + 26), u16_at(bytes, lo + 28)) {
                let data_at = lo + 30 + lname as usize + lextra as usize;
                let end = data_at.saturating_add(csize as usize);
                if end <= bytes.len() && !name.ends_with('/') {
                    let raw = &bytes[data_at..end];
                    let data = match method {
                        STORED => Some(raw.to_vec()),
                        DEFLATE => miniz_oxide::inflate::decompress_to_vec_with_limit(
                            raw,
                            (usize_ as usize).saturating_add(1024),
                        )
                        .ok(),
                        _ => None,
                    };
                    if let Some(data) = data {
                        out.push(Entry { name, data });
                    }
                }
            }
        }
        i = name_at + name_len as usize + extra_len as usize + cmt_len as usize;
    }
    out
}

/// Extract every file an artifact contains.
///
/// Directory entries are skipped. Returns an empty vector rather than an error
/// for an archive it cannot read: the caller decides what a missing capsule
/// means, and it is never "carry on as though it worked".
pub fn read_all(bytes: &[u8]) -> Vec<Entry> {
    // The central directory first: it is authoritative, and it is the only
    // thing that works for a streamed archive.
    let central = read_central(bytes);
    if !central.is_empty() {
        return central;
    }

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

#[cfg(test)]
mod provenance_tests {
    use super::*;

    /// The commitment lives in the capsule, not in the archive around it.
    ///
    /// The check originally ran on the raw artifact bytes. GitHub deflates the
    /// capsule inside the zip, so the commitment was never present there: every
    /// build was refused and nothing was ever delivered. This asserts the two
    /// halves of that - invisible before extraction, present after.
    #[test]
    fn the_commitment_is_only_visible_after_extraction() {
        let commitment = [0x7Cu8; 32];

        // A capsule carrying the commitment, in compressible surroundings so
        // deflate genuinely rewrites the bytes.
        let mut capsule = b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".repeat(64);
        capsule.extend_from_slice(&commitment);
        capsule.extend_from_slice(&b"BBBBBBBBBBBBBBBB".repeat(64));

        let deflated = miniz_oxide::deflate::compress_to_vec(&capsule, 6);

        let mut zip = Vec::new();
        zip.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        zip.extend_from_slice(&[20, 0]);
        zip.extend_from_slice(&[0, 0]);
        zip.extend_from_slice(&8u16.to_le_bytes()); // deflate
        zip.extend_from_slice(&[0, 0, 0, 0]);
        zip.extend_from_slice(&0u32.to_le_bytes());
        zip.extend_from_slice(&(deflated.len() as u32).to_le_bytes());
        zip.extend_from_slice(&(capsule.len() as u32).to_le_bytes());
        zip.extend_from_slice(&(b"nyedarch-capsule".len() as u16).to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(b"nyedarch-capsule");
        zip.extend_from_slice(&deflated);

        let in_archive = zip.windows(32).any(|w| w == commitment);
        assert!(
            !in_archive,
            "the commitment must not be findable in the compressed archive - \
             if it were, this test could not catch the bug it exists for"
        );

        let entry = capsule_from_artifact(&zip).expect("the capsule extracts");
        assert!(
            entry.data.windows(32).any(|w| w == commitment),
            "the commitment must be present once the capsule is extracted"
        );
        assert_eq!(entry.data, capsule, "extraction must be exact");
    }
}
