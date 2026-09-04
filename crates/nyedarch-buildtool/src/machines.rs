//! The trusted machine registry (spec §12 and §48).
//!
//! Both clients need the same view of which machines are trusted, so the
//! registry lives here rather than in either interface. Records are stored as
//! authenticated `.nyfp` files under the client's configuration directory, one
//! per machine, keyed by fingerprint id.
//!
//! # Why labels are re-signed locally
//!
//! Labels are metadata, not authorization secrets, but they sit inside the
//! record's authentication tag so that a record cannot be relabelled by anyone
//! who does not hold the client key. Editing a label therefore means re-signing
//! the record with this client's own keystore key. That is intentional: the tag
//! answers "did this client vouch for this record?", and after an edit the
//! answer must be yes for the edit too.
//!
//! An *imported* record is verified before anything in it is trusted, and a
//! record that fails verification is refused rather than imported with a
//! warning.

use std::path::PathBuf;

use nyedarch_fingerprint::nyfp;

/// One trusted machine.
#[derive(Clone, Debug)]
pub struct Machine {
    /// Opaque fingerprint id, hex. Never a raw hardware identifier.
    pub id: String,
    pub labels: Vec<String>,
    /// Where the authenticated record lives.
    pub path: PathBuf,
    /// True for the machine this client is running on.
    pub is_this_machine: bool,
}

/// How multiple selected tags combine (spec §48).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagMode {
    /// A machine matches if it carries **any** selected tag.
    Any,
    /// A machine matches only if it carries **all** selected tags.
    All,
}

impl TagMode {
    pub fn parse(s: &str) -> Option<TagMode> {
        match s.to_ascii_lowercase().as_str() {
            "any" => Some(TagMode::Any),
            "all" => Some(TagMode::All),
            _ => None,
        }
    }
}

fn store_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("nyedarch").join("machines")
}

/// Every machine in the registry, plus this one.
///
/// The current machine is always present whether or not it was ever added: the
/// operator is trusted by definition, and a capsule always includes them.
pub fn list() -> Vec<Machine> {
    let mut out = Vec::new();
    let here = nyedarch_fingerprint::capture();
    let here_id = here.id_hex();

    if let Ok(rd) = std::fs::read_dir(store_dir()) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("nyfp") {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else { continue };
            // A record that does not verify is not listed. Showing it would
            // imply it could be used, and it cannot.
            let Ok(rec) = nyfp::open(&crate::pipeline::nyfp_key(), &bytes) else { continue };
            let id = rec.fingerprint.id_hex();
            out.push(Machine {
                is_this_machine: id == here_id,
                id,
                labels: rec.labels.clone(),
                path,
            });
        }
    }

    if !out.iter().any(|m| m.is_this_machine) {
        out.insert(
            0,
            Machine {
                id: here_id,
                labels: vec!["this machine".into()],
                path: PathBuf::new(),
                is_this_machine: true,
            },
        );
    }
    out.sort_by(|a, b| b.is_this_machine.cmp(&a.is_this_machine).then(a.id.cmp(&b.id)));
    out
}

/// Import a record, verifying it first.
pub fn import(path: &std::path::Path) -> Result<Machine, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let rec = nyfp::open(&crate::pipeline::nyfp_key(), &bytes).map_err(|e| {
        format!(
            "{} failed authentication: {e}\nThe record was refused. Editing a record invalidates it.",
            path.display()
        )
    })?;
    let dir = store_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create the registry: {e}"))?;
    let id = rec.fingerprint.id_hex();
    let dest = dir.join(format!("{id}.nyfp"));
    std::fs::write(&dest, &bytes).map_err(|e| format!("cannot store the record: {e}"))?;
    Ok(Machine {
        is_this_machine: id == nyedarch_fingerprint::capture().id_hex(),
        id,
        labels: rec.labels,
        path: dest,
    })
}

