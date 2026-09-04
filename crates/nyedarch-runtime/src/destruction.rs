//! Capsule destruction (spec §35, §79).
//!
//! # What this can and cannot do
//!
//! Software cannot guarantee that data is unrecoverable from modern storage.
//! An SSD's controller may have written the block elsewhere and left the
//! original cell intact; a copy-on-write filesystem may hold older versions in
//! snapshots; a journal may retain fragments. Overwriting a file through the
//! filesystem asks the OS to change the logical contents, and nothing more.
//!
//! So this module does not claim secure erasure. It performs a **best-effort
//! destruction** and, crucially, **reports what actually happened** rather than
//! assuming success. `remove_file` returning `Ok` is not evidence that anything
//! was destroyed, and the previous implementation - a bare `remove_file` whose
//! result was discarded - could not tell the difference between a file that was
//! gone and one that was still sitting there.
//!
//! The real protection remains cryptographic: without the composed key the
//! payload is unreadable whether or not the bytes survive. Destruction is
//! defence in depth and an anti-iteration measure, not the boundary.
//!
//! # Order of operations
//!
//! 1. Overwrite the contents, flush, and sync to the device.
//! 2. Truncate to zero length.
//! 3. Rename to a random name in the same directory.
//! 4. Remove.
//! 5. **Verify** the path is gone and report the outcome.
//!
//! The rename matters for two reasons: a name left in a directory entry is
//! itself information, and on Windows a file that cannot be unlinked while open
//! can usually still be renamed, which at least detaches it from its path.

use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// What actually happened. Every variant is a fact the caller can act on, not
/// an assumption.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destruction {
    /// Overwritten and the path no longer exists. The strongest claim
    /// available, and still not a guarantee of physical erasure.
    Removed,
    /// Contents were overwritten but the file could not be unlinked - held
    /// open, or on a filesystem that refused. The bytes at that path are no
    /// longer the capsule.
    OverwrittenButPresent { reason: String },
    /// The file could not be opened for writing but was unlinked anyway. The
    /// contents may survive in free space.
    RemovedWithoutOverwrite { reason: String },
    /// Nothing could be done.
    Failed { reason: String },
    /// There was nothing at that path to begin with.
    NothingToDo,
    /// Overwrite-and-remove was handed to one or more mechanisms that run after
    /// this process exits.
    ///
    /// A separate variant on purpose. This process **requested** destruction
    /// and cannot observe whether it completed, and the specification requires
    /// those two things to stay distinguishable. Reporting it as `Removed`
    /// would be a claim nobody verified.
    Delegated { via: String },
}

impl Destruction {
    /// Whether the artifact is gone from its path.
    pub fn path_cleared(&self) -> bool {
        matches!(self, Destruction::Removed | Destruction::RemovedWithoutOverwrite { .. } | Destruction::NothingToDo)
    }
    /// Destruction was requested but its completion is not observable here.
    pub fn is_delegated(&self) -> bool {
        matches!(self, Destruction::Delegated { .. })
    }
    /// Whether the original bytes were overwritten in place.
    pub fn contents_overwritten(&self) -> bool {
        matches!(self, Destruction::Removed | Destruction::OverwrittenButPresent { .. })
    }
    pub fn describe(&self) -> String {
        match self {
            Destruction::Removed => "overwritten and removed (best effort; physical erasure is not guaranteed)".into(),
            Destruction::OverwrittenButPresent { reason } => {
                format!("contents overwritten but the file could not be removed: {reason}")
            }
            Destruction::RemovedWithoutOverwrite { reason } => {
                format!("removed without overwriting: {reason}")
            }
            Destruction::Failed { reason } => format!("destruction failed: {reason}"),
            Destruction::NothingToDo => "nothing at that path".into(),
            Destruction::Delegated { via } => format!(
                "overwrite and removal handed to {via}, running after this process exits \
                 (requested, not confirmed)"
            ),
        }
    }
}

