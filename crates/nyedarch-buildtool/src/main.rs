//! nyedarch-buildtool — prototype CLI standing in for the GUI within the sandbox.
//!
//! Usage:
//!   nyedarch-buildtool seal <input-dir> <runtime-out-dir> <passphrase>
//!
//! It captures THIS machine's fingerprint (always-trusted creator, spec §12),
//! mints a factor secret, builds the sealed policy record, derives the build-
//! time payload key from (machine + passphrase), seals the package, and
//! generates a compilable runtime project.

mod bench;

use nyedarch_buildtool::{config, eula, remote};

use std::path::{Path, PathBuf};

use nyedarch_crypto::Argon2Params;


fn main() {
    use clap::Parser;
    use nyedarch_buildtool::cli::{Cli, Command, MachinesAction};

    // Parse before anything else, so `--help` and `--version` work without
    // touching the keystore, the licence record, or the fingerprint engine.
    // A user asking what the tool does should not have to accept a licence to
    // find out.
    let cli = Cli::parse();

    // Rebuilt for the sections still driven by positional parsing. Removing the
    // last of those is mechanical; doing it in one step would have meant a very
    // large untested change, so the parser is authoritative and the older code
    // is fed from it.
    let args: Vec<String> = match &cli.command {
        Command::Seal(a) => {
            let mut v = vec!["nyedarch".into(), "seal".into(),
                a.input.to_string_lossy().to_string(),
                a.output.to_string_lossy().to_string(),
                match a.passphrase.clone() {
                    Some(p) => p,
                    // Falling back to the environment keeps the passphrase out
                    // of the process list, which the help text promises.
                    None => match std::env::var("NYEDARCH_PASSPHRASE") {
                        Ok(p) if !p.is_empty() => p,
                        _ => {
                            eprintln!("error: no passphrase given.");
                            eprintln!("Pass one as an argument, or set NYEDARCH_PASSPHRASE.");
                            eprintln!("The passphrase is mandatory and cannot be disabled.");
                            std::process::exit(2);
                        }
                    },
                }];
            if let Some(m) = a.location { v.push("--location".into()); v.push(m.to_string()); }
            if !a.time.is_empty() { v.push("--time".into()); v.push(a.time.join(",")); }
            v.push("--time-tolerance-min".into()); v.push(a.time_tolerance_min.to_string());
            v.push("--tz-offset-min".into()); v.push(a.tz_offset_min.to_string());
            if a.one_shot { v.push("--one-shot".into()); }
            for t in &a.trust { v.push("--trust".into()); v.push(t.to_string_lossy().to_string()); }
            for t in &a.trust_tag { v.push("--trust-tag".into()); v.push(t.clone()); }
            v.push("--tag-mode".into()); v.push(a.tag_mode.clone());
            if !a.trust_search.is_empty() { v.push("--trust-search".into()); v.push(a.trust_search.clone()); }
            v.push("--compression".into()); v.push(a.compression.clone());
            v.push("--hardware".into()); v.push(a.hardware.clone());
            if cli.creator { v.push("--creator".into()); }
            v
        }
        Command::Build(a) => vec!["nyedarch".into(), "build".into(),
            a.project.to_string_lossy().to_string(), a.owner.clone(), a.repo.clone(), a.visibility.clone()],
        Command::Visibility(a) => vec!["nyedarch".into(), "visibility".into(),
            a.owner.clone(), a.repo.clone(), a.visibility.clone()],
        Command::Export(a) => {
            let mut v = vec!["nyedarch".into(), "export".into(), a.output.to_string_lossy().to_string()];
            v.extend(a.labels.clone());
            v
        }
        Command::Run(a) => {
            let mut v = vec!["nyedarch".into(), "run".into(), a.capsule.to_string_lossy().to_string()];
            if let Some(o) = &a.output { v.push(o.to_string_lossy().to_string()); }
            v
        }
        Command::Bench => vec!["nyedarch".into(), "bench".into()],
        Command::Machines { action } => {
            let mut v = vec!["nyedarch".into(), "machines".into()];
            match action {
                MachinesAction::List { query, tag, mode } => {
                    v.push("list".into());
                    if !query.is_empty() { v.push(query.clone()); }
                    for t in tag { v.push("--tag".into()); v.push(t.clone()); }
                    v.push("--mode".into()); v.push(mode.clone());
                }
                MachinesAction::Add { file } => {
                    v.push("add".into()); v.push(file.to_string_lossy().to_string());
                }
                MachinesAction::Remove { id } => { v.push("remove".into()); v.push(id.clone()); }
                MachinesAction::Label { id, labels } => {
                    v.push("label".into()); v.push(id.clone()); v.extend(labels.clone());
                }
            }
            v
        }
    };

    // Client anti-analysis (spec §9: anti-RE applies to the client too).
    // Advisory: it warns, it never denies a legitimate action.
    if let Some(w) = nyedarch_buildtool::harden::startup_check() {
        eprintln!("warning: {w}");
    }
    // Key storage quality is reported, never silently accepted (spec §44).
    if let Some(src) = nyedarch_buildtool::keystore::current_source() {
        if let Some(w) = src.warning() {
            eprintln!("warning: {w}");
        }
    }

    // EULA gating (spec §13): NYEDArch cannot be used until explicitly accepted.
    eula::require_acceptance();

    // `export` writes this machine's authenticated .nyfp record so another
    // operator can add it as a trusted machine (spec §10/§12).
    if args.len() >= 3 && args[1] == "export" {
        let fp = nyedarch_fingerprint::capture();
        let labels: Vec<String> = args.get(3..).map(|r| r.to_vec()).unwrap_or_default();
        let rec = nyedarch_fingerprint::nyfp::Record {
            version: 1,
            fingerprint: fp.clone(),
            labels: labels.clone(),
            created_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        };
        let bytes = nyedarch_fingerprint::nyfp::seal(&nyedarch_buildtool::pipeline::nyfp_key(), &rec).expect("seal nyfp");
        std::fs::write(&args[2], &bytes).expect("write nyfp");
        println!("[buildtool] exported fingerprint {} labels={:?} -> {}",
            &fp.id_hex()[..16], labels, args[2]);
        println!("[buildtool] record is MAC-authenticated; editing it invalidates it.");
        return;
    }

    // `run` launches a finished capsule as an independent process. This is the
    // command-line form of the client's drag-and-drop launcher; both call the
    // same `nyedarch_core::launch` code.
    if args.len() >= 2 && args[1] == "bench" {
        bench::run();
        return;
    }

    if args.len() >= 3 && args[1] == "run" {
        let path = std::path::PathBuf::from(&args[2]);
        let out = args.get(3).map(std::path::PathBuf::from);
        match nyedarch_core::launch::launch(&path, out.as_deref()) {
            Ok(pid) => {
                println!("[nyedarch] capsule launched as an independent process (pid {pid})");
                println!("[nyedarch] it performs its own authorization; this client grants it nothing.");
            }
            Err(e) => {
                eprintln!("error: {e}");
                eprintln!("hint: {}", nyedarch_core::launch::terminal_hint());
                std::process::exit(2);
            }
        }
        return;
    }

    // `build` runs the mandatory remote GitHub build for an already-sealed
    // project. `seal --remote ...` performs both in one step.
    if args.len() >= 5 && args[1] == "build" {
        let project = std::path::PathBuf::from(&args[2]);
        let owner = args[3].clone();
        let repo = args[4].clone();
        let private = args.get(5).map(|v| v != "public").unwrap_or(true);
        let targets = vec![
            nyedarch_github::Target::LinuxGnu,
            nyedarch_github::Target::WindowsMsvc,
            nyedarch_github::Target::MacosAppleSilicon,
        ];
        let ra = remote::RemoteBuildArgs {
            token: None,
            owner: &owner,
            repo: &repo,
            private,
            targets,
            project_dir: &project,
            build_id: nyedarch_core::Ulid::new().to_string(),
            package_commitment: file_digest(&project.join("capsule.nyeda")),
            runtime_commitment: [0u8; 32],
        };
        match remote::run_remote_build(&ra) {
            Ok(id) => println!("[nyedarch] remote build {id} complete"),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(4);
            }
        }
        return;
    }

    // `machines` manages the trusted machine registry (spec §12/§48).
    if args.len() >= 2 && args[1] == "machines" {
        use nyedarch_buildtool::machines::{self, TagMode};
        let sub = args.get(2).map(|s| s.as_str()).unwrap_or("list");
        match sub {
            "list" => {
                // Optional filters: machines list [query] [--tag T]... [--mode any|all]
                let mut query = String::new();
                let mut tags: Vec<String> = Vec::new();
                let mut mode = TagMode::Any;
                let mut i = 3;
                while i < args.len() {
                    match args[i].as_str() {
                        "--tag" => {
                            i += 1;
                            if let Some(t) = args.get(i) {
                                tags.push(t.clone());
                            }
                        }
                        "--mode" => {
                            i += 1;
                            mode = args.get(i).and_then(|m| TagMode::parse(m)).unwrap_or(TagMode::Any);
                        }
                        other => query = other.to_string(),
                    }
                    i += 1;
                }
                let sel = machines::select(&query, &tags, mode);
                println!();
                println!("  {:<18}  {:<10}  {}", "MACHINE", "", "LABELS");
                for m in &sel {
                    println!(
                        "  {:<18}  {:<10}  {}",
                        &m.id[..16.min(m.id.len())],
                        if m.is_this_machine { "this one" } else { "" },
                        m.labels.join(", ")
                    );
                }
                println!();
                println!("  Included machines: {}", sel.len());
                if !tags.is_empty() {
                    println!(
                        "  Filter: {} tag(s), {} semantics. This machine is always included.",
                        tags.len(),
                        if mode == TagMode::All { "ALL" } else { "ANY" }
                    );
                }
                let labels = machines::all_labels();
                if !labels.is_empty() {
                    println!("  Known labels: {}", labels.join(", "));
                }
            }
            "add" => {
                let Some(file) = args.get(3) else {
                    eprintln!("usage: nyedarch-buildtool machines add <file.nyfp>");
                    std::process::exit(2);
                };
                match machines::import(std::path::Path::new(file)) {
                    Ok(m) => println!("[nyedarch] trusted {} labels={:?}", &m.id[..16], m.labels),
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(2);
                    }
                }
            }
            "remove" => {
                let Some(id) = args.get(3) else {
                    eprintln!("usage: nyedarch-buildtool machines remove <id-prefix>");
                    std::process::exit(2);
                };
                match machines::remove(id) {
                    Ok(id) => println!("[nyedarch] removed {}", &id[..16]),
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(2);
                    }
                }
            }
            "label" => {
                if args.len() < 5 {
                    eprintln!("usage: nyedarch-buildtool machines label <id-prefix> <label>...");
                    std::process::exit(2);
                }
                let labels: Vec<String> = args[4..].to_vec();
                match machines::set_labels(&args[3], labels) {
                    Ok(m) => println!("[nyedarch] {} labels={:?}", &m.id[..16], m.labels),
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(2);
                    }
                }
            }
            other => {
                eprintln!("unknown machines subcommand '{other}'");
                eprintln!("  machines list [query] [--tag T]... [--mode any|all]");
                eprintln!("  machines add <file.nyfp>");
                eprintln!("  machines remove <id-prefix>");
                eprintln!("  machines label <id-prefix> <label>...");
                std::process::exit(2);
            }
        }
        return;
    }

    // `visibility` switches a build repository between public and private.
    if args.len() >= 5 && args[1] == "visibility" {
        let owner = args[2].clone();
        let repo = args[3].clone();
        let private = match args[4].as_str() {
            "private" => true,
            "public" => false,
            other => {
                eprintln!("error: visibility must be 'public' or 'private', not '{other}'");
                std::process::exit(2);
            }
        };
        match remote::set_repository_visibility(None, &owner, &repo, private) {
            Ok(()) => {
                println!(
                    "[nyedarch] {owner}/{repo} is now {}",
                    if private { "private" } else { "PUBLIC" }
                );
                if !private {
                    println!("[nyedarch] a public repository exposes your build logs and workflow to anyone.");
                    println!("[nyedarch] the capsule source stays encrypted either way, and the");
                    println!("[nyedarch] decryption key remains a repository secret.");
                }
            }
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(4);
            }
        }
        return;
    }

    if args.len() < 5 || args[1] != "seal" {
        // Unreachable for any input clap accepted; kept as a guard rather than
        // an `unreachable!()`, because aborting is not how this program fails.
        eprintln!("error: internal argument routing failed; run `nyedarch --help`");
        std::process::exit(2);
    }
    let protections = match config::parse(&args[5..]) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };
    let input = PathBuf::from(&args[2]);
    let out_project = PathBuf::from(&args[3]);
    let passphrase = args[4].as_bytes().to_vec();

    // Interactive Argon2id would be used in production; keep it light for the
    // demo build so it runs fast. Parameters are stored in the header.
    let argon = Argon2Params { m_cost: 64 * 1024, t_cost: 2, p_cost: 1 };

    // Trusted machines selected from the registry by label (spec §48), added to
    // any records named directly with --trust.
    let mut trust_files: Vec<String> = protections.trust_files.clone();
    if !protections.trust_tags.is_empty() || !protections.trust_query.is_empty() {
        use nyedarch_buildtool::machines::{self, TagMode};
        let mode = if protections.tag_mode_all { TagMode::All } else { TagMode::Any };
        let sel = machines::select(&protections.trust_query, &protections.trust_tags, mode);
        for m in &sel {
            if m.is_this_machine || m.path.as_os_str().is_empty() {
                continue; // always included; nothing to load
            }
            trust_files.push(m.path.to_string_lossy().to_string());
        }
        println!(
            "[buildtool] machine selection: {} tag(s), {} semantics -> {} machine(s) including this one",
            protections.trust_tags.len(),
            if protections.tag_mode_all { "ALL" } else { "ANY" },
            sel.len()
        );
    }

    // The seal itself runs through the shared pipeline, exactly as the desktop
    // client does. Keeping a second implementation here is what previously let
    // the two drift apart, so there is only one.
    let compression = protections.compression;
    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let req = nyedarch_buildtool::SealRequest {
        source: input.clone(),
        project_dir: out_project.clone(),
        passphrase: String::from_utf8_lossy(&passphrase).to_string(),
        location_tolerance_m: protections.location_tolerance_m,
        schedule: protections.schedule.clone(),
        one_shot: protections.one_shot,
        trust_files: trust_files.iter().map(std::path::PathBuf::from).collect(),
        argon,
        compression,
        cancelled,
        creator_mode: protections.creator_mode,
        hardware: protections.hardware,
    };

    let roots = nyedarch_buildtool::runtime_source_root();
    let outcome = match nyedarch_buildtool::seal_and_generate(&req, &roots, |stage| {
        println!("[buildtool] {}", stage.message());
    }) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(3);
        }
    };

    println!("[buildtool] creator fingerprint: {}", outcome.creator_fingerprint);
    println!("[buildtool] package sealed: {} bytes", outcome.package_bytes);
    println!("[buildtool] capsule project generated at {}", outcome.project_dir.display());
    println!("[buildtool] build nonce: {:016x}", outcome.build_nonce);
    // Security downgrades are printed in every mode, not just creator mode.
    for w in &outcome.warnings {
        eprintln!("warning: {w}");
    }

    if protections.creator_mode {
        println!();
        println!("  -- CREATOR MODE (diagnostic only) --------------");
        for d in &outcome.diagnostics {
            println!("   {d}");
        }
        println!("   Creator mode changes nothing about the capsule: it grants no");
        println!("   authority, skips no check, and weakens no protection.");
        println!("  ------------------------------------------------");
    }

    let trusted_count = outcome.trusted_machines;

    println!("  -- SECURITY REVIEW -----------------------------");
    println!("   MACHINE AUTHORIZATION   mandatory - {trusted_count} trusted fingerprint(s)");
    println!("   PASSPHRASE              mandatory - Argon2id m={}MiB t={} p={}",
        argon.m_cost / 1024, argon.t_cost, argon.p_cost);
    println!("   COMPRESSION             {}", compression.label());
    match protections.location_tolerance_m {
        Some(m) => println!("   GEOLOCATION             enabled  - tolerance {m} m"),
        None => println!("   GEOLOCATION             disabled"),
    }
    match &protections.schedule {
        Some(s) => println!("   TIME                    enabled  - {} slot(s) +/-{} min, tz {:+} min",
            s.slots_minutes.len(), s.tolerance_minutes, s.tz_offset_minutes),
        None => println!("   TIME                    disabled"),
    }
    println!("   EXECUTION               {}", if protections.one_shot { "one-shot" } else { "reusable" });
    println!("  ------------------------------------------------");
}


fn file_digest(p: &Path) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"nyedarch:v1:package-commitment");
    if let Ok(b) = std::fs::read(p) {
        h.update(&b);
    }
    h.finalize().into()
}

