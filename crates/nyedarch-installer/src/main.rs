//! NYEDArch installer.
//!
//! One binary, identical behaviour on Windows, macOS, and Linux. It installs a
//! release directory into a per-user location, verifies integrity first, and
//! tells the user exactly what it did.
//!
//! Design constraints, deliberately chosen:
//!
//! * **No elevation.** Installing to a per-user directory avoids asking for
//!   administrator or root rights. A security tool that demands elevation to
//!   install has a larger blast radius than it needs.
//! * **Verify before writing.** Checksums are checked against `SHA256SUMS`
//!   before a single file is copied, so a truncated or corrupted download fails
//!   before it can half-install.
//! * **Reversible.** `--uninstall` removes exactly what was installed, using a
//!   manifest written at install time rather than guessing.
//! * **Honest about PATH.** The installer does not silently edit shell profiles
//!   or the registry. It prints the exact line to add, because modifying a
//!   user's environment behind their back is precisely the behaviour people
//!   distrust in installers.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

const APP: &str = "NYEDArch";
const MANIFEST: &str = "installed-files.txt";
const BUNDLE_ID: &str = "net.nyedarch.client";

/// The `Info.plist` for the macOS application bundle.
///
/// The usage description is the load-bearing part. macOS refuses location to a
/// process with no bundle carrying `NSLocationWhenInUseUsageDescription`, so
/// without this file CoreLocation is denied no matter how correct the code is,
/// and NYEDArch falls back to the browser consent flow. Installing as a bundle
/// is what makes the native provider reachable at all.
pub fn info_plist(version: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>{APP}</string>
  <key>CFBundleDisplayName</key><string>{APP}</string>
  <key>CFBundleIdentifier</key><string>{BUNDLE_ID}</string>
  <key>CFBundleVersion</key><string>{version}</string>
  <key>CFBundleShortVersionString</key><string>{version}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleExecutable</key><string>{APP}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <!-- Shown by macOS in its own permission prompt. Location is optional and is
       only requested when a capsule is built with the location protection. -->
  <key>NSLocationWhenInUseUsageDescription</key>
  <string>NYEDArch uses your location only when you enable the location protection, so a capsule can be bound to the place you choose. Your coordinates are converted to a region identifier and are never stored or transmitted.</string>
  <key>NSLocationUsageDescription</key>
  <string>NYEDArch uses your location only when you enable the location protection, so a capsule can be bound to the place you choose.</string>
</dict>
</plist>
"#
    )
}

// ------------------------------------------------------------------ output --

fn styled(code: &str, s: &str) -> String {
    // Only colour when attached to a terminal that plausibly supports it.
    let tty = std::env::var_os("NO_COLOR").is_none()
        && (cfg!(unix) || std::env::var_os("WT_SESSION").is_some() || std::env::var_os("TERM").is_some());
    if tty {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}
fn bold(s: &str) -> String { styled("1", s) }
fn dim(s: &str) -> String { styled("2", s) }
fn ok_s(s: &str) -> String { styled("32", s) }
fn warn_s(s: &str) -> String { styled("33", s) }
fn err_s(s: &str) -> String { styled("31", s) }

fn step(msg: &str) { println!("\n{} {}", bold("==>"), bold(msg)); }
fn ok(msg: &str) { println!("    {} {}", ok_s("[ok]"), msg); }
fn warn(msg: &str) { println!("    {} {}", warn_s("!"), msg); }
fn fail(msg: &str) { eprintln!("    {} {}", err_s("[X]"), msg); }

// ------------------------------------------------------------- destinations --

/// True when installing as a macOS application bundle.
pub fn is_bundle_target() -> bool {
    cfg!(target_os = "macos")
}

/// Where a release item lands inside a `.app` bundle.
///
/// The executables must sit in `Contents/MacOS`, and everything else in
/// `Contents/Resources`, or macOS will not treat the directory as an
/// application. The GUI is renamed to the bundle name because
/// `CFBundleExecutable` must match.
pub fn bundle_destination(item: &str) -> Option<String> {
    Some(match item {
        "bin" => return None, // handled file by file
        "runtime-src" => "Contents/Resources/runtime-src".to_string(),
        "docs" => "Contents/Resources/docs".to_string(),
        // macOS reads the icon from the bundle, not from the running process,
        // so it has to be placed rather than compiled in.
        "AppIcon.icns" | "icon.png" => "Contents/Resources/AppIcon.icns".to_string(),
        "install" | "install.exe" => "Contents/MacOS/install".to_string(),
        MANIFEST => format!("Contents/{MANIFEST}"),
        other => format!("Contents/Resources/{other}"),
    })
}

/// Per-user install root, following each platform's convention.
fn default_prefix() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        // %LOCALAPPDATA%\Programs\NYEDArch
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local).join("Programs").join(APP);
        }
        PathBuf::from(".").join(APP)
    }
    #[cfg(target_os = "macos")]
    {
        // ~/Applications/NYEDArch.app - a real application bundle, per-user so
        // no administrator rights are needed.
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Applications")
                .join(format!("{APP}.app"));
        }
        PathBuf::from(".").join(format!("{APP}.app"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // XDG: ~/.local/share/nyedarch
        let dir = APP.to_ascii_lowercase();
        if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(data).join(&dir);
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".local").join("share").join(&dir);
        }
        PathBuf::from(".").join(&dir)
    }
}