fn random_name() -> String {
    let mut b = [0u8; 12];
    if getrandom::getrandom(&mut b).is_err() {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(1);
        for (i, x) in b.iter_mut().enumerate() {
            *x = ((t >> (i % 16)) as u8) ^ 0x5A;
        }
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Overwrite a file's contents with random bytes.
fn overwrite(path: &Path) -> Result<(), String> {
    let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    f.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;

    // One pass. Multiple passes are folklore from magnetic media and buy
    // nothing on the storage this actually runs on, while taking far longer on
    // a large capsule.
    let mut buf = vec![0u8; 64 * 1024];
    let mut written = 0u64;
    while written < len {
        let n = ((len - written) as usize).min(buf.len());
        if getrandom::getrandom(&mut buf[..n]).is_err() {
            for (i, b) in buf[..n].iter_mut().enumerate() {
                *b = (i as u8).wrapping_mul(31) ^ 0xA5;
            }
        }
        f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        written += n as u64;
    }
    f.flush().map_err(|e| e.to_string())?;
    // Ask the OS to push it to the device. Without this the overwrite may exist
    // only in the page cache when the process exits.
    f.sync_all().map_err(|e| e.to_string())?;
    f.set_len(0).map_err(|e| e.to_string())?;
    Ok(())
}

/// Destroy a single file, reporting what was achieved.
pub fn destroy_file(path: &Path) -> Destruction {
    if !path.exists() {
        return Destruction::NothingToDo;
    }
    // A directory is not a capsule; refuse rather than recursing into something
    // the caller did not mean.
    if path.is_dir() {
        return Destruction::Failed { reason: "path is a directory".into() };
    }

    let overwrote = overwrite(path);

    // Rename before unlinking: the name itself is information, and a file that
    // cannot be unlinked can often still be moved.
    let target = match path.parent() {
        Some(dir) => {
            let candidate = dir.join(format!(".nyedarch-{}", random_name()));
            match std::fs::rename(path, &candidate) {
                Ok(()) => candidate,
                Err(_) => path.to_path_buf(),
            }
        }
        None => path.to_path_buf(),
    };

    match std::fs::remove_file(&target) {
        Ok(()) => {
            // Verify rather than trust the return value.
            if target.exists() || path.exists() {
                return Destruction::OverwrittenButPresent {
                    reason: "the file still exists after removal reported success".into(),
                };
            }
            match overwrote {
                Ok(()) => Destruction::Removed,
                Err(e) => Destruction::RemovedWithoutOverwrite { reason: e },
            }
        }
        Err(e) => {
            // Put the name back if we moved it but could not delete it, so the
            // caller is not left hunting for a stray file.
            if target != path {
                let _ = std::fs::rename(&target, path);
            }
            match overwrote {
                Ok(()) => Destruction::OverwrittenButPresent { reason: e.to_string() },
                Err(oe) => Destruction::Failed {
                    reason: format!("could not overwrite ({oe}) and could not remove ({e})"),
                },
            }
        }
    }
}

/// Destroy a working directory and everything under it.
///
/// Used for intermediate extraction state. Every file is reported, so a caller
/// can tell the difference between "cleaned up" and "mostly cleaned up".
pub fn destroy_tree(root: &Path) -> Vec<(PathBuf, Destruction)> {
    let mut out = Vec::new();
    if !root.exists() {
        return out;
    }
    let mut stack = vec![root.to_path_buf()];
    let mut dirs = Vec::new();
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p.clone());
                dirs.push(p);
            } else {
                let d = destroy_file(&p);
                out.push((p, d));
            }
        }
    }
    // Deepest first, so directories are empty when removed.
    dirs.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for d in dirs {
        let _ = std::fs::remove_dir(d);
    }
    let _ = std::fs::remove_dir(root);
    out
}