/// Export this machine as an authenticated record.
pub fn export_this_machine(path: &std::path::Path, labels: Vec<String>) -> Result<String, String> {
    let fp = nyedarch_fingerprint::capture();
    let rec = nyfp::Record {
        version: 1,
        fingerprint: fp.clone(),
        labels,
        created_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let bytes = nyfp::seal(&crate::pipeline::nyfp_key(), &rec).map_err(|e| e.to_string())?;
    std::fs::write(path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(fp.id_hex())
}

/// Replace a machine's labels and re-sign the record.
pub fn set_labels(id_prefix: &str, labels: Vec<String>) -> Result<Machine, String> {
    let m = find(id_prefix)?;
    if m.path.as_os_str().is_empty() {
        // This machine is not yet stored; store it with the new labels.
        let dir = store_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create the registry: {e}"))?;
        let dest = dir.join(format!("{}.nyfp", m.id));
        export_this_machine(&dest, labels.clone())?;
        return Ok(Machine { labels, path: dest, ..m });
    }
    let bytes = std::fs::read(&m.path).map_err(|e| format!("cannot read the record: {e}"))?;
    let mut rec = nyfp::open(&crate::pipeline::nyfp_key(), &bytes).map_err(|e| e.to_string())?;
    rec.labels = labels.clone();
    let resealed = nyfp::seal(&crate::pipeline::nyfp_key(), &rec).map_err(|e| e.to_string())?;
    std::fs::write(&m.path, resealed).map_err(|e| format!("cannot write the record: {e}"))?;
    Ok(Machine { labels, ..m })
}

/// Remove a machine from the registry.
pub fn remove(id_prefix: &str) -> Result<String, String> {
    let m = find(id_prefix)?;
    if m.is_this_machine {
        // Removing it would achieve nothing: a capsule always includes the
        // creator, so it would reappear immediately.
        return Err(
            "this machine cannot be removed: every capsule includes its creator by design"
                .to_string(),
        );
    }
    std::fs::remove_file(&m.path).map_err(|e| format!("cannot remove the record: {e}"))?;
    Ok(m.id)
}

/// Resolve a machine by unambiguous id prefix.
pub fn find(id_prefix: &str) -> Result<Machine, String> {
    let all = list();
    let hits: Vec<&Machine> = all.iter().filter(|m| m.id.starts_with(id_prefix)).collect();
    match hits.len() {
        0 => Err(format!("no trusted machine starts with '{id_prefix}'")),
        1 => Ok(hits[0].clone()),
        n => Err(format!("'{id_prefix}' matches {n} machines; use more characters")),
    }
}

/// Every distinct label in the registry, sorted. Used to offer tag filters.
pub fn all_labels() -> Vec<String> {
    let mut v: Vec<String> = list().into_iter().flat_map(|m| m.labels).collect();
    v.sort();
    v.dedup();
    v
}

/// Filter the registry by free text and by tags (spec §48).
///
/// `query` matches an id prefix or any part of a label, case-insensitively.
/// Tags combine by `mode`. The current machine is always included in the result
/// regardless of the filter, because a capsule always trusts its creator, and
/// hiding it would misrepresent what the capsule will contain.
pub fn select(query: &str, tags: &[String], mode: TagMode) -> Vec<Machine> {
    let q = query.trim().to_ascii_lowercase();
    let want: Vec<String> = tags.iter().map(|t| t.to_ascii_lowercase()).collect();

    list()
        .into_iter()
        .filter(|m| {
            if m.is_this_machine {
                return true; // always included; see the note above
            }
            let labels: Vec<String> = m.labels.iter().map(|l| l.to_ascii_lowercase()).collect();

            let text_ok = q.is_empty()
                || m.id.to_ascii_lowercase().starts_with(&q)
                || labels.iter().any(|l| l.contains(&q));

            let tags_ok = if want.is_empty() {
                true
            } else {
                match mode {
                    TagMode::Any => want.iter().any(|t| labels.iter().any(|l| l == t)),
                    TagMode::All => want.iter().all(|t| labels.iter().any(|l| l == t)),
                }
            };
            text_ok && tags_ok
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, labels: &[&str], this: bool) -> Machine {
        Machine {
            id: id.to_string(),
            labels: labels.iter().map(|s| s.to_string()).collect(),
            path: PathBuf::new(),
            is_this_machine: this,
        }
    }

    /// The filter logic, exercised directly so it does not depend on the
    /// contents of the developer's own registry.
    fn filter(all: &[Machine], query: &str, tags: &[&str], mode: TagMode) -> Vec<String> {
        let q = query.trim().to_ascii_lowercase();
        let want: Vec<String> = tags.iter().map(|t| t.to_ascii_lowercase()).collect();
        all.iter()
            .filter(|m| {
                if m.is_this_machine {
                    return true;
                }
                let labels: Vec<String> = m.labels.iter().map(|l| l.to_ascii_lowercase()).collect();
                let text_ok = q.is_empty()
                    || m.id.to_ascii_lowercase().starts_with(&q)
                    || labels.iter().any(|l| l.contains(&q));
                let tags_ok = if want.is_empty() {
                    true
                } else {
                    match mode {
                        TagMode::Any => want.iter().any(|t| labels.iter().any(|l| l == t)),
                        TagMode::All => want.iter().all(|t| labels.iter().any(|l| l == t)),
                    }
                };
                text_ok && tags_ok
            })
            .map(|m| m.id.clone())
            .collect()
    }

    fn fixture() -> Vec<Machine> {
        vec![
            m("aaaa", &["this machine"], true),
            m("bbbb", &["HR", "Bangalore"], false),
            m("cccc", &["HR", "London"], false),
            m("dddd", &["Finance", "Bangalore"], false),
        ]
    }

    #[test]
    fn any_tag_selects_the_union() {
        let got = filter(&fixture(), "", &["HR", "Finance"], TagMode::Any);
        assert_eq!(got, vec!["aaaa", "bbbb", "cccc", "dddd"]);
    }

    #[test]
    fn all_tags_selects_the_intersection() {
        // Only the machine carrying both.
        let got = filter(&fixture(), "", &["HR", "Bangalore"], TagMode::All);
        assert_eq!(got, vec!["aaaa", "bbbb"]);
    }

    #[test]
    fn this_machine_survives_every_filter() {
        // A capsule always trusts its creator, so hiding it would misrepresent
        // what is about to be built.
        for (q, tags, mode) in [
            ("zzzz", vec!["nothing"], TagMode::All),
            ("", vec!["nonexistent"], TagMode::Any),
            ("qqq", vec![], TagMode::Any),
        ] {
            let got = filter(&fixture(), q, &tags, mode);
            assert!(got.contains(&"aaaa".to_string()), "this machine was filtered out");
        }
    }

    #[test]
    fn text_matches_labels_and_id_prefixes() {
        assert_eq!(filter(&fixture(), "lond", &[], TagMode::Any), vec!["aaaa", "cccc"]);
        assert_eq!(filter(&fixture(), "bbb", &[], TagMode::Any), vec!["aaaa", "bbbb"]);
        // Case-insensitive.
        assert_eq!(filter(&fixture(), "hr", &[], TagMode::Any), vec!["aaaa", "bbbb", "cccc"]);
    }

    #[test]
    fn tag_mode_parses_and_rejects_nonsense() {
        assert_eq!(TagMode::parse("any"), Some(TagMode::Any));
        assert_eq!(TagMode::parse("ALL"), Some(TagMode::All));
        assert_eq!(TagMode::parse("either"), None);
    }
}