/// Where a user's executables normally live, for the PATH hint.
fn user_bin_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        None // Windows has no conventional per-user bin dir; PATH is edited instead.
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("bin"))
    }
}

fn exe_name(stem: &str) -> String {
    if cfg!(windows) { format!("{stem}.exe") } else { stem.to_string() }
}

// -------------------------------------------------------------- source dir --

/// Locate the release directory: the folder containing this installer.
fn source_dir() -> io::Result<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if dir.join("SHA256SUMS").is_file() || dir.join("bin").is_dir() {
                return Ok(dir.to_path_buf());
            }
            // Installer may sit in bin/ inside the release.
            if let Some(parent) = dir.parent() {
                if parent.join("SHA256SUMS").is_file() || parent.join("bin").is_dir() {
                    return Ok(parent.to_path_buf());
                }
            }
        }
    }
    let cwd = std::env::current_dir()?;
    if cwd.join("bin").is_dir() {
        return Ok(cwd);
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "could not locate the release directory; run this installer from inside it",
    ))
}

// ------------------------------------------------------------ verification --

fn sha256_file(path: &Path) -> io::Result<String> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    io::copy(&mut f, &mut h)?;
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Verify every file listed in SHA256SUMS. Returns (checked, mismatches).
fn verify(src: &Path) -> io::Result<(usize, Vec<String>)> {
    let sums = src.join("SHA256SUMS");
    if !sums.is_file() {
        return Ok((0, vec![]));
    }
    let text = fs::read_to_string(&sums)?;
    let mut checked = 0usize;
    let mut bad = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Format: "<hex>  <path>"
        let (hex, rel) = match line.split_once("  ") {
            Some(x) => x,
            None => continue,
        };
        let rel = rel.trim_start_matches("./");
        if rel == "SHA256SUMS" {
            continue;
        }
        let path = src.join(rel);
        if !path.is_file() {
            bad.push(format!("missing: {rel}"));
            continue;
        }
        match sha256_file(&path) {
            Ok(actual) if actual == hex => checked += 1,
            Ok(_) => bad.push(format!("checksum mismatch: {rel}")),
            Err(e) => bad.push(format!("unreadable: {rel} ({e})")),
        }
    }
    Ok((checked, bad))
}

// ------------------------------------------------------------------- copy ----

fn copy_tree(src: &Path, dst: &Path, installed: &mut Vec<String>, root: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to, installed, root)?;
        } else {
            fs::copy(&from, &to)?;
            preserve_exec(&from, &to);
            if let Ok(rel) = to.strip_prefix(root) {
                installed.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    Ok(())
}

/// Keep the executable bit on Unix. Windows has no such bit; executability
/// there follows from the file extension.
fn preserve_exec(from: &Path, to: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(md) = fs::metadata(from) {
            let mode = md.permissions().mode();
            if mode & 0o111 != 0 {
                let _ = fs::set_permissions(to, fs::Permissions::from_mode(mode | 0o755));
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (from, to);
    }
}

// ------------------------------------------------------------------- main ----

struct Opts {
    prefix: Option<PathBuf>,
    uninstall: bool,
    verify_only: bool,
    assume_yes: bool,
    force: bool,
}

fn parse_args() -> Result<Opts, String> {
    let mut o = Opts { prefix: None, uninstall: false, verify_only: false, assume_yes: false, force: false };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--prefix" => {
                i += 1;
                o.prefix = Some(PathBuf::from(args.get(i).ok_or("--prefix needs a directory")?));
            }
            "--uninstall" => o.uninstall = true,
            "--verify" => o.verify_only = true,
            "--yes" | "-y" => o.assume_yes = true,
            "--force" => o.force = true,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown option: {other}")),
        }
        i += 1;
    }
    Ok(o)
}