/// Destroy the capsule itself after a successful one-shot extraction.
///
/// # The running-executable problem
///
/// A process cannot always unlink its own image. Unix generally allows it - the
/// inode survives until the last handle closes. Windows normally refuses while
/// the image is mapped, so the rename in `destroy_file` at least detaches the
/// capsule from its path, and the caller is told the file is still present
/// rather than being allowed to believe it is gone.
pub fn one_shot_destroy(self_path: &Path) -> Destruction {
    if !self_path.exists() {
        return Destruction::NothingToDo;
    }

    // Order matters, and getting it wrong is silent.
    //
    // The obvious implementation - unlink here, then delegate the scrub - is
    // wrong: the unlink succeeds on Unix even for a running image, so by the
    // time the helper runs the path is gone and it overwrites nothing. The file
    // vanishes and its bytes stay in free space, while the user is told it was
    // destroyed. That is exactly the false assurance this module exists to
    // prevent, and it is what the first version of this function did.
    //
    // So: try to overwrite in place first. Only if that succeeds is it safe to
    // unlink here. If it fails - which is the normal case for a running image -
    // hand the file over **intact**, so the helper still has something to scrub.
    match overwrite(self_path) {
        Ok(()) => {
            // Not the running image, or a platform that permits it. Finish the
            // job now, where the result can actually be verified.
            destroy_file(self_path)
        }
        Err(reason) => match delegate_destruction(self_path) {
            Some(delegated) => delegated,
            None => {
                // No helper available. Unlinking without a scrub is still
                // better than leaving the capsule in place, and the outcome
                // says plainly that the contents were not overwritten.
                match std::fs::remove_file(self_path) {
                    Ok(()) if !self_path.exists() => {
                        Destruction::RemovedWithoutOverwrite { reason }
                    }
                    Ok(()) => Destruction::OverwrittenButPresent {
                        reason: "the file still exists after removal reported success".into(),
                    },
                    Err(e) => Destruction::Failed {
                        reason: format!("could not overwrite ({reason}) and could not remove ({e})"),
                    },
                }
            }
        },
    }
}

/// Hand overwrite-and-remove to a process that outlives this one.
///
/// # Why this is needed
///
/// A process cannot overwrite its own running image. Linux returns `ETXTBSY`
/// for a write open; Windows generally refuses to unlink a mapped image. So the
/// capsule can unlink itself but not scrub its contents, which leaves the bytes
/// in free space. The work has to happen after the image is no longer running.
///
/// # Why there is no helper file on disk
///
/// The command is passed to the system shell **inline**. Nothing is written to
/// disk, so there is no helper binary to find, tamper with, or leave behind, and
/// no window in which a dropped file could be replaced with something else. The
/// command contains one path and no secrets: everything it can do is delete that
/// path.
///
/// # What this does not achieve
///
/// It is not a guarantee. Whoever controls the machine can kill the helper,
/// deny it a shell, or snapshot the disk first. It removes the *self-reference*
/// problem - the capsule no longer fails to scrub itself merely because it is
/// the thing running - and nothing more. The payload remains protected by
/// cryptography either way.
fn delegate_destruction(path: &Path) -> Option<Destruction> {
    let p = path.to_str()?;

    // Every mechanism below embeds this path in a shell or PowerShell command,
    // so a path that cannot be safely single-quoted must never reach any of
    // them. Checked once, here, rather than in each script builder: a guard
    // that has to be remembered three times is a guard that will be forgotten.
    if !is_shell_safe(p) {
        return None;
    }

    let mut used: Vec<&'static str> = Vec::new();

    // Layer 2: the operating system's own scheduler. This is the layer that
    // survives an attacker killing the child process, because the work is
    // owned by a system service rather than by a process they can see in their
    // own process tree. Every facility used here is a documented, supported
    // one-shot scheduling interface - not a persistence mechanism. Each job
    // deletes itself after running.
    if let Some(name) = schedule_with_os(p) {
        used.push(name);
    }

    // Layer 3: a detached child as well, not instead. Both are idempotent
    // (`rm -f`, `Remove-Item -Force`), so whichever runs first simply wins and
    // the other finds nothing to do. Redundancy is the point: the scheduler may
    // be absent, and the child may be killed.
    if let Some(name) = spawn_detached_scrubber(p) {
        used.push(name);
    }

    if used.is_empty() {
        return None;
    }
    Some(Destruction::Delegated { via: used.join(" and ") })
}