fn print_help() {
    println!("{}", bold("NYEDArch installer"));
    println!();
    println!("  nyedarch-install                 install to the default per-user location");
    println!("  nyedarch-install --prefix DIR    install somewhere specific");
    println!("  nyedarch-install --verify        check integrity only, install nothing");
    println!("  nyedarch-install --uninstall     remove a previous installation");
    println!("  nyedarch-install --yes           do not prompt");
    println!("  nyedarch-install --force         overwrite an existing installation");
    println!();
    println!("  No administrator or root rights are required, and no shell profile");
    println!("  or registry key is modified. PATH instructions are printed for you");
    println!("  to apply yourself.");
}

fn confirm(prompt: &str, assume_yes: bool) -> bool {
    if assume_yes {
        return true;
    }
    print!("    {prompt} [y/N]: ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

fn main() {
    let opts = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            fail(&e);
            eprintln!("    try --help");
            std::process::exit(2);
        }
    };

    println!("{}", bold("NYEDArch - Not Your Everyday Archive"));
    println!("{}", dim("installer"));

    let prefix = opts.prefix.clone().unwrap_or_else(default_prefix);

    if opts.uninstall {
        std::process::exit(uninstall(&prefix));
    }

    let src = match source_dir() {
        Ok(s) => s,
        Err(e) => {
            fail(&e.to_string());
            std::process::exit(1);
        }
    };

    step("Verifying release integrity");
    match verify(&src) {
        Ok((0, _)) => warn("no SHA256SUMS found — skipping integrity check"),
        Ok((n, bad)) if bad.is_empty() => ok(&format!("{n} files verified")),
        Ok((_, bad)) => {
            for b in bad.iter().take(10) {
                fail(b);
            }
            fail("release integrity check FAILED");
            eprintln!();
            eprintln!("    Do not install this copy. Re-download it.");
            std::process::exit(1);
        }
        Err(e) => {
            fail(&format!("could not verify: {e}"));
            std::process::exit(1);
        }
    }
    println!("    {}", dim("Checksums confirm the files arrived intact. They are not a"));
    println!("    {}", dim("substitute for a signed release, and prove nothing if they came"));
    println!("    {}", dim("from the same place as the files."));

    if opts.verify_only {
        println!("\n{} verification only; nothing installed.", ok_s("Done."));
        return;
    }

    step("Install location");
    println!("    {}", prefix.display());
    if prefix.exists() && !opts.force {
        warn("an installation already exists there");
        if !confirm("Overwrite it?", opts.assume_yes) {
            println!("\n    Cancelled. Nothing was changed.");
            std::process::exit(1);
        }
        let _ = remove_previous(&prefix);
    }

    step("Installing");
    let mut installed: Vec<String> = Vec::new();

    // The installer copies itself, so `--uninstall` still works after the
    // distribution media is discarded - which is the normal case.
    let self_name = if cfg!(windows) { "install.exe" } else { "install" };

    let items = [
        self_name,
        "bin", "runtime-src", "docs",
        "START_HERE.md", "README.md", "EULA.md",
        "BUILD_INFO.txt", "BENCHMARKS.txt", "SHA256SUMS",
        "NYEDArch_Technical_Documentation.docx",
        "NYEDArch_Technical_Documentation.pdf",
    ];

    // Destination for a release item. On macOS everything is laid out inside an
    // application bundle; elsewhere the release structure is kept as-is.
    let dest_for = |item: &str| -> Option<PathBuf> {
        if is_bundle_target() {
            bundle_destination(item).map(|rel| prefix.join(rel))
        } else {
            Some(prefix.join(item))
        }
    };

    let copy_item = |item: &str, installed: &mut Vec<String>| -> io::Result<()> {
        let from = src.join(item);
        if !from.exists() {
            return Ok(());
        }

        // `bin` is handled per-file so executables reach Contents/MacOS and the
        // GUI can be renamed to match CFBundleExecutable.
        if item == "bin" && is_bundle_target() {
            let macos_dir = prefix.join("Contents/MacOS");
            fs::create_dir_all(&macos_dir)?;
            for entry in fs::read_dir(&from)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                // CFBundleExecutable must match the bundle name.
                let target_name = if name == "nyedarch-gui" { APP.to_string() } else { name };
                let to = macos_dir.join(&target_name);
                fs::copy(entry.path(), &to)?;
                preserve_exec(&entry.path(), &to);
                if let Ok(rel) = to.strip_prefix(&prefix) {
                    installed.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
            ok("Contents/MacOS (executables)");
            return Ok(());
        }

        let Some(to) = dest_for(item) else { return Ok(()) };
        if from.is_dir() {
            copy_tree(&from, &to, installed, &prefix)?;
        } else {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&from, &to)?;
            preserve_exec(&from, &to);
            if let Ok(rel) = to.strip_prefix(&prefix) {
                installed.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
        ok(item);
        Ok(())
    };

    for item in items {
        if let Err(e) = copy_item(item, &mut installed) {
            fail(&format!("failed installing {item}: {e}"));
            fail("rolling back");
            let _ = remove_previous(&prefix);
            std::process::exit(1);
        }
    }

    // The bundle is only an application once Info.plist exists.
    if is_bundle_target() {
        let contents = prefix.join("Contents");
        if let Err(e) = fs::create_dir_all(&contents)
            .and_then(|_| fs::write(contents.join("Info.plist"), info_plist(env!("CARGO_PKG_VERSION"))))
        {
            fail(&format!("could not write Info.plist: {e}"));
            let _ = remove_previous(&prefix);
            std::process::exit(1);
        }
        installed.push("Contents/Info.plist".to_string());
        ok("Contents/Info.plist (declares the location usage description)");

        // PkgInfo is legacy but costs nothing and keeps Finder happy.
        let _ = fs::write(contents.join("PkgInfo"), "APPL????");
        installed.push("Contents/PkgInfo".to_string());

        adhoc_sign(&prefix);
    }

    // Record what was installed, so uninstall removes exactly this and no more.
    //
    // Inside a bundle this must live under Contents/: anything at the bundle
    // root is not part of the application as macOS understands it, and a real
    // Mac showed it sitting outside.
    let manifest = if is_bundle_target() {
        prefix.join("Contents").join(MANIFEST)
    } else {
        prefix.join(MANIFEST)
    };
    let mut body = String::new();
    for f in &installed {
        body.push_str(f);
        body.push('\n');
    }
    if fs::write(&manifest, body).is_err() {
        warn("could not write the install manifest; --uninstall will be less precise");
    }
    ok(&format!("{} files installed", installed.len()));

    step("Finishing up");
    // Inside a bundle the executables live in Contents/MacOS, not bin/.
    let bin_dir = if is_bundle_target() {
        prefix.join("Contents/MacOS")
    } else {
        prefix.join("bin")
    };
    let cli = bin_dir.join(exe_name("nyedarch"));

    // On Unix, offer a symlink into the conventional user bin directory.
    #[cfg(not(target_os = "windows"))]
    if let Some(ubin) = user_bin_dir() {
        if fs::create_dir_all(&ubin).is_ok() {
            let link = ubin.join("nyedarch");
            let _ = fs::remove_file(&link);
            match std::os::unix::fs::symlink(&cli, &link) {
                Ok(()) => ok(&format!("linked {} -> {}", link.display(), cli.display())),
                Err(_) => warn("could not create a symlink in ~/.local/bin"),
            }
        }
    }

    println!();
    println!("{}", bold("Installed."));
    println!();
    println!("  Client:   {}", cli.display());
    let gui = if is_bundle_target() {
        bin_dir.join(APP)
    } else {
        bin_dir.join(exe_name("nyedarch-gui"))
    };
    if gui.exists() {
        println!("  Desktop:  {}", gui.display());
    }
    if is_bundle_target() {
        println!("  Bundle:   {}", prefix.display());
    }
    let res = if is_bundle_target() { prefix.join("Contents/Resources") } else { prefix.clone() };
    println!("  Docs:     {}", res.join("docs").display());
    println!("  Guide:    {}", res.join("START_HERE.md").display());
    println!();

    println!("{}", bold("Add it to your PATH"));
    if cfg!(windows) {
        println!("  PowerShell (current user, permanent):");
        println!("    {}", dim(&format!(
            "[Environment]::SetEnvironmentVariable('Path', $env:Path + ';{}', 'User')",
            bin_dir.display()
        )));
        println!();
        println!("{}", bold("Optional: run .nyarch capsules from a terminal"));
        println!("  Windows decides what is executable from the file extension, so a");
        println!("  capsule will not run by name until .NYARCH is in PATHEXT:");
        println!("    {}", dim(
            "[Environment]::SetEnvironmentVariable('PATHEXT', $env:PATHEXT + ';.NYARCH', 'User')"
        ));
        println!("  Without this, use: {}", dim("nyedarch run capsule.nyarch"));
    } else {
        println!("  Add to your shell profile if ~/.local/bin is not already on PATH:");
        println!("    {}", dim("export PATH=\"$HOME/.local/bin:$PATH\""));
    }

    if is_bundle_target() {
        println!();
        println!("{}", bold("Location on macOS"));
        println!("  Installing as an application bundle is what makes the native");
        println!("  provider usable: macOS refuses location to a process with no");
        println!("  bundle declaring a usage description. Launch NYEDArch from");
        println!("  {} so the permission prompt appears.", dim("Applications"));
        println!("  Run from a bare terminal instead and location falls back to");
        println!("  the browser consent flow, which also works.");
    }

    println!();
    println!("{}", bold("Next"));
    println!("  1. Read {}", res.join("EULA.md").display());
    println!("  2. Read {}", res.join("START_HERE.md").display());
    println!("  3. Seal something:");
    println!("     {}", dim("nyedarch seal ./my_folder ./capsule_project \"a strong passphrase\""));
    println!();
    println!("  {}", warn_s("A capsule is not a backup. Lose the passphrase, the authorized"));
    println!("  {}", warn_s("machine, or your key material and the data is unrecoverable."));
    println!();
}

/// Ad-hoc sign the bundle so macOS treats it as a stable identity.
///
/// This is **not** a substitute for Developer ID signing and notarisation. It
/// gives the bundle a consistent code identity, which is what lets macOS
/// remember a location permission decision instead of re-prompting or refusing
/// outright. Distributing to other people still requires real signing.
fn adhoc_sign(prefix: &Path) {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("codesign")
            .args(["--force", "--deep", "--sign", "-"])
            .arg(prefix)
            .output();
        match out {
            Ok(o) if o.status.success() => ok("ad-hoc signed"),
            Ok(_) | Err(_) => warn(
                "could not ad-hoc sign the bundle. macOS may refuse to remember a location \
                 permission decision; the browser fallback still works.",
            ),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = prefix;
    }
}

fn remove_previous(prefix: &Path) -> io::Result<()> {
    // Prefer the manifest so we only remove what we installed.
    let manifest = if is_bundle_target() {
        let inside = prefix.join("Contents").join(MANIFEST);
        if inside.exists() { inside } else { prefix.join(MANIFEST) }
    } else {
        prefix.join(MANIFEST)
    };
    if let Ok(text) = fs::read_to_string(&manifest) {
        let mut dirs: BTreeMap<PathBuf, ()> = BTreeMap::new();
        for rel in text.lines().filter(|l| !l.trim().is_empty()) {
            let p = prefix.join(rel);
            let _ = fs::remove_file(&p);
            if let Some(d) = p.parent() {
                dirs.insert(d.to_path_buf(), ());
            }
        }
        let _ = fs::remove_file(&manifest);
        drop(dirs);
        // Prune empty directories bottom-up. Collecting only the parents of
        // removed files is not enough: intermediate directories such as
        // runtime-src/<crate>/src leave empty ancestors behind.
        prune_empty_dirs(prefix);
        let _ = fs::remove_dir(prefix);
        return Ok(());
    }
    // No manifest: remove the directory only if it looks like ours.
    if prefix.join("bin").exists() && prefix.join("docs").exists() {
        return fs::remove_dir_all(prefix);
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "refusing to delete a directory that does not look like a NYEDArch installation",
    ))
}

/// Depth-first removal of empty directories under `dir`.
fn prune_empty_dirs(dir: &Path) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            prune_empty_dirs(&p);
            let _ = fs::remove_dir(&p); // only succeeds when empty
        }
    }
}