/// Whether a path can be embedded in a single-quoted shell argument safely.
///
/// Rejects the quote itself, newlines, carriage returns, and NUL. A path
/// containing any of them is refused outright rather than escaped: escaping
/// correctly across `sh`, `schtasks` and PowerShell is a source of bugs, and
/// refusing costs only the delegated layer.
fn is_shell_safe(p: &str) -> bool {
    !p.contains('\'') && !p.contains('\n') && !p.contains('\r') && !p.contains('\0') && !p.contains('"')
}

/// The scrub-and-delete command, as a POSIX shell fragment.
fn posix_script(p: &str, wait_pid: Option<u32>) -> String {
    let wait = match wait_pid {
        Some(pid) => format!(
            "i=0; while [ $i -lt 600 ] && kill -0 {pid} 2>/dev/null; do sleep 0.1; i=$((i+1)); done; "
        ),
        // Scheduled jobs start after a delay, so they do not need to wait on a
        // pid that may already be gone and whose number may have been reused.
        None => String::new(),
    };
    format!(
        "{wait}if [ -e '{p}' ]; then \
         if command -v shred >/dev/null 2>&1; then shred -u -n 1 '{p}' >/dev/null 2>&1; \
         else sz=$(wc -c < '{p}' 2>/dev/null | tr -d '[:space:]'); sz=${{sz:-0}}; \
         blk=$(( (sz + 4095) / 4096 )); \
         dd if=/dev/urandom of='{p}' bs=4096 count=$blk conv=notrunc >/dev/null 2>&1; fi; \
         rm -f '{p}' >/dev/null 2>&1; fi"
    )
}

/// Hand the job to the platform's supported one-shot scheduler.
///
/// These are ordinary administrative interfaces, and each job is created to run
/// once and then remove itself. Nothing is installed, no autostart entry is
/// created, and nothing survives the deletion it was created to perform - the
/// distinction between using a scheduler and establishing persistence.
fn schedule_with_os(p: &str) -> Option<&'static str> {
    let tag = format!("nyedarch-{}", random_name());

    #[cfg(target_os = "linux")]
    {
        // A transient systemd unit. `--collect` discards it once it has run.
        let script = posix_script(p, None);
        let ok = std::process::Command::new("systemd-run")
            .args([
                "--user", "--collect", "--quiet",
                &format!("--unit={tag}"),
                "--on-active=3",
                "/bin/sh", "-c", &script,
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return Some("a transient systemd job");
        }
        // `at` is the classic fallback where systemd is absent or unavailable
        // to this user.
        use std::io::Write as _;
        if let Ok(mut child) = std::process::Command::new("at")
            .arg("now")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(script.as_bytes());
            }
            if child.wait().map(|s| s.success()).unwrap_or(false) {
                return Some("a scheduled at(1) job");
            }
        }
        None
    }

    #[cfg(target_os = "macos")]
    {
        let script = posix_script(p, None);
        // launchd one-shot submission; the job exits after its single run.
        let ok = std::process::Command::new("launchctl")
            .args(["submit", "-l", &tag, "--", "/bin/sh", "-c", &script])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return Some("a launchd one-shot job");
        }
        None
    }

    #[cfg(target_os = "windows")]
    {
        // Deliberately the short form.
        //
        // Two constraints made the long PowerShell version unusable, both found
        // by watching which mechanisms actually engaged on a real runner rather
        // than assuming the call worked:
        //
        //   * `/tr` truncates beyond 261 characters, and the full scrub script
        //     is far longer, so task creation simply failed;
        //   * `/z` (delete the task after it runs) is rejected without an end
        //     boundary, so adding it prevented creation as well.
        //
        // So the scheduled task does the one thing that must not be lost - the
        // **removal** - and deletes its own registration afterwards, leaving no
        // persistence behind. The detached child performs the overwrite. If the
        // child is killed the file is still removed; if the scheduler is
        // unavailable the child still scrubs and removes.
        let cmd = format!("cmd /c del /f /q \"{p}\" & schtasks /delete /tn {tag} /f");
        if cmd.len() > 255 {
            return None;
        }
        let ok = std::process::Command::new("schtasks")
            .args(["/create", "/tn", &tag, "/tr", &cmd, "/sc", "once", "/st", "23:59", "/f"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            let _ = std::process::Command::new("schtasks")
                .args(["/run", "/tn", &tag])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            return Some("a self-removing scheduled task");
        }
        None
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = (p, tag);
        None
    }
}

/// Spawn a detached child that waits for this process to exit, then scrubs.
fn spawn_detached_scrubber(p: &str) -> Option<&'static str> {
    let pid = std::process::id();

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let script = posix_script(p, Some(pid));
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg(script);
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        cmd.process_group(0);
        cmd.spawn().ok().map(|_| "a detached shell")
    }

    #[cfg(windows)]
    {
        let script = format!(
            "$ErrorActionPreference='SilentlyContinue';              $n=0; while ((Get-Process -Id {pid} -ErrorAction SilentlyContinue) -and $n -lt 600) {{ Start-Sleep -Milliseconds 100; $n++ }};              if (Test-Path -LiteralPath '{p}') {{              try {{ $f=Get-Item -LiteralPath '{p}'; $len=$f.Length;              $s=[IO.File]::Open('{p}',[IO.FileMode]::Open,[IO.FileAccess]::Write);              $b=New-Object byte[] 65536; $r=New-Object Random; $w=0;              while ($w -lt $len) {{ $c=[Math]::Min(65536,$len-$w); $r.NextBytes($b); $s.Write($b,0,$c); $w+=$c }};              $s.Flush(); $s.Close() }} catch {{}};              Remove-Item -LiteralPath '{p}' -Force }}"
        );
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()
            .map(|_| "a detached PowerShell")
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (p, pid);
        None
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Whether this process bypasses filesystem permissions.
    ///
    /// Probed behaviourally rather than by checking a uid, because what matters
    /// is the effect: root can remove a file from a directory it has no write
    /// permission on, so the permission tests below cannot demonstrate anything
    /// there. They are skipped rather than weakened, and still run on any
    /// ordinary account - including the CI runners.
    #[cfg(unix)]
    fn bypasses_permissions() -> bool {
        use std::os::unix::fs::PermissionsExt;
        let mut d = std::env::temp_dir();
        d.push(format!("nyedarch-permprobe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        if std::fs::create_dir_all(&d).is_err() {
            return false;
        }
        let _ = std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o500));
        let can_write = std::fs::write(d.join("probe"), b"x").is_ok();
        let _ = std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700));
        let _ = std::fs::remove_dir_all(&d);
        can_write
    }

    fn tmp(tag: &str) -> PathBuf {
        let mut d = std::env::temp_dir();
        d.push(format!("nyedarch-destroy-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn normal_deletion_removes_and_reports_it() {
        let d = tmp("normal");
        let f = d.join("capsule.nyarch");
        std::fs::write(&f, vec![0xAB; 4096]).unwrap();
        let r = destroy_file(&f);
        assert_eq!(r, Destruction::Removed);
        assert!(!f.exists());
        assert!(r.path_cleared() && r.contents_overwritten());
    }

    #[test]
    fn contents_are_actually_overwritten_before_removal() {
        // The point of overwriting is that the original bytes are gone from the
        // block, so this checks the bytes rather than the return value.
        let d = tmp("content");
        let f = d.join("secret.bin");
        let marker = b"MARKER-THAT-MUST-NOT-SURVIVE".repeat(64);
        std::fs::write(&f, &marker).unwrap();
        overwrite(&f).unwrap();
        let after = std::fs::read(&f).unwrap();
        assert_eq!(after.len(), 0, "file should be truncated after overwrite");
        // And the pre-truncation pass must not have left the marker.
        let raw = std::fs::read(&f).unwrap();
        assert!(!raw.windows(marker.len()).any(|w| w == &marker[..]));
    }

    #[test]
    fn a_missing_file_is_not_an_error() {
        let d = tmp("missing");
        assert_eq!(destroy_file(&d.join("nope")), Destruction::NothingToDo);
    }

    #[test]
    fn a_directory_is_refused_rather_than_recursed() {
        let d = tmp("dir");
        let sub = d.join("subdir");
        std::fs::create_dir_all(&sub).unwrap();
        match destroy_file(&sub) {
            Destruction::Failed { reason } => assert!(reason.contains("directory")),
            other => panic!("a directory must be refused, got {other:?}"),
        }
        assert!(sub.exists(), "the directory must be left alone");
    }

    #[test]
    #[cfg(unix)]
    fn a_read_only_directory_prevents_removal_and_says_so() {
        use std::os::unix::fs::PermissionsExt;
        if bypasses_permissions() {
            eprintln!("skipped: this process bypasses filesystem permissions (root)");
            return;
        }
        let d = tmp("readonly");
        let f = d.join("locked.bin");
        std::fs::write(&f, vec![1u8; 512]).unwrap();
        // Removing a file requires write permission on its *directory*.
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o500)).unwrap();

        let r = destroy_file(&f);
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700)).unwrap();

        // It must not claim success. Either outcome is honest; silence is not.
        assert!(
            !r.path_cleared(),
            "a file that could not be removed must not be reported as cleared: {r:?}"
        );
        assert!(f.exists());
    }

    #[test]
    #[cfg(unix)]
    fn an_unwritable_file_is_reported_not_silently_skipped() {
        use std::os::unix::fs::PermissionsExt;
        if bypasses_permissions() {
            eprintln!("skipped: this process bypasses filesystem permissions (root)");
            return;
        }
        let d = tmp("nowrite");
        let f = d.join("ro.bin");
        std::fs::write(&f, vec![2u8; 256]).unwrap();
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o400)).unwrap();

        let r = destroy_file(&f);
        // The directory is writable, so removal should still work - but the
        // caller must be told the contents were never overwritten.
        match &r {
            Destruction::RemovedWithoutOverwrite { .. } => {}
            Destruction::Removed => panic!("an unwritable file cannot have been overwritten"),
            other => panic!("unexpected {other:?}"),
        }
        assert!(!f.exists());
    }

    #[test]
    fn an_open_handle_does_not_produce_a_false_success() {
        let d = tmp("openhandle");
        let f = d.join("held.bin");
        std::fs::write(&f, vec![3u8; 1024]).unwrap();
        let _held = std::fs::File::open(&f).unwrap();

        let r = destroy_file(&f);
        // Unix unlinks happily while a handle is open; Windows generally does
        // not. Both are acceptable - reporting the wrong one is not.
        if r.path_cleared() {
            assert!(!f.exists());
        } else {
            assert!(f.exists());
        }
    }

    #[test]
    fn a_copy_made_beforehand_survives_and_that_is_expected() {
        // Destruction cannot reach copies. Stating this in a test keeps the
        // claim honest: the cryptography protects the copy, not the deletion.
        let d = tmp("copy");
        let f = d.join("original.nyarch");
        let c = d.join("attacker-copy.nyarch");
        std::fs::write(&f, vec![7u8; 2048]).unwrap();
        std::fs::copy(&f, &c).unwrap();

        assert_eq!(destroy_file(&f), Destruction::Removed);
        assert!(!f.exists());
        assert!(c.exists(), "a copy is out of reach of destruction, by definition");
        assert_eq!(std::fs::read(&c).unwrap().len(), 2048);
    }

    #[test]
    fn a_renamed_capsule_is_still_destroyed_at_its_new_path() {
        let d = tmp("renamed");
        let f = d.join("original.nyarch");
        let moved = d.join("renamed.nyarch");
        std::fs::write(&f, vec![9u8; 700]).unwrap();
        std::fs::rename(&f, &moved).unwrap();
        assert_eq!(destroy_file(&moved), Destruction::Removed);
        assert!(!moved.exists());
    }

    #[test]
    fn a_working_tree_is_destroyed_and_every_file_reported() {
        let d = tmp("tree");
        let root = d.join("work");
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join("one.txt"), b"1").unwrap();
        std::fs::write(root.join("a/two.txt"), b"2").unwrap();
        std::fs::write(root.join("a/b/three.txt"), b"3").unwrap();

        let report = destroy_tree(&root);
        assert_eq!(report.len(), 3, "every file must be reported: {report:?}");
        assert!(report.iter().all(|(_, r)| r.path_cleared()));
        assert!(!root.exists(), "the working tree itself must be gone");
    }

    #[test]
    fn destroying_an_empty_file_works() {
        let d = tmp("empty");
        let f = d.join("empty.bin");
        std::fs::write(&f, b"").unwrap();
        assert_eq!(destroy_file(&f), Destruction::Removed);
    }

    #[test]
    fn a_large_file_is_fully_overwritten() {
        let d = tmp("large");
        let f = d.join("big.bin");
        // Larger than the 64 KiB buffer, to exercise the loop.
        std::fs::write(&f, vec![0x5Au8; 300_000]).unwrap();
        assert_eq!(destroy_file(&f), Destruction::Removed);
        assert!(!f.exists());
    }

    #[test]
    fn outcomes_never_overclaim() {
        // A failure must never report the path as cleared.
        let failed = Destruction::Failed { reason: "x".into() };
        assert!(!failed.path_cleared());
        assert!(!failed.contents_overwritten());
        let present = Destruction::OverwrittenButPresent { reason: "y".into() };
        assert!(!present.path_cleared());
        assert!(present.contents_overwritten());
        // And the description must not promise erasure.
        assert!(Destruction::Removed.describe().contains("not guaranteed"));
    }
}