fn uninstall(prefix: &Path) -> i32 {
    step("Uninstalling");
    println!("    {}", prefix.display());
    if !prefix.exists() {
        warn("nothing installed there");
        return 0;
    }
    match remove_previous(prefix) {
        Ok(()) => {
            if prefix.exists() {
                warn(&format!("some files could not be removed; check {}", prefix.display()));
            }
            #[cfg(not(target_os = "windows"))]
            if let Some(ubin) = user_bin_dir() {
                let link = ubin.join("nyedarch");
                // symlink_metadata does NOT follow the link. Once the target is
                // gone the link is dangling, and exists() would report false —
                // leaving a broken symlink on the user's PATH.
                if fs::symlink_metadata(&link).is_ok() {
                    let _ = fs::remove_file(&link);
                    ok("removed the ~/.local/bin symlink");
                }
            }
            ok("removed");
            println!();
            println!("    {}", dim("Capsules you created are untouched, and your EULA"));
            println!("    {}", dim("acceptance record in your config directory remains."));
            0
        }
        Err(e) => {
            fail(&e.to_string());
            1
        }
    }
}

#[cfg(test)]
mod bundle_tests {
    use super::*;

    /// The usage description is the reason the bundle exists. Without it macOS
    /// refuses location outright, so this is asserted rather than assumed.
    #[test]
    fn info_plist_declares_location_usage() {
        let p = info_plist("0.0.1");
        assert!(p.contains("NSLocationWhenInUseUsageDescription"));
        assert!(p.contains("NSLocationUsageDescription"));
        // It must say what the location is used for, not just claim a right to it.
        assert!(p.contains("location protection"));
        assert!(p.contains("never stored or transmitted"));
    }