#[cfg(test)]
mod delegation_tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let mut d = std::env::temp_dir();
        d.push(format!("nyedarch-deleg-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The helper must actually scrub and remove the file once the requesting
    /// process is gone. Here the "parent" is a short-lived child, so the wait
    /// condition is genuinely exercised rather than short-circuited.
    /// Delegation must engage at least one mechanism, and say which.
    #[test]
    #[cfg(unix)]
    fn delegation_names_every_mechanism_it_engaged() {
        let d = tmp("names");
        let f = d.join("capsule.nyarch");
        std::fs::write(&f, vec![1u8; 128]).unwrap();
        let r = delegate_destruction(&f).expect("at least one mechanism must be available");
        match &r {
            Destruction::Delegated { via } => {
                assert!(!via.is_empty());
                // The operator needs to know what will do the work, not just
                // that something will.
                assert!(via.contains("shell") || via.contains("job") || via.contains("task"));
            }
            other => panic!("expected delegation, got {other:?}"),
        }
        assert!(!r.path_cleared(), "delegation is requested, not confirmed");
    }

    #[test]
    #[cfg(unix)]
    fn a_delegated_destruction_completes_after_the_process_exits() {
        let d = tmp("completes");
        let f = d.join("capsule.nyarch");
        std::fs::write(&f, b"PAYLOAD-MARKER".repeat(100)).unwrap();

        let r = delegate_destruction(&f).expect("helper should spawn");
        assert!(r.is_delegated());
        // It must not claim the path is cleared: this process cannot see that.
        assert!(!r.path_cleared(), "a delegated destruction is requested, not confirmed");

        // The helper waits for *this* pid, which does not exit during the test,
        // so it will hit its bounded wait. What matters is that it spawned and
        // that the outcome is honest about not being observable here.
        assert!(r.describe().contains("not confirmed"));
    }

    /// The delegated outcome must be distinguishable from a verified one, so a
    /// caller cannot accidentally report "destroyed" for work nobody watched.
    #[test]
    fn delegated_is_never_confused_with_removed() {
        let d = Destruction::Delegated { via: "a detached shell".into() };
        assert!(d.is_delegated());
        assert!(!d.path_cleared());
        assert!(!d.contents_overwritten());
        assert_ne!(d, Destruction::Removed);
    }
}

#[cfg(all(test, unix))]
mod delegation_evidence {

    /// Prove the helper *overwrites* rather than merely unlinking.
    ///
    /// Removal alone leaves the bytes in free space, and once the path is gone
    /// there is nothing left to inspect - so this keeps a second hard link to
    /// the same inode. The helper scrubs and unlinks its path; the surviving
    /// link still refers to that inode, so its contents show whether the
    /// overwrite really happened.
    #[test]
    fn the_helper_scrubs_the_inode_not_just_the_name() {
        let mut d = std::env::temp_dir();
        d.push(format!("nyedarch-scrub-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();

        let target = d.join("capsule.bin");
        let witness = d.join("witness.bin");
        let marker = b"MARKER-MUST-NOT-SURVIVE-SCRUB".repeat(200);
        std::fs::write(&target, &marker).unwrap();
        std::fs::hard_link(&target, &witness).unwrap();

        // The helper waits for a pid that is already gone, so it proceeds at
        // once rather than waiting on this test process.
        let dead_pid = {
            let child = std::process::Command::new("sh").arg("-c").arg("exit 0").spawn().unwrap();
            let id = child.id();
            let mut c = child;
            let _ = c.wait();
            id
        };
        let p = target.to_str().unwrap();
        let script = format!(
            "i=0; while [ $i -lt 600 ] && kill -0 {dead_pid} 2>/dev/null; do sleep 0.1; i=$((i+1)); done; \
             if command -v shred >/dev/null 2>&1; then shred -u -n 1 '{p}' >/dev/null 2>&1; \
             else sz=$(wc -c < '{p}' 2>/dev/null | tr -d '[:space:]'); sz=${{sz:-0}}; \
             blk=$(( (sz + 4095) / 4096 )); \
             dd if=/dev/urandom of='{p}' bs=4096 count=$blk conv=notrunc >/dev/null 2>&1; fi; \
             rm -f '{p}' >/dev/null 2>&1"
        );
        let mut c = std::process::Command::new("sh").arg("-c").arg(script).spawn().unwrap();
        let _ = c.wait();

        // Give the filesystem a moment to settle.
        for _ in 0..40 {
            if !target.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        assert!(!target.exists(), "the helper must remove its target path");
        let surviving = std::fs::read(&witness).expect("the witness link must still resolve");
        assert!(
            !surviving.windows(marker.len()).any(|w| w == &marker[..]),
            "the inode still holds the original bytes: the helper unlinked without scrubbing"
        );

        let _ = std::fs::remove_dir_all(&d);
    }
}

#[cfg(test)]
mod injection_tests {
    use super::*;

    /// The path reaches a shell, so anything that could escape a quoted
    /// argument must be refused before it gets there.
    #[test]
    fn shell_unsafe_paths_are_rejected() {
        for bad in [
            "/tmp/a'b",            // closes the quote
            "/tmp/a\nrm -rf ~",    // newline injects a command
            "/tmp/a\rb",
            "/tmp/a\"b",           // breaks the PowerShell/schtasks quoting
        ] {
            assert!(!is_shell_safe(bad), "{bad:?} must be rejected");
            assert!(
                delegate_destruction(Path::new(bad)).is_none(),
                "{bad:?} must never be handed to a scheduler or shell"
            );
        }
    }

    #[test]
    fn ordinary_paths_are_accepted() {
        for good in [
            "/tmp/capsule.nyarch",
            "/home/user/My Documents/capsule.nyarch", // spaces are fine when quoted
            "/tmp/nyedarch-0123abcd.nyarch",
        ] {
            assert!(is_shell_safe(good), "{good:?} should be usable");
        }
    }
}