    /// `CFBundleExecutable` must match the file actually placed in
    /// Contents/MacOS, or macOS will not launch the bundle.
    #[test]
    fn info_plist_executable_matches_the_bundle_name() {
        let p = info_plist("0.0.1");
        assert!(p.contains(&format!("<key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleExecutable</key><string>{APP}</string>")));
        assert!(p.contains(&format!("<key>CFBundleIdentifier</key><string>{BUNDLE_ID}</string>")));
        assert!(p.contains("<key>CFBundlePackageType</key><string>APPL</string>"));
    }

    #[test]
    fn info_plist_is_well_formed_enough_to_parse() {
        let p = info_plist("1.2.3");
        assert!(p.starts_with("<?xml"));
        assert!(p.contains("<plist version=\"1.0\">"));
        assert!(p.trim_end().ends_with("</plist>"));
        assert_eq!(p.matches("<dict>").count(), p.matches("</dict>").count());
        assert!(p.contains("<string>1.2.3</string>"));
    }

    /// Resources belong under Contents/Resources and nothing may land loose in
    /// the bundle root, which would stop it being a valid application.
    #[test]
    fn nothing_is_installed_at_the_bundle_root() {
        // Everything an application owns belongs under Contents/. A real macOS
        // install showed the manifest outside it, which this now prevents.
        for item in ["docs", "EULA.md", "SHA256SUMS", "runtime-src", "install", MANIFEST] {
            if let Some(d) = bundle_destination(item) {
                assert!(d.starts_with("Contents/"), "{item} -> {d} is outside Contents/");
            }
        }
    }

    #[test]
    fn bundle_layout_places_items_correctly() {
        assert_eq!(
            bundle_destination("runtime-src").as_deref(),
            Some("Contents/Resources/runtime-src")
        );
        assert_eq!(bundle_destination("docs").as_deref(), Some("Contents/Resources/docs"));
        assert_eq!(bundle_destination("EULA.md").as_deref(), Some("Contents/Resources/EULA.md"));
        assert_eq!(bundle_destination("install").as_deref(), Some("Contents/MacOS/install"));
        // `bin` is handled per-file so executables reach Contents/MacOS.
        assert_eq!(bundle_destination("bin"), None);

        for item in ["docs", "EULA.md", "SHA256SUMS", "runtime-src"] {
            let d = bundle_destination(item).unwrap();
            assert!(
                d.starts_with("Contents/"),
                "{item} would land outside Contents/, which is not a valid bundle"
            );
        }
    }
}
