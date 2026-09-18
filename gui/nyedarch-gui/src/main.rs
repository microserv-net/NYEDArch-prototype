//! NYEDArch desktop client.
//!
//! A stepped workflow -- Source, Protections, Machines, Targets, Build, Run --
//! with a persistent navigation rail, a menu bar for the actions that do not
//! belong in the flow, and a posture meter that reacts as protections change.
//!
//! Behaviour is unchanged from the command-line client it mirrors: machine and
//! passphrase protections are mandatory and cannot be switched off, the creator
//! fingerprint is always captured and included, and a dropped capsule is
//! launched through `nyedarch_core::launch` as an independent process that
//! performs its own authorization.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod theme;
mod widgets;

use eframe::egui;
use egui::{pos2, vec2, Align2, Color32, Rect, Rounding, Sense, Stroke};

use std::sync::mpsc::{channel, Receiver, Sender};

use nyedarch_buildtool::{seal_and_generate, runtime_source_root, SealRequest, Stage};

use theme as t;
use widgets as w;


/// Parse "HH:MM" into minutes past midnight.
fn parse_hhmm(s: &str) -> Option<u32> {
    let (h, m) = s.trim().split_once(':')?;
    let h: u32 = h.trim().parse().ok()?;
    let m: u32 = m.trim().parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some(h * 60 + m)
}

// ------------------------------------------------------------------ state ---

/// Progress from the sealing worker. Every variant corresponds to real work.
enum BuildMsg {
    Stage(Stage),
    Note(String),
    Done {
        project: std::path::PathBuf,
        bytes: usize,
        machines: usize,
        nonce: u64,
    },
    Failed(String),
}


#[derive(PartialEq, Clone, Copy, Debug)]
enum Step {
    Source,
    Protections,
    Machines,
    Targets,
    Build,
    Run,
}

impl Step {
    const ALL: [Step; 6] = [
        Step::Source,
        Step::Protections,
        Step::Machines,
        Step::Targets,
        Step::Build,
        Step::Run,
    ];
    fn label(self) -> &'static str {
        match self {
            Step::Source => "Source",
            Step::Protections => "Protections",
            Step::Machines => "Machines",
            Step::Targets => "Targets",
            Step::Build => "Build",
            Step::Run => "Run capsule",
        }
    }
    fn hint(self) -> &'static str {
        match self {
            Step::Source => "What to protect",
            Step::Protections => "How it unlocks",
            Step::Machines => "Where it opens",
            Step::Targets => "Which platforms",
            Step::Build => "Create the capsule",
            Step::Run => "Open a capsule",
        }
    }
    fn index(self) -> usize {
        Step::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

/// Which modal panel, if any, is open.
#[derive(PartialEq, Clone, Copy)]
enum Modal {
    None,
    Eula,
    About,
    Shortcuts,
}

struct Protections {
    machine: bool,
    passphrase: bool,
    location: bool,
    location_tolerance_m: u32,
    time: bool,
    time_of_day: String,
    time_tolerance_min: u32,
    one_shot: bool,
}

impl Default for Protections {
    fn default() -> Self {
        // Machine and passphrase are always on. They exist as fields for
        // rendering only; nothing in the interface can clear them.
        Self {
            machine: true,
            passphrase: true,
            location: false,
            location_tolerance_m: 60,
            time: false,
            time_of_day: "14:00".to_string(),
            time_tolerance_min: 15,
            one_shot: false,
        }
    }
}


struct App {
    step: Step,
    step_changed_at: f64,
    modal: Modal,

    source_path: String,
    passphrase: String,
    protections: Protections,

    /// The build repository name. Locked until the user chooses to change it,
    /// because a wrong name here silently creates a second repository.
    gh_repo_locked: bool,
    /// Locked once a token has been stored, so it is not retyped every build.
    gh_token_locked: bool,
    /// Resolved from the token, not typed. None until first resolved.
    gh_owner_resolved: Option<String>,
    /// GitHub credentials. The token is held in memory for this session only
    /// and is never written to disk by NYEDArch.
    gh_owner: String,
    gh_repo: String,
    gh_token: String,
    /// Whether to dispatch the remote build after sealing.
    target_windows: bool,
    target_macos: bool,
    target_linux: bool,

    log: Vec<(f64, String)>,
    building: bool,
    build_progress: f32,
    /// Where the sealed capsule project was written, once it exists.
    built_project: Option<std::path::PathBuf>,
    /// Progress and completion from the worker thread. Sealing runs off the UI
    /// thread because Argon2id is deliberately slow and would freeze the window.
    build_rx: Option<Receiver<BuildMsg>>,

    dropped_capsule: Option<std::path::PathBuf>,
    run_out_dir: String,
    run_status: String,
    run_ok: bool,

    /// Machine selector state (spec §48): free-text search, selected tags, and
    /// whether tags combine with ANY or ALL.
    /// Compression effort (spec §16) and creator mode (spec §50).
    compression: nyedarch_package::pipeline::CompressionMode,
    creator_mode: bool,
    /// Verbose, timestamped diagnostics in the activity log.
    with_logs: bool,
    /// Set to cancel an in-flight build (spec §61).
    cancel_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// Creator-mode diagnostics from the last build.
    pending_diagnostics: Vec<String>,
    /// Accumulated laps for the perimeter pulse.
    ///
    /// Integrated rather than derived from the clock, so changing speed changes
    /// the *rate* and never teleports the comet to a new position.
    pulse_phase: f64,
    pulse_last: f64,
    machine_query: String,
    selected_tags: Vec<String>,
    tag_mode_all: bool,
    toast: Option<(String, f64, Color32)>,
    /// The licence must be accepted before the application can be used
    /// (spec §13). This gates the whole window, not just a menu item.
    eula_accepted: bool,
}

impl Default for App {
    fn default() -> Self {
        // The creator machine is always trusted and always included.
        // Capturing here confirms the fingerprint engine works before the user
        // reaches the Machines step; the registry reports the live value.
        let _ = nyedarch_fingerprint::capture();

        // A token saved on a previous run. Stored encrypted under a key derived
        // from the client master key, beside the machine records.
        let stored_token = nyedarch_buildtool::keystore::load_secret("github-token")
            .map(|t| String::from_utf8_lossy(&t).to_string())
            .unwrap_or_default();
        let token_locked = !stored_token.is_empty();
        Self {
            step: Step::Source,
            step_changed_at: 0.0,
            modal: Modal::None,
            source_path: String::new(),
            passphrase: String::new(),
            protections: Protections::default(),
            gh_repo_locked: true,
            // Restored from the encrypted store if one was saved earlier.
            gh_token_locked: token_locked,
            gh_owner_resolved: None,
            // Pre-fill from the environment if present, exactly as the CLI does.
            gh_owner: String::new(),
            gh_repo: "nyedarch-builds".to_string(),
            gh_token: std::env::var("NYEDARCH_GITHUB_TOKEN")
                .or_else(|_| std::env::var("GITHUB_TOKEN"))
                .unwrap_or_default(),
            // Default to the platform this client is running on: the capsule a
            // user most likely wants first is one that runs where they are.
            target_windows: cfg!(target_os = "windows"),
            target_macos: cfg!(target_os = "macos"),
            target_linux: cfg!(target_os = "linux"),
            log: {
                let mut l = vec![(0.0, "Ready. Choose what to protect.".to_string())];
                // Same client anti-analysis check the CLI runs.
                if let Some(w) = nyedarch_buildtool::harden::startup_check() {
                    l.push((0.0, w.to_string()));
                }
                if let Some(w) = nyedarch_buildtool::keystore::current_source()
                    .and_then(|s| s.warning())
                {
                    l.push((0.0, w.to_string()));
                }
                l
            },
            building: false,
            build_progress: 0.0,
            built_project: None,
            build_rx: None,
            dropped_capsule: None,
            run_out_dir: String::new(),
            run_status: String::new(),
            run_ok: false,
            compression: nyedarch_package::pipeline::CompressionMode::Automatic,
            creator_mode: false,
            with_logs: false,
            cancel_flag: None,
            pending_diagnostics: Vec::new(),
            pulse_phase: 0.0,
            pulse_last: 0.0,
            machine_query: String::new(),
            selected_tags: Vec::new(),
            tag_mode_all: false,
            toast: None,
            eula_accepted: nyedarch_buildtool::eula::already_accepted(),
        }
    }
}

impl App {
    fn active_protections(&self) -> usize {
        2 + self.protections.location as usize + self.protections.time as usize
    }

    /// Whether the passphrase is strong enough to build with.
    ///
    /// Refused rather than warned about. The passphrase is the factor an
    /// attacker actually attacks offline: Argon2id makes each guess expensive,
    /// but it cannot rescue a phrase that appears in a wordlist.
    fn passphrase_acceptable(&self) -> bool {
        w::passphrase_strength(&self.passphrase).0 >= 0.4
    }

    fn step_done(&self, s: Step) -> bool {
        match s {
            Step::Source => !self.source_path.trim().is_empty(),
            Step::Protections => self.passphrase_acceptable(),
            Step::Machines => true, // the creator machine is always trusted
            Step::Targets => {
                // A target with no token is not a step you can proceed from:
                // the build is remote, so it cannot start without credentials.
                (self.target_windows || self.target_macos || self.target_linux)
                    && !self.gh_token.trim().is_empty()
            }
            Step::Build => self.build_progress >= 1.0,
            Step::Run => self.run_ok,
        }
    }

    /// Repository visibility follows creator mode, and nothing else.
    ///
    /// There used to be a separate "private repository" switch beside creator
    /// mode. Two controls for one decision is one too many, and they could
    /// disagree - a capsule built in "creator mode" on a private repository
    /// produced diagnostics nobody could see, and the reverse quietly published
    /// build logs. Creator mode now means exactly one thing: build in the open.
    fn repo_should_be_private(&self) -> bool {
        !self.creator_mode
    }

    /// The account the token belongs to, resolved once and remembered.
    fn resolved_owner(&mut self) -> Option<String> {
        if self.gh_token.trim().is_empty() {
            return None;
        }
        if self.gh_owner_resolved.is_none() {
            self.gh_owner_resolved =
                nyedarch_buildtool::remote::resolve_owner(Some(self.gh_token.trim().to_string()))
                    .ok();
        }
        self.gh_owner_resolved.clone()
    }

    fn selected_targets(&self) -> Vec<nyedarch_github::Target> {
        let mut v = Vec::new();
        if self.target_linux {
            v.push(nyedarch_github::Target::LinuxGnu);
        }
        if self.target_windows {
            v.push(nyedarch_github::Target::WindowsMsvc);
        }
        if self.target_macos {
            v.push(nyedarch_github::Target::MacosAppleSilicon);
        }
        v
    }

    fn ready_to_build(&self) -> bool {
        self.step_done(Step::Source) && self.step_done(Step::Protections) && self.step_done(Step::Targets)
    }

    fn goto(&mut self, s: Step, now: f64) {
        if s != self.step {
            self.step = s;
            self.step_changed_at = now;
        }
    }

    /// Append an activity line.
    ///
    /// Timestamped in IST when verbose logging is on, so a line here lines up
    /// with the same run's command-line output. Without it the two logs
    /// describe one build in two vocabularies and neither can be checked
    /// against the other.
    fn say(&mut self, now: f64, msg: impl Into<String>) {
        let text = msg.into();
        let text = if self.with_logs {
            format!("{}  {text}", nyedarch_buildtool::logging::timestamp())
        } else {
            text
        };
        self.log.push((now, text));
        if self.log.len() > 200 {
            self.log.remove(0);
        }
    }

    fn toast(&mut self, now: f64, msg: impl Into<String>, colour: Color32) {
        self.toast = Some((msg.into(), now, colour));
    }

    // ---------------------------------------------------------- actions ----

    /// Open a native folder chooser. This is a real dialog, not a placeholder:
    /// without it the source field could only be typed into, which was the
    /// defect this replaces.
    fn pick_source_folder(&mut self, now: f64) {
        let mut dialog = rfd::FileDialog::new().set_title("Choose a folder to protect");
        if let Some(cur) = std::path::Path::new(self.source_path.trim()).parent() {
            if cur.is_dir() {
                dialog = dialog.set_directory(cur);
            }
        }
        match dialog.pick_folder() {
            Some(p) => {
                self.say(now, format!("Source set to {}", p.display()));
                self.source_path = p.to_string_lossy().to_string();
                self.toast(now, "Source selected", t::SKY);
            }
            None => self.say(now, "Folder selection cancelled."),
        }
    }

    fn pick_source_file(&mut self, now: f64) {
        match rfd::FileDialog::new()
            .set_title("Choose a file to protect")
            .pick_file()
        {
            Some(p) => {
                self.say(now, format!("Source set to {}", p.display()));
                self.source_path = p.to_string_lossy().to_string();
                self.toast(now, "Source selected", t::SKY);
            }
            None => self.say(now, "File selection cancelled."),
        }
    }

    fn pick_output_dir(&mut self, now: f64) {
        if let Some(p) = rfd::FileDialog::new()
            .set_title("Choose where to extract")
            .pick_folder()
        {
            self.run_out_dir = p.to_string_lossy().to_string();
            self.say(now, format!("Extract target set to {}", p.display()));
        }
    }

    fn pick_capsule(&mut self, now: f64) {
        match rfd::FileDialog::new()
            .set_title("Open a NYEDArch capsule")
            .add_filter("NYEDArch capsule", &["nyarch"])
            .pick_file()
        {
            Some(p) => {
                self.goto(Step::Run, now);
                match nyedarch_core::launch::validate(&p) {
                    Ok(()) => {
                        self.run_status.clear();
                        self.run_ok = false;
                        self.say(now, format!("Loaded {}", p.display()));
                        self.toast(now, "Capsule ready", t::SKY);
                        self.dropped_capsule = Some(p);
                    }
                    Err(e) => {
                        self.dropped_capsule = None;
                        self.run_status = format!("Cannot run that file: {e}");
                        self.toast(now, "Not a capsule", t::ROSE);
                    }
                }
            }
            None => {}
        }
    }

    fn import_machine(&mut self, now: f64) {
        if let Some(p) = rfd::FileDialog::new()
            .set_title("Import a trusted machine record")
            .add_filter("NYEDArch fingerprint", &["nyfp"])
            .pick_file()
        {
            // Verification happens inside the registry, so both clients apply
            // exactly the same check. A record that fails is refused outright.
            match nyedarch_buildtool::machines::import(&p) {
                Ok(m) => {
                    self.say(
                        now,
                        format!("Imported machine {} labels={:?}", &m.id[..16.min(m.id.len())], m.labels),
                    );
                    self.toast(now, "Machine verified and added", t::SKY);
                    self.goto(Step::Machines, now);
                }
                Err(e) => {
                    for line in e.lines() {
                        self.say(now, line.to_string());
                    }
                    self.toast(now, "Record refused", t::ROSE);
                }
            }
        }
    }

    fn export_machine(&mut self, now: f64) {
        if let Some(p) = rfd::FileDialog::new()
            .set_title("Export this machine")
            .set_file_name("this-machine.nyfp")
            .add_filter("NYEDArch fingerprint", &["nyfp"])
            .save_file()
        {
            match nyedarch_buildtool::machines::export_this_machine(
                &p,
                vec!["exported".to_string()],
            ) {
                Ok(id) => {
                    self.say(now, format!("Exported {} to {}", &id[..16.min(id.len())], p.display()));
                    self.say(now, "The record is authenticated; editing it invalidates it.");
                    self.toast(now, "Machine exported", t::SKY);
                }
                Err(e) => {
                    self.say(now, format!("Export failed: {e}"));
                    self.toast(now, "Export failed", t::ROSE);
                }
            }
        }
    }

    /// Apply the chosen visibility to the build repository now, without waiting
    /// for the next build. Mirrors `nyedarch visibility` in the CLI.
    fn apply_visibility(&mut self, now: f64) {
        if self.gh_owner.trim().is_empty() || self.gh_repo.trim().is_empty() {
            self.say(now, "Set the GitHub owner and repository first.");
            self.toast(now, "GitHub details missing", t::ROSE);
            return;
        }
        let token = if self.gh_token.trim().is_empty() { None } else { Some(self.gh_token.trim().to_string()) };
        match nyedarch_buildtool::remote::set_repository_visibility(
            token,
            self.gh_owner.trim(),
            self.gh_repo.trim(),
            self.repo_should_be_private(),
        ) {
            Ok(()) => {
                let word = if self.repo_should_be_private() { "private" } else { "PUBLIC" };
                self.say(now, format!("{}/{} is now {word}.", self.gh_owner.trim(), self.gh_repo.trim()));
                if !self.repo_should_be_private() {
                    self.say(now, "A public repository exposes your build logs and workflow to anyone.");
                    self.say(now, "The capsule source stays encrypted either way; the key remains a repository secret.");
                }
                self.toast(now, format!("Repository is {word}"), if self.repo_should_be_private() { t::SKY } else { t::ROSE });
            }
            Err(e) => {
                for line in e.lines() {
                    self.say(now, line.to_string());
                }
                self.toast(now, "Visibility unchanged", t::ROSE);
            }
        }
    }

    /// Start a real seal on a worker thread.
    ///
    /// Nothing here is simulated: this is the same `seal_and_generate` the
    /// command-line client runs, and the progress bar advances only when the
    /// pipeline reports a stage.
    fn start_build(&mut self, now: f64) {
        if self.building {
            return;
        }
        let source = std::path::PathBuf::from(self.source_path.trim());
        if !source.exists() {
            self.say(now, format!("Source does not exist: {}", source.display()));
            self.toast(now, "Source not found", t::ROSE);
            return;
        }

        // Ask where the *capsule* goes, not where to put a project.
        //
        // The generated project is working material: it holds the runtime
        // source, the vendored crates and the sealed package. Handing that to
        // the user gave them six confusing items instead of one executable, and
        // exposed a template the design deliberately does not ship (spec §22).
        // It is now created under a temporary directory and removed afterwards.
        let stem = source
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "nyedarch".into());
        let capsule_path = match rfd::FileDialog::new()
            .set_title("Where should the capsule be saved?")
            .set_file_name(&format!("{stem}.nyarch"))
            .add_filter("NYEDArch capsule", &["nyarch"])
            .save_file()
        {
            Some(p) => p,
            None => {
                self.say(now, "Build cancelled: no destination chosen.");
                return;
            }
        };

        let project_dir = std::env::temp_dir().join(format!(
            "nyedarch-build-{}-{}",
            std::process::id(),
            now as u64
        ));

        let schedule = if self.protections.time {
            match parse_hhmm(&self.protections.time_of_day) {
                Some(mins) => Some(nyedarch_crypto::timewin::DailySchedule {
                    slots_minutes: vec![mins],
                    tolerance_minutes: self.protections.time_tolerance_min,
                    tz_offset_minutes: 0,
                }),
                None => {
                    self.say(now, "Time must be HH:MM.");
                    self.toast(now, "Invalid time", t::ROSE);
                    return;
                }
            }
        } else {
            None
        };

        // Machines selected in the Machines step, resolved through the shared
        // registry so the GUI and CLI trust exactly the same set.
        let mode = if self.tag_mode_all {
            nyedarch_buildtool::machines::TagMode::All
        } else {
            nyedarch_buildtool::machines::TagMode::Any
        };
        let trust_files: Vec<std::path::PathBuf> =
            nyedarch_buildtool::machines::select(&self.machine_query, &self.selected_tags, mode)
                .into_iter()
                .filter(|m| !m.is_this_machine && !m.path.as_os_str().is_empty())
                .map(|m| m.path)
                .collect();

        // Cancellation flag, shared with the worker (spec §61).
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let req = SealRequest {
            source,
            project_dir: project_dir.clone(),
            passphrase: self.passphrase.clone(),
            location_tolerance_m: if self.protections.location {
                Some(self.protections.location_tolerance_m)
            } else {
                None
            },
            schedule,
            one_shot: self.protections.one_shot,
            trust_files,
            argon: nyedarch_crypto::Argon2Params { m_cost: 64 * 1024, t_cost: 2, p_cost: 1 },
            compression: self.compression,
            cancelled: cancel.clone(),
            creator_mode: self.creator_mode,
            // The desktop client prefers hardware and reports any downgrade;
            // `--hardware required` is available on the command line for builds
            // that must not proceed without it.
            hardware: nyedarch_platform::hardware::HardwarePolicy::Preferred,
        };

        // The remote build is part of the pipeline, not an option.
        //
        // It used to be switchable, which left a state where the client had
        // generated a capsule project and simply stopped - the user was holding
        // unbuilt source and no capsule. Missing credentials are now a refusal
        // with an explanation, not a silent change of behaviour.
        let owner = match self.resolved_owner() {
            Some(o) => o,
            None => {
                self.say(now, "A GitHub token is required: capsules are built remotely.");
                self.say(now, "Add one under Targets, or set NYEDARCH_GITHUB_TOKEN.");
                self.toast(now, "GitHub token required", t::ROSE);
                return;
            }
        };
        let remote = Some((
            owner,
            self.gh_repo.trim().to_string(),
            self.gh_token.trim().to_string(),
            self.repo_should_be_private(),
            self.selected_targets(),
            capsule_path.clone(),
        ));

        // Checked by the worker to decide whether the working files can go.
        let deliver_check = capsule_path.clone();

        let (tx, rx): (Sender<BuildMsg>, Receiver<BuildMsg>) = channel();
        self.build_rx = Some(rx);
        self.cancel_flag = Some(cancel.clone());
        self.building = true;
        self.build_progress = 0.0;
        self.built_project = None;
        self.say(now, "Starting seal.");

        std::thread::spawn(move || {
            let roots = runtime_source_root();
            let tx2 = tx.clone();
            let result = seal_and_generate(&req, &roots, move |stage| {
                let _ = tx2.send(BuildMsg::Stage(stage));
            });
            match result {
                Ok(o) => {
                    if let Some((owner, repo, token, private, targets, deliver)) = remote {
                        let _ = tx.send(BuildMsg::Note(
                            "Dispatching remote build on GitHub Actions.".to_string(),
                        ));
                        let ra = nyedarch_buildtool::remote::RemoteBuildArgs {
                            token: Some(token),
                            owner: &owner,
                            repo: &repo,
                            private,
                            targets,
                            project_dir: &o.project_dir,
                            deliver_to: Some(deliver.clone()),
                            build_id: nyedarch_core::Ulid::new().to_string(),
                            package_commitment: o.package_commitment,
                            runtime_commitment: o.runtime_commitment,
                        };
                        let tx3 = tx.clone();
                        match nyedarch_buildtool::remote::run_remote_build_with(&ra, move |m| {
                            let _ = tx3.send(BuildMsg::Note(m));
                        }) {
                            Ok(run) => {
                                let _ = tx.send(BuildMsg::Note(format!(
                                    "Remote build dispatched, workflow run {run}."
                                )));
                            }
                            Err(e) => {
                                let _ = tx.send(BuildMsg::Note(format!("Remote build failed: {e}")));
                            }
                        }
                    }
                    for w in &o.warnings {
                        let _ = tx.send(BuildMsg::Note(format!("warning: {w}")));
                    }
                    for d in &o.diagnostics {
                        let _ = tx.send(BuildMsg::Note(format!("  {d}")));
                    }
                    // Only discard the working files if the capsule actually
                    // arrived.
                    //
                    // The previous version deleted them unconditionally, right
                    // after logging "the project was still written; you can
                    // build it locally" - so when the remote build failed or
                    // timed out, the user was left with nothing at all and a
                    // message pointing at a directory that had just been
                    // removed. Working material is only worth deleting once
                    // there is something better to keep.
                    let delivered = deliver_check.exists();
                    if delivered {
                        let _ = std::fs::remove_dir_all(&o.project_dir);
                        let _ = tx.send(BuildMsg::Note(format!(
                            "Capsule saved to {}",
                            deliver_check.display()
                        )));
                        let _ = tx.send(BuildMsg::Note("Working files removed.".to_string()));
                    } else {
                        // No local fallback.
                        //
                        // Capsules are built remotely, and that is a security
                        // property rather than a convenience: the build
                        // environment is fixed, the artifact is checked against
                        // a commitment made beforehand, and the toolchain is
                        // not whatever happens to be on this machine. Quietly
                        // compiling here when the remote build failed would
                        // produce a capsule none of that applied to.
                        let _ = tx.send(BuildMsg::Note(
                            "No capsule was produced: the remote build did not return one."
                                .to_string(),
                        ));
                        let _ = tx.send(BuildMsg::Note(
                            "Nothing has been built locally - capsules are always built remotely."
                                .to_string(),
                        ));
                        let _ = tx.send(BuildMsg::Note(
                            "Check the workflow run on GitHub, then build again.".to_string(),
                        ));
                        // Working files are still removed: they are the runtime
                        // source, and they are not a deliverable.
                        let _ = std::fs::remove_dir_all(&o.project_dir);
                    }
                    let _ = tx.send(BuildMsg::Done {
                        project: o.project_dir,
                        bytes: o.package_bytes,
                        machines: o.trusted_machines,
                        nonce: o.build_nonce,
                    });
                }
                Err(e) => {
                    let _ = tx.send(BuildMsg::Failed(e.to_string()));
                }
            }
        });
    }

    /// Drain worker messages each frame.
    fn poll_build(&mut self, now: f64) {
        let mut msgs = Vec::new();
        if let Some(rx) = &self.build_rx {
            while let Ok(m) = rx.try_recv() {
                msgs.push(m);
            }
        }
        for m in msgs {
            match m {
                BuildMsg::Stage(s) => {
                    self.build_progress = s.progress();
                    self.say(now, s.message());
                }
                BuildMsg::Note(m) => self.say(now, m),
                BuildMsg::Done { project, bytes, machines, nonce } => {
                    let _ = &project;
                    self.building = false;
                    self.cancel_flag = None;
                    self.build_progress = 1.0;
                    self.built_project = Some(project.clone());
                    self.build_rx = None;
                    self.say(now, format!("Sealed {bytes} bytes for {machines} machine(s), build {nonce:016x}."));

                    for d in std::mem::take(&mut self.pending_diagnostics) {
                        self.say(now, format!("  {d}"));
                    }
                    self.toast(now, "Capsule project created", t::SKY);
                }
                BuildMsg::Failed(e) => {
                    self.building = false;
                    self.cancel_flag = None;
                    self.build_progress = 0.0;
                    self.build_rx = None;
                    for line in e.lines() {
                        self.say(now, line.to_string());
                    }
                    self.toast(now, "Build failed", t::ROSE);
                }
            }
        }
    }

    fn reset(&mut self, now: f64) {
        // The trusted machine registry is on disk and survives a reset; only
        // the in-progress capsule is cleared.
        *self = App {
            step_changed_at: now,
            ..App::default()
        };
        self.say(now, "New capsule started.");
    }
}

// -------------------------------------------------------------- menu bar ----

impl App {
    fn menu_bar(&mut self, ctx: &egui::Context, now: f64) {
        egui::TopBottomPanel::top("menubar")
            .exact_height(30.0)
            .frame(
                egui::Frame::none()
                    .fill(t::RAIL)
                    .inner_margin(egui::Margin::symmetric(6.0, 2.0)),
            )
            .show(ctx, |ui| {
                // No rule under the bar.
                //
                // A hairline there cuts the window in two and leaves a visible
                // seam between the toolbar and the body. The bar shares the
                // rail's surface colour instead, so the top of the window reads
                // as one continuous plane and the content below floats on it.

                egui::menu::bar(ui, |ui| {
                    // Capsule: the object this application exists to make.
                    ui.menu_button("Capsule", |ui| {
                        if ui.button("New capsule").clicked() {
                            self.reset(now);
                            ui.close_menu();
                        }
                        if ui.button("Choose folder to protect...").clicked() {
                            self.pick_source_folder(now);
                            ui.close_menu();
                        }
                        if ui.button("Choose file to protect...").clicked() {
                            self.pick_source_file(now);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Open capsule to run...").clicked() {
                            self.pick_capsule(now);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Quit").clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });

                    // Machines: import and export live here rather than being
                    // buried in a step, since they are used between sessions.
                    ui.menu_button("Machines", |ui| {
                        if ui.button("Import trusted machine (.nyfp)...").clicked() {
                            self.import_machine(now);
                            ui.close_menu();
                        }
                        if ui.button("Export this machine (.nyfp)...").clicked() {
                            self.export_machine(now);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Recapture this machine").clicked() {
                            // The registry always reports the live fingerprint,
                            // so this simply reports what it now is.
                            let fp = nyedarch_fingerprint::capture();
                            self.say(now, format!("This machine is {}", &fp.id_hex()[..16]));
                            self.toast(now, "Fingerprint recaptured", t::SKY);
                            ui.close_menu();
                        }
                    });

                    // Protections: quick toggles mirroring the step, so they can
                    // be reached without navigating.
                    ui.menu_button("Protections", |ui| {
                        // These are not actions, so they are not buttons.
                        //
                        // A disabled button paints nothing under this theme and
                        // read as an empty gap at the top of the menu. They are
                        // statements of fact, and now look like it.
                        ui.label(
                            egui::RichText::new("Machine · always on")
                                .size(t::SMALL)
                                .color(t::SKY_DEEP),
                        );
                        ui.label(
                            egui::RichText::new("Passphrase · always on")
                                .size(t::SMALL)
                                .color(t::SKY_DEEP),
                        );
                        ui.separator();
                        if ui
                            .checkbox(&mut self.protections.location, "Location")
                            .clicked()
                        {
                            self.step_changed_at = now;
                        }
                        ui.checkbox(&mut self.protections.time, "Time window");
                        ui.separator();
                        if ui
                            .checkbox(&mut self.protections.one_shot, "One-shot capsule")
                            .clicked()
                            && self.protections.one_shot
                        {
                            self.toast(now, "Deletion cannot be guaranteed on SSDs", t::ROSE);
                        }
                    });

                    // Build: the action, plus where it happens.
                    ui.menu_button("Build", |ui| {
                        // Status, not an action - a disabled button paints
                        // nothing under this theme and reads as an empty gap.
                        ui.label(
                            egui::RichText::new("Capsules are always built remotely")
                                .size(t::SMALL)
                                .color(t::SKY_DEEP),
                        );
                        ui.separator();
                        let ready = self.ready_to_build() && !self.building;
                        if ui
                            .add_enabled(ready, egui::Button::new("Build capsule"))
                            .clicked()
                        {
                            self.goto(Step::Build, now);
                            self.start_build(now);
                            ui.close_menu();
                        }
                        ui.separator();
                        ui.checkbox(&mut self.target_linux, "Target: Linux");
                        ui.checkbox(&mut self.target_windows, "Target: Windows");
                        ui.checkbox(&mut self.target_macos, "Target: macOS");
                        ui.separator();
                        ui.checkbox(&mut self.repo_should_be_private(), "Private repository");
                    });

                    // Help: licence and reference material.
                    ui.menu_button("Help", |ui| {
                        if ui.button("End User Licence Agreement").clicked() {
                            self.modal = Modal::Eula;
                            ui.close_menu();
                        }
                        if ui.button("Keyboard shortcuts").clicked() {
                            self.modal = Modal::Shortcuts;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("About NYEDArch").clicked() {
                            self.modal = Modal::About;
                            ui.close_menu();
                        }
                    });

                    // Right-aligned live status.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (label, colour) = if self.building {
                            ("building", t::SKY)
                        } else if self.build_progress >= 1.0 {
                            ("sealed", t::SKY)
                        } else {
                            ("idle", t::INK_MUTED)
                        };
                        let p = ui.cursor().min + vec2(-72.0, 4.0);
                        w::pill(ui.painter(), p, label, colour, t::alpha(colour, 0.12));
                        ui.add_space(78.0);
                    });
                });
            });
    }
}

// ------------------------------------------------------------------- rail ---

impl App {
    fn rail(&mut self, ctx: &egui::Context, now: f64) {
        egui::SidePanel::left("rail")
            .exact_width(236.0)
            .resizable(false)
            .frame(egui::Frame::none().fill(t::RAIL).inner_margin(egui::Margin {
                left: 12.0,
                right: 12.0,
                top: 16.0,
                bottom: 12.0,
            }))
            .show(ctx, |ui| {
                // Wordmark with the aperture.
                let (logo_rect, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 56.0), Sense::hover());
                let open = self.active_protections() as f32 / 4.0;
                w::aperture(ui.painter(),
                    pos2(logo_rect.left() + 23.0, logo_rect.center().y),
                    44.0,
                    now,
                    1.0 - open * 0.8,
                    self.building,
                );
                ui.painter().text(
                    pos2(logo_rect.left() + 52.0, logo_rect.center().y - 9.0),
                    Align2::LEFT_CENTER,
                    "NYEDArch",
                    t::font(19.0),
                    t::INK,
                );
                ui.painter().text(
                    pos2(logo_rect.left() + 52.0, logo_rect.center().y + 10.0),
                    Align2::LEFT_CENTER,
                    "Not Your Everyday Archive",
                    t::font(t::MICRO),
                    t::INK_MUTED,
                );

                ui.add_space(12.0);
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                ui.painter().rect_filled(r, Rounding::ZERO, t::LINE);
                ui.add_space(10.0);

                // Sliding active indicator, painted before the items.
                let first_y = ui.cursor().top();
                let item_h = 50.0 + ui.spacing().item_spacing.y;
                let target_y = first_y + self.step.index() as f32 * item_h;
                let y = ui
                    .ctx()
                    .animate_value_with_time(egui::Id::new("rail_ind"), target_y, 0.22);
                let ind = Rect::from_min_size(pos2(ui.min_rect().left() - 8.0, y + 9.0), vec2(3.0, 32.0));
                ui.painter().rect_filled(ind, Rounding::same(2.0), t::SKY);
                w::glow(ui.painter(), ind.center(), 20.0, t::SKY, 0.4);

                let mut clicked = None;
                for (i, s) in Step::ALL.iter().enumerate() {
                    let done = self.step_done(*s) && *s != self.step;
                    if w::nav_item(ui, i, s.label(), s.hint(), *s == self.step, done).clicked() {
                        clicked = Some(*s);
                    }
                }
                if let Some(s) = clicked {
                    self.goto(s, now);
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    ui.add_space(2.0);
                    w::vault_core(
                        ui,
                        208.0,
                        [
                            true, // machine: mandatory, always engaged
                            self.passphrase_acceptable(),
                            self.protections.location,
                            self.protections.time,
                        ],
                        self.build_progress,
                        self.building,
                        now,
                    );
                });
            });
    }
}

// ------------------------------------------------------------------ views ---

impl App {
    fn view_source(&mut self, ui: &mut egui::Ui, now: f64) {
        w::section_title(
            ui,
            "What are you protecting?",
            "A capsule carries your data, the rules for opening it, and its own extraction logic.",
        );

        let mut want_folder = false;
        let mut want_file = false;
        w::card(
            ui,
            "src",
            126.0,
            t::SKY,
            !self.source_path.is_empty(),
            false,
            |ui, _r, _l| {
                ui.label(egui::RichText::new("SOURCE").size(t::MICRO).color(t::INK_MUTED));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let w = ui.available_width() - 210.0;
                    w::field(ui, "src_path", &mut self.source_path, "choose a folder or file", w.max(120.0), false);
                    if w::ghost_button(ui, "pickdir", "Folder...", 96.0).clicked() {
                        want_folder = true;
                    }
                    if w::ghost_button(ui, "pickfile", "File...", 88.0).clicked() {
                        want_file = true;
                    }
                });
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(
                        "Directory structure, permissions and symlinks are preserved.",
                    )
                    .size(t::MICRO)
                    .color(t::INK_MUTED),
                );
            },
        );
        if want_folder {
            self.pick_source_folder(now);
        }
        if want_file {
            self.pick_source_file(now);
        }

        ui.add_space(14.0);

        let cards = [
            ("Sealed", "Compressed and encrypted in bounded chunks. Nothing plaintext is written.", t::VIOLET),
            ("Bound", "Tied to this build. A payload cannot be moved into another capsule.", t::SKY),
            ("Standalone", "No reader, no server, no network. The capsule carries everything.", t::SKY),
        ];
        let cw = (ui.available_width() - 20.0) / 3.0;
        ui.horizontal(|ui| {
            for (i, (title, body, accent)) in cards.iter().enumerate() {
                let (rect, _) = ui.allocate_exact_size(vec2(cw, 104.0), Sense::hover());
                ui.painter()
                    .rect(rect, t::card_rounding(), t::alpha(t::SURFACE, 0.85), t::hairline());
                let bar = Rect::from_min_size(rect.min + vec2(16.0, 16.0), vec2(24.0, 3.0));
                ui.painter().rect_filled(bar, Rounding::same(2.0), *accent);
                ui.painter().text(
                    rect.min + vec2(16.0, 28.0),
                    Align2::LEFT_TOP,
                    *title,
                    t::font(t::H_SECTION),
                    t::INK,
                );
                let galley =
                    ui.painter()
                        .layout(body.to_string(), t::font(t::SMALL), t::INK_SOFT, cw - 32.0);
                ui.painter().galley(rect.min + vec2(16.0, 52.0), galley, t::INK_SOFT);
                if i < 2 {
                    ui.add_space(10.0 - ui.spacing().item_spacing.x);
                }
            }
        });
    }

    fn view_protections(&mut self, ui: &mut egui::Ui, now: f64) {
        w::section_title(
            ui,
            "Protections",
            "Every enabled protection must pass. Any one failing means the capsule stays shut.",
        );

        let mut machine = self.protections.machine;
        w::card_outlined(ui, "p_machine", 62.0, t::SKY, |ui, rect, _l| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Machine").size(t::H_SECTION).color(t::INK));
                    ui.label(
                        egui::RichText::new("Opens only on trusted machines. Cannot be disabled.")
                            .size(t::SMALL)
                            .color(t::INK_SOFT),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    w::switch(ui, "sw_machine", &mut machine, true);
                });
            });
            w::pill(ui.painter(), pos2(rect.left() + 100.0, rect.top() + 14.0), "always on", t::SKY, t::alpha(t::SKY, 0.12));
        });

        ui.add_space(8.0);
        let mut pass_on = self.protections.passphrase;
        w::card_outlined(ui, "p_pass", 146.0, t::SKY, |ui, rect, _l| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Passphrase").size(t::H_SECTION).color(t::INK));
                    ui.label(
                        egui::RichText::new("Memory-hard Argon2id with a per-capsule salt.")
                            .size(t::SMALL)
                            .color(t::INK_SOFT),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    w::switch(ui, "sw_pass", &mut pass_on, true);
                });
            });
            w::pill(ui.painter(), pos2(rect.left() + 132.0, rect.top() + 14.0), "always on", t::SKY, t::alpha(t::SKY, 0.12));
            ui.add_space(8.0);
            let w = ui.available_width();
            w::field(ui, "passphrase", &mut self.passphrase, "choose a strong passphrase", w, true);
            ui.add_space(6.0);
            let (score, why, colour) = w::passphrase_strength(&self.passphrase);
            w::strength_meter(ui, w, score, why, colour);
        });

        ui.add_space(16.0);
        ui.label(egui::RichText::new("OPTIONAL").size(t::MICRO).color(t::INK_MUTED));
        ui.add_space(6.0);

        let mut loc = self.protections.location;
        let loc_h = if loc { 138.0 } else { 66.0 };
        w::card_plain(ui, "p_loc", loc_h, t::SKY, loc, |ui, _r, _l| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Location").size(t::H_SECTION).color(t::INK));
                    ui.label(
                        egui::RichText::new("Acquired automatically. A vague fix is refused.")
                            .size(t::SMALL)
                            .color(t::INK_SOFT),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    w::switch(ui, "sw_loc", &mut loc, false);
                });
            });
            if loc {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Within").size(t::SMALL).color(t::INK_SOFT));
                    let w = (ui.available_width() - 20.0).min(320.0);
                    w::slider(ui, "loc_tol", &mut self.protections.location_tolerance_m, 25..=1000, w, " m");
                });
                ui.label(
                    egui::RichText::new("A reading less accurate than this is refused, not accepted.")
                        .size(t::MICRO)
                        .color(t::INK_MUTED),
                );
            }
        });
        self.protections.location = loc;

        ui.add_space(8.0);
        let mut tm = self.protections.time;
        let time_h = if tm { 146.0 } else { 66.0 };
        w::card_plain(ui, "p_time", time_h, t::SKY, tm, |ui, _r, _l| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Time window").size(t::H_SECTION).color(t::INK));
                    ui.label(
                        egui::RichText::new("A recurring daily window the capsule checks itself.")
                            .size(t::SMALL)
                            .color(t::INK_SOFT),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    w::switch(ui, "sw_time", &mut tm, false);
                });
            });
            if tm {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("At").size(t::SMALL).color(t::INK_SOFT));
                    w::field(ui, "time_of_day", &mut self.protections.time_of_day, "HH:MM", 84.0, false);
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("give or take").size(t::SMALL).color(t::INK_SOFT));
                    w::slider(ui, "time_tol", &mut self.protections.time_tolerance_min, 1..=180, 190.0, " min");
                });
                ui.label(
                    egui::RichText::new("Recurring, not an expiry: this repeats every day.")
                        .size(t::MICRO)
                        .color(t::INK_MUTED),
                );
            }
        });
        self.protections.time = tm;

        ui.add_space(8.0);
        let mut one = self.protections.one_shot;
        w::card_plain(ui, "p_shot", 66.0, t::ROSE, one, |ui, _r, _l| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("One-shot capsule").size(t::H_SECTION).color(t::INK));
                    ui.label(
                        egui::RichText::new("Best-effort self-delete after a verified extraction.")
                            .size(t::SMALL)
                            .color(t::INK_SOFT),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    w::switch(ui, "sw_shot", &mut one, false);
                });
            });
        });
        if one != self.protections.one_shot {
            if one {
                self.toast(now, "Deletion cannot be guaranteed on SSDs", t::ROSE);
            }
            self.protections.one_shot = one;
        }
    }

    fn view_machines(&mut self, ui: &mut egui::Ui, now: f64) {
        use nyedarch_buildtool::machines::{self, TagMode};

        w::section_title(
            ui,
            "Trusted machines",
            "A capsule opens on ANY trusted machine. This one is always included.",
        );

        // Search and tag filter.
        w::card_plain(ui, "msearch", 106.0, t::SKY, false, |ui, _r, _l| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Search").size(t::SMALL).color(t::INK_SOFT));
                let w = ui.available_width() - 190.0;
                w::field(ui, "msearch", &mut self.machine_query, "id or label", w, false);
                ui.add_space(8.0);
                let label = if self.tag_mode_all { "ALL tags" } else { "ANY tag" };
                if w::ghost_button(ui, "tagmode", label, 108.0).clicked() {
                    self.tag_mode_all = !self.tag_mode_all;
                }
            });
            ui.add_space(8.0);

            let labels = machines::all_labels();
            if labels.is_empty() {
                ui.label(
                    egui::RichText::new("No labels yet. Import a machine record to add some.")
                        .size(t::MICRO)
                        .color(t::INK_MUTED),
                );
            } else {
                ui.horizontal_wrapped(|ui| {
                    for lab in labels {
                        let on = self.selected_tags.contains(&lab);
                        let colour = if on { t::SKY } else { t::INK_MUTED };
                        let galley = ui.painter().layout_no_wrap(
                            lab.clone(),
                            t::font(t::MICRO),
                            colour,
                        );
                        let size = galley.size() + vec2(20.0, 10.0);
                        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                        ui.painter().rect(
                            rect,
                            Rounding::same(rect.height() / 2.0),
                            t::alpha(colour, if on { 0.20 } else { 0.08 }),
                            Stroke::new(1.0_f32, t::alpha(colour, if on { 0.8 } else { 0.35 })),
                        );
                        ui.painter().galley(rect.min + vec2(10.0, 5.0), galley, colour);
                        if resp.clicked() {
                            if on {
                                self.selected_tags.retain(|t| t != &lab);
                            } else {
                                self.selected_tags.push(lab.clone());
                            }
                        }
                    }
                });
            }
        });

        ui.add_space(12.0);

        let mode = if self.tag_mode_all { TagMode::All } else { TagMode::Any };
        let selected = machines::select(&self.machine_query, &self.selected_tags, mode);

        // What the capsule will actually contain (spec §48).
        w::card_outlined(ui, "mcount", 54.0, t::SKY, |ui, _r, _l| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("Included machines: {}", selected.len()))
                        .size(t::H_SECTION)
                        .color(t::INK),
                );
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("this machine is always included")
                        .size(t::MICRO)
                        .color(t::INK_MUTED),
                );
            });
        });

        ui.add_space(10.0);

        for (i, m) in selected.iter().enumerate() {
            let accent = if m.is_this_machine { t::VIOLET } else { t::SKY };
            let id = m.id[..16.min(m.id.len())].to_string();
            let labels = m.labels.join("   ");
            let creator = m.is_this_machine;
            w::card_plain(ui, &format!("m{i}"), 66.0, accent, creator, |ui, rect, _l| {
                ui.label(egui::RichText::new(&id).size(t::BODY).monospace().color(t::INK));
                ui.label(egui::RichText::new(&labels).size(t::MICRO).color(t::INK_MUTED));
                if creator {
                    w::pill(
                        ui.painter(),
                        pos2(rect.right() - 118.0, rect.center().y - 9.0),
                        "this machine",
                        t::VIOLET,
                        t::alpha(t::VIOLET, 0.12),
                    );
                }
            });
            ui.add_space(8.0);
        }

        ui.add_space(4.0);
        let mut do_import = false;
        let mut do_export = false;
        ui.horizontal(|ui| {
            if w::ghost_button(ui, "import", "Import machine (.nyfp)", 196.0).clicked() {
                do_import = true;
            }
            if w::ghost_button(ui, "export", "Export this machine", 176.0).clicked() {
                do_export = true;
            }
        });
        if do_import {
            self.import_machine(now);
        }
        if do_export {
            self.export_machine(now);
        }

        ui.add_space(12.0);
        w::note(ui, "Records are authenticated. Editing a record's labels invalidates it, so a machine cannot be relabelled into a capsule it was never trusted for.", t::VIOLET, t::alpha(t::VIOLET, 0.08));
    }

    fn view_targets(&mut self, ui: &mut egui::Ui, now: f64) {
        w::section_title(
            ui,
            "Build targets",
            "Capsules are compiled remotely, then checked against a commitment made before the build started.",
        );

        let mut rows = [
            ("Linux", "x86_64-unknown-linux-gnu", &mut self.target_linux),
            ("Windows", "x86_64-pc-windows-msvc", &mut self.target_windows),
            ("macOS", "aarch64-apple-darwin", &mut self.target_macos),
        ];
        for (i, (name, triple, flag)) in rows.iter_mut().enumerate() {
            let on = **flag;
            let mut local = on;
            w::card_plain(ui, &format!("tg{i}"), 60.0, t::SKY, on, |ui, _r, _l| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(*name).size(t::H_SECTION).color(t::INK));
                        ui.label(
                            egui::RichText::new(*triple)
                                .size(t::MICRO)
                                .monospace()
                                .color(t::INK_MUTED),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        w::switch(ui, &format!("swtg{i}"), &mut local, false);
                    });
                });
            });
            **flag = local;
            ui.add_space(8.0);
        }

        ui.add_space(6.0);
        ui.add_space(8.0);
        let mut apply_now = false;
        ui.horizontal(|ui| {
            if w::ghost_button(ui, "applyvis", "Apply visibility to the repository now", 300.0).clicked() {
                apply_now = true;
            }
            ui.label(
                egui::RichText::new("refused while a build is running")
                    .size(t::MICRO)
                    .color(t::INK_MUTED),
            );
        });
        if apply_now {
            self.apply_visibility(now);
        }

        ui.add_space(14.0);
        ui.label(egui::RichText::new("REMOTE BUILD").size(t::MICRO).color(t::INK_MUTED));
        ui.add_space(6.0);

        // Credentials, not a switch. The build always happens remotely.
        let locked = self.gh_repo_locked;
        let token_locked = self.gh_token_locked;
        let mut unlock = false;
        let mut save_token = false;
        let mut change_token = false;
        w::card_plain(ui, "gh", 168.0, t::SKY, false, |ui, rect, _l| {
            ui.label(egui::RichText::new("BUILD ACCOUNT").size(t::MICRO).color(t::INK_MUTED));
            ui.add_space(10.0);

            // One label column for every row. Mixed ad-hoc spacing is what made
            // the token row sit crooked and overflow the card.
            let label_w = 96.0;
            let field_w = (rect.width() - 36.0 - label_w).max(180.0);

            ui.horizontal(|ui| {
                ui.add_sized(
                    vec2(label_w, 24.0),
                    egui::Label::new(
                        egui::RichText::new("Account").size(t::SMALL).color(t::INK_SOFT),
                    ),
                );
                match &self.gh_owner_resolved {
                    // Read from the token: it already proves which account it
                    // belongs to, so asking would only invite a mismatch.
                    Some(o) => {
                        ui.label(egui::RichText::new(o).size(t::SMALL).color(t::INK));
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("from token").size(t::MICRO).color(t::SKY_DEEP),
                        );
                    }
                    None => {
                        ui.label(
                            egui::RichText::new("resolved from the token once it is entered")
                                .size(t::SMALL)
                                .color(t::INK_MUTED),
                        );
                    }
                }
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_sized(
                    vec2(label_w, 24.0),
                    egui::Label::new(
                        egui::RichText::new("Repository").size(t::SMALL).color(t::INK_SOFT),
                    ),
                );
                if locked {
                    ui.label(egui::RichText::new(&self.gh_repo).size(t::SMALL).color(t::INK));
                    ui.add_space(10.0);
                    if w::ghost_button(ui, "unlockrepo", "Change", 82.0).clicked() {
                        unlock = true;
                    }
                } else {
                    w::field(ui, "gh_repo", &mut self.gh_repo, "repository", field_w, false);
                }
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_sized(
                    vec2(label_w, 24.0),
                    egui::Label::new(
                        egui::RichText::new("Token").size(t::SMALL).color(t::INK_SOFT),
                    ),
                );
                if token_locked {
                    // Never shown again once stored, not even masked: there is
                    // no reason to put it back on screen.
                    ui.label(egui::RichText::new("saved on this machine").size(t::SMALL).color(t::INK));
                    ui.add_space(10.0);
                    if w::ghost_button(ui, "changetoken", "Change", 82.0).clicked() {
                        change_token = true;
                    }
                } else {
                    w::field(
                        ui,
                        "gh_token",
                        &mut self.gh_token,
                        "personal access token, repo scope",
                        field_w - 96.0,
                        true,
                    );
                    ui.add_space(8.0);
                    if w::ghost_button(ui, "savetoken", "Save", 82.0).clicked() {
                        save_token = true;
                    }
                }
            });
        });
        if unlock {
            self.gh_repo_locked = false;
        }
        if save_token {
            let value = self.gh_token.trim().to_string();
            if value.is_empty() {
                self.toast(now, "Enter a token first", t::ROSE);
            } else {
                match nyedarch_buildtool::keystore::save_secret("github-token", value.as_bytes()) {
                    Ok(()) => {
                        self.gh_token_locked = true;
                        self.gh_owner_resolved = None; // re-resolve under the new token
                        self.say(now, "Token stored, encrypted, on this machine.");
                        self.toast(now, "Token saved", t::SKY);
                    }
                    Err(e) => {
                        self.say(now, format!("Could not store the token: {e}"));
                        self.toast(now, "Token not saved", t::ROSE);
                    }
                }
            }
        }
        if change_token {
            nyedarch_buildtool::keystore::forget_secret("github-token");
            self.gh_token.clear();
            self.gh_token_locked = false;
            self.gh_owner_resolved = None;
            self.say(now, "Stored token removed. Enter a new one.");
        }

        ui.add_space(10.0);
        w::note(
            ui,
            "The capsule source is encrypted before it is pushed, and the key is a repository \
             secret only the build runner can read. GitHub never receives your files, passphrase, \
             fingerprints or any payload key.",
            t::SKY,
            t::SKY_WASH,
        );
    }

    fn view_build(&mut self, ui: &mut egui::Ui, now: f64) {
        w::section_title(ui, "Build the capsule", "Check the summary, then build.");

        let col_gap = 20.0;
        let left_w = 210.0;
        let right_w = (ui.available_width() - left_w - col_gap).max(260.0);

        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                vec2(left_w, 250.0),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    ui.add_space(6.0);
                    w::build_indicator(ui, 176.0, self.build_progress, self.building, now);
                    ui.add_space(8.0);
                    let label = if self.building {
                        format!("{:.0}%", self.build_progress * 100.0)
                    } else if self.build_progress >= 1.0 {
                        "Sealed".to_string()
                    } else {
                        "Not built".to_string()
                    };
                    ui.label(egui::RichText::new(label).size(t::H_SECTION).color(t::INK));
                },
            );

            ui.add_space(col_gap);

            ui.allocate_ui_with_layout(
                vec2(right_w, 250.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let rows: Vec<(String, String, Color32)> = vec![
                        (
                            "Machine".into(),
                            format!(
                            "{} trusted machine(s)",
                            nyedarch_buildtool::machines::select(
                                &self.machine_query,
                                &self.selected_tags,
                                if self.tag_mode_all {
                                    nyedarch_buildtool::machines::TagMode::All
                                } else {
                                    nyedarch_buildtool::machines::TagMode::Any
                                },
                            )
                            .len()
                        ),
                            t::SKY,
                        ),
                        ("Passphrase".into(), "Argon2id, memory-hard".into(), t::SKY),
                        (
                            "Location".into(),
                            if self.protections.location {
                                format!("within {} m", self.protections.location_tolerance_m)
                            } else {
                                "off".into()
                            },
                            if self.protections.location { t::SKY } else { t::INK_MUTED },
                        ),
                        (
                            "Time".into(),
                            if self.protections.time {
                                format!(
                                    "{} +/- {} min, daily",
                                    self.protections.time_of_day, self.protections.time_tolerance_min
                                )
                            } else {
                                "off".into()
                            },
                            if self.protections.time { t::SKY } else { t::INK_MUTED },
                        ),
                        (
                            "Execution".into(),
                            if self.protections.one_shot { "one-shot".into() } else { "reusable".into() },
                            if self.protections.one_shot { t::ROSE } else { t::INK_MUTED },
                        ),
                        (
                            "Repository".into(),
                            if self.repo_should_be_private() { "private".into() } else { "PUBLIC".into() },
                            if self.repo_should_be_private() { t::SKY } else { t::ROSE },
                        ),
                    ];

                    ui.label(egui::RichText::new("SUMMARY").size(t::MICRO).color(t::INK_MUTED));
                    ui.add_space(4.0);
                    for (i, (k, v, c)) in rows.iter().enumerate() {
                        let (rect, _) =
                            ui.allocate_exact_size(vec2(right_w, 34.0), Sense::hover());
                        if i % 2 == 0 {
                            ui.painter().rect_filled(
                                rect,
                                Rounding::same(8.0),
                                t::alpha(t::SURFACE, 0.75),
                            );
                        }
                        ui.painter()
                            .circle_filled(pos2(rect.left() + 12.0, rect.center().y), 4.0, *c);
                        ui.painter().text(
                            pos2(rect.left() + 26.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            k,
                            t::font(t::SMALL),
                            t::INK_SOFT,
                        );
                        ui.painter().text(
                            pos2(rect.right() - 10.0, rect.center().y),
                            Align2::RIGHT_CENTER,
                            v,
                            t::font(t::SMALL),
                            *c,
                        );
                    }
                },
            );
        });

        ui.add_space(14.0);
        let ready = self.ready_to_build() && !self.building;
        let label = if self.building { "Building..." } else { "Build capsule" };
        ui.horizontal(|ui| {
            if w::primary_button(ui, "build", label, ready, 200.0).clicked() && ready {
                self.start_build(now);
            }
            if self.building {
                ui.add_space(10.0);
                if w::ghost_button(ui, "cancelbuild", "Cancel", 120.0).clicked() {
                    if let Some(c) = &self.cancel_flag {
                        c.store(true, std::sync::atomic::Ordering::Relaxed);
                        self.say(now, "Cancelling; nothing partial will be left behind.");
                    }
                }
            }
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            use nyedarch_package::pipeline::CompressionMode as CM;
            ui.label(egui::RichText::new("Compression").size(t::SMALL).color(t::INK_SOFT));
            ui.add_space(8.0);
            let modes = [CM::Automatic, CM::Balanced, CM::Maximum, CM::Fast];
            let labels: Vec<&str> = modes.iter().map(|m| m.label()).collect();
            let current = modes.iter().position(|m| *m == self.compression).unwrap_or(0);
            if let Some(i) = w::segmented(ui, "compression", &labels, current) {
                self.compression = modes[i];
            }
        });

        ui.add_space(6.0);
        let mut creator = self.creator_mode;
        ui.horizontal(|ui| {
            w::switch(ui, "sw_creator", &mut creator, false);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Creator mode").size(t::SMALL).color(t::INK));
                ui.label(
                    egui::RichText::new(
                        "Diagnostics about this build only. It grants no authority, skips no check, \
                         and weakens no capsule.",
                    )
                    .size(t::MICRO)
                    .color(t::INK_MUTED),
                );
            });
        });
        self.creator_mode = creator;

        ui.add_space(6.0);
        let mut logs = self.with_logs;
        ui.horizontal(|ui| {
            w::switch(ui, "sw_logs", &mut logs, false);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Verbose logs").size(t::SMALL).color(t::INK));
                ui.label(
                    egui::RichText::new(
                        "Timestamped diagnostics in IST: which provider answered a location \
                         request and how accurate it was, how long key derivation took, how much \
                         was sealed.",
                    )
                    .size(t::MICRO)
                    .color(t::INK_MUTED),
                );
            });
        });
        if logs != self.with_logs {
            self.with_logs = logs;
            // The pipeline reads this flag on the worker thread, so it has to be
            // set globally rather than passed down.
            nyedarch_buildtool::logging::set_verbose(logs);
            // Buffer rather than write to stderr: from an application bundle,
            // stderr goes nowhere the user will look.
            nyedarch_buildtool::logging::set_capture(logs);
            self.say(now, if logs { "Verbose logging on." } else { "Verbose logging off." });
        }
        if !self.ready_to_build() {
            ui.add_space(6.0);
            let missing = if !self.step_done(Step::Source) {
                "Choose what to protect first."
            } else if !self.step_done(Step::Protections) {
                "Set a passphrase first."
            } else {
                "Choose at least one build target."
            };
            ui.label(egui::RichText::new(missing).size(t::SMALL).color(t::INK_MUTED));
        }

        ui.add_space(16.0);

        // No progress bar here.
        //
        // The perimeter pulse already reports that work is happening, and it
        // does it from anywhere in the window. A second indicator for the same
        // fact competes with it and adds nothing - the stage line below says
        // what is happening, which a bar cannot.
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(if self.building {
                    "Working"
                } else if self.build_progress >= 1.0 {
                    "Complete"
                } else {
                    "Ready"
                })
                .size(t::SMALL)
                .color(if self.building { t::SKY_DEEP } else { t::INK_SOFT }),
            );
        });
        ui.add_space(10.0);

        let log_h = 150.0_f32.min(ui.available_height().max(90.0));
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), log_h), Sense::hover());
        ui.painter().rect(rect, t::card_rounding(), t::SUNKEN, t::hairline());
        let mut y = rect.bottom() - 20.0;
        for (ts, line) in self.log.iter().rev() {
            if y < rect.top() + 10.0 {
                break;
            }
            let age = (now - ts) as f32;
            let fade = (1.0 - (age / 60.0)).clamp(0.4, 1.0);
            // Colour by kind: attention for warnings, refusal for failures.
            let lower = line.to_ascii_lowercase();
            let col = if lower.contains("warning") || lower.contains("could not") {
                t::AMBER
            } else if lower.contains("failed") || lower.contains("refused") {
                t::ROSE
            } else {
                t::INK_SOFT
            };
            ui.painter().text(
                pos2(rect.left() + 14.0, y),
                Align2::LEFT_CENTER,
                line,
                t::mono(t::SMALL),
                t::alpha(col, fade),
            );
            y -= 19.0;
        }
    }

    fn view_run(&mut self, ui: &mut egui::Ui, now: f64, hovering_file: bool) {
        w::section_title(
            ui,
            "Run a capsule",
            "Drop a .nyarch capsule anywhere on this window, or open one from the Capsule menu.",
        );

        let name = self
            .dropped_capsule
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|f| f.to_string_lossy().to_string());
        w::drop_zone(ui, 200.0, hovering_file, name.as_deref(), now);

        ui.add_space(14.0);
        let mut want_out = false;
        w::card_plain(ui, "outdir", 88.0, t::SKY, false, |ui, _r, _l| {
            ui.label(egui::RichText::new("EXTRACT TO").size(t::MICRO).color(t::INK_MUTED));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let w = ui.available_width() - 110.0;
                w::field(ui, "outdir", &mut self.run_out_dir, "leave empty for the capsule's default", w.max(120.0), false);
                if w::ghost_button(ui, "pickout", "Choose...", 100.0).clicked() {
                    want_out = true;
                }
            });
        });
        if want_out {
            self.pick_output_dir(now);
        }

        ui.add_space(12.0);
        let can_run = self.dropped_capsule.is_some();
        if w::primary_button(ui, "run", "Run capsule", can_run, 190.0).clicked() && can_run {
            if let Some(path) = self.dropped_capsule.clone() {
                let out = if self.run_out_dir.trim().is_empty() {
                    None
                } else {
                    Some(std::path::PathBuf::from(self.run_out_dir.trim()))
                };
                match nyedarch_core::launch::launch(&path, out.as_deref()) {
                    Ok(pid) => {
                        self.run_ok = true;
                        self.run_status = format!("Running as an independent process (pid {pid}).");
                        self.say(now, format!("Launched capsule, pid {pid}."));
                        self.toast(now, "Capsule launched", t::SKY);
                    }
                    Err(e) => {
                        self.run_ok = false;
                        self.run_status = format!("Could not start it: {e}");
                        self.say(now, format!("Launch failed: {e}"));
                        self.toast(now, "Launch failed", t::ROSE);
                    }
                }
            }
        }

        if !self.run_status.is_empty() {
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new(&self.run_status)
                    .size(t::SMALL)
                    .color(if self.run_ok { t::SKY } else { t::ROSE }),
            );
        }

        ui.add_space(10.0);
        w::note(ui, "The capsule authorizes itself. This application grants it nothing and never sees its passphrase.", t::SKY, t::alpha(t::SKY, 0.08));
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(nyedarch_core::launch::terminal_hint())
                .size(t::MICRO)
                .color(t::INK_MUTED),
        );
    }
}

// ----------------------------------------------------------------- modals ---

impl App {
    /// Licence gate (spec §13). Until this is accepted the application shows
    /// nothing else: no menus, no steps, no fingerprint work. Declining closes
    /// the window rather than leaving a half-usable client.
    fn draw_eula_gate(&mut self, ctx: &egui::Context, now: f64) {
        let screen = ctx.screen_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("eula_veil"),
        ));
        painter.rect_filled(screen, Rounding::ZERO, t::CANVAS);
        // A soft sky bloom behind the licence panel, painted straight onto the
        // background layer rather than through a throwaway Ui.
        w::glow(
            &painter,
            pos2(screen.center().x, screen.top() + screen.height() * 0.30),
            screen.width() * 0.45,
            t::SKY,
            0.25,
        );

        egui::Area::new(egui::Id::new("eula_gate"))
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_max_width(660.0);
                egui::Frame::none()
                    .fill(t::SURFACE)
                    .stroke(Stroke::new(1.0_f32, t::alpha(t::SKY, 0.35)))
                    .rounding(t::card_rounding())
                    .inner_margin(egui::Margin::symmetric(26.0, 22.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(52.0, 52.0), Sense::hover());
                            w::aperture(ui.painter(), r.center(), 46.0, now, 0.0, false);
                            ui.add_space(6.0);
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new("Before you use NYEDArch")
                                        .size(t::H_TITLE)
                                        .color(t::INK),
                                );
                                ui.label(
                                    egui::RichText::new("End User Licence Agreement")
                                        .size(t::SMALL)
                                        .color(t::INK_MUTED),
                                );
                            });
                        });
                        ui.add_space(14.0);

                        egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                            for para in nyedarch_buildtool::eula::disclosures() {
                                ui.label(
                                    egui::RichText::new(para).size(t::SMALL).color(t::INK_SOFT),
                                );
                                ui.add_space(8.0);
                            }
                        });

                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            if w::primary_button(ui, "eula_ok", "I accept", true, 180.0).clicked() {
                                match nyedarch_buildtool::eula::record_acceptance() {
                                    Ok(()) => {
                                        self.eula_accepted = true;
                                        self.say(now, "Licence accepted and recorded.");
                                    }
                                    Err(e) => {
                                        // Accepting but failing to persist would
                                        // silently re-prompt forever; say so.
                                        self.eula_accepted = true;
                                        self.say(now, format!("Accepted, but {e}"));
                                    }
                                }
                            }
                            ui.add_space(10.0);
                            if w::ghost_button(ui, "eula_no", "Decline and quit", 180.0).clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        });
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Full text: docs/EULA.md")
                                .size(t::MICRO)
                                .color(t::INK_MUTED),
                        );
                    });
            });
    }

    fn draw_modal(&mut self, ctx: &egui::Context) {
        if self.modal == Modal::None {
            return;
        }
        let (title, body): (&str, Vec<&str>) = match self.modal {
            Modal::Eula => (
                "End User Licence Agreement",
                vec![
                    "NYEDArch reads platform identifiers to build a machine fingerprint. Only salted digests are stored; raw identifiers never are, and nothing is transmitted.",
                    "Capsules are compiled by GitHub Actions in a repository in your own account. Private is the default. A public repository exposes your generated source and build logs.",
                    "Private key material is held in your platform's secure storage. LOSING IT MAY MAKE EXISTING CAPSULES PERMANENTLY UNRECOVERABLE. There is no recovery and no backdoor.",
                    "The time protection uses the local clock, which whoever controls the machine can change. It is a recurring policy control, not a tamper-proof expiry.",
                    "One-shot deletion is best effort. Software cannot guarantee erasure on SSDs or copy-on-write filesystems.",
                    "NYEDArch raises the cost of unauthorized access. It is NOT unbreakable. See docs/ANTI_RE_ANALYSIS.md for exactly what someone holding a capsule can extract.",
                    "A capsule is not a backup. Keep independent copies of anything you seal.",
                    "Full text: docs/EULA.md",
                ],
            ),
            Modal::About => (
                "About NYEDArch",
                vec![
                    "NYEDArch - Not Your Everyday Archive.",
                    "An ordinary archive is passive: it waits for a program to open it, and once copied it protects nothing. A NYEDArch capsule is the opposite. It is an executable that carries its data, the rules for opening it, and the logic to enforce them.",
                    "The aperture in this interface is the idea itself: layers that stay shut until every protection is satisfied, on the machine, by the person, in the place, at the time you chose.",
                    "Prototype 0.0.1  ·  crypto version 1  ·  package format 1",
                ],
            ),
            Modal::Shortcuts => (
                "Keyboard",
                vec![
                    "Ctrl+N       New capsule",
                    "Ctrl+O       Open a capsule to run",
                    "Ctrl+B       Build capsule",
                    "Ctrl+Enter   Build capsule",
                    "Esc          Close this panel",
                    "",
                    "Drop a .nyarch file anywhere on the window to load it.",
                ],
            ),
            Modal::None => return,
        };

        let screen = ctx.screen_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("modal_veil"),
        ));
        painter.rect_filled(screen, Rounding::ZERO, t::alpha(t::CANVAS, 0.72));

        let mut open = true;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .default_width(560.0)
            .frame(
                egui::Frame::none()
                    .fill(t::SURFACE)
                    .stroke(Stroke::new(1.0_f32, t::alpha(t::SKY, 0.35)))
                    .rounding(t::card_rounding())
                    .inner_margin(egui::Margin::symmetric(22.0, 18.0)),
            )
            .show(ctx, |ui| {
                for line in body {
                    if line.is_empty() {
                        ui.add_space(6.0);
                        continue;
                    }
                    let mono = matches!(self.modal, Modal::Shortcuts);
                    let rt = if mono {
                        egui::RichText::new(line).monospace().size(t::SMALL).color(t::INK_SOFT)
                    } else {
                        egui::RichText::new(line).size(t::SMALL).color(t::INK_SOFT)
                    };
                    ui.label(rt);
                    ui.add_space(6.0);
                }
                ui.add_space(6.0);
                if w::ghost_button(ui, "modal_close", "Close", 110.0).clicked() {
                    self.modal = Modal::None;
                }
            });
        if !open {
            self.modal = Modal::None;
        }
    }
}

// ------------------------------------------------------------------- app ----

impl App {
    fn handle_drops(&mut self, ctx: &egui::Context, now: f64) {
        let dropped: Vec<std::path::PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if dropped.is_empty() {
            return;
        }
        self.goto(Step::Run, now);
        for path in dropped {
            match nyedarch_core::launch::validate(&path) {
                Ok(()) => {
                    self.run_status.clear();
                    self.run_ok = false;
                    self.toast(now, "Capsule ready", t::SKY);
                    self.say(now, format!("Loaded {}", path.display()));
                    self.dropped_capsule = Some(path);
                }
                Err(e) => {
                    self.dropped_capsule = None;
                    self.run_ok = false;
                    self.run_status = format!("Cannot run that file: {e}");
                    self.toast(now, "Not a capsule", t::ROSE);
                }
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context, now: f64) {
        let (new_c, open_c, build_c, esc) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(egui::Key::N),
                i.modifiers.command && i.key_pressed(egui::Key::O),
                (i.modifiers.command && i.key_pressed(egui::Key::B))
                    || (i.modifiers.command && i.key_pressed(egui::Key::Enter)),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if esc {
            self.modal = Modal::None;
        }
        if new_c {
            self.reset(now);
        }
        if open_c {
            self.pick_capsule(now);
        }
        if build_c && self.ready_to_build() && !self.building {
            self.goto(Step::Build, now);
            self.start_build(now);
        }
    }

    fn draw_toast(&mut self, ctx: &egui::Context, now: f64) {
        let Some((msg, at, colour)) = self.toast.clone() else {
            return;
        };
        let age = (now - at) as f32;
        let life = 2.8;
        if age > life {
            self.toast = None;
            return;
        }
        let appear = t::ease_out_back((age / 0.28).clamp(0.0, 1.0));
        let fade = if age > life - 0.5 {
            1.0 - (age - (life - 0.5)) / 0.5
        } else {
            1.0
        };

        let screen = ctx.screen_rect();
        let (wd, ht) = (300.0, 46.0);
        let rect = Rect::from_min_size(
            pos2(screen.right() - wd - 22.0, screen.top() + 46.0 + (appear - 1.0) * 40.0),
            vec2(wd, ht),
        );
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("toast"),
        ));
        painter.rect(
            rect,
            t::card_rounding(),
            t::alpha(t::SUNKEN, 0.97 * fade),
            Stroke::new(1.0_f32, t::alpha(colour, 0.85 * fade)),
        );
        painter.circle_filled(pos2(rect.left() + 20.0, rect.center().y), 5.0, t::alpha(colour, fade));
        painter.text(
            pos2(rect.left() + 38.0, rect.center().y),
            Align2::LEFT_CENTER,
            msg,
            t::font(t::SMALL),
            t::alpha(t::INK, fade),
        );
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        t::apply(ctx);
        ctx.request_repaint_after(std::time::Duration::from_millis(16));

        let now = ctx.input(|i| i.time);
                let hovering_file = ctx.input(|i| !i.raw.hovered_files.is_empty());

        // Nothing else runs until the licence is accepted.
        if !self.eula_accepted {
            self.draw_eula_gate(ctx, now);
            self.draw_toast(ctx, now);
            return;
        }

        self.shortcuts(ctx, now);
        self.handle_drops(ctx, now);

        self.poll_build(now);

        // Diagnostics produced on the worker thread, drained into the log the
        // user is actually looking at.
        if self.with_logs {
            for line in nyedarch_buildtool::logging::drain() {
                self.log.push((now, line));
            }
        }

        self.menu_bar(ctx, now);
        self.rail(ctx, now);

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(t::CANVAS))
            .show(ctx, |ui| {
                let full = ui.max_rect();

                let since = (now - self.step_changed_at) as f32;
                let e = t::ease_out_cubic((since / 0.30).clamp(0.0, 1.0));
                let content = full.shrink2(vec2(30.0, 22.0)).translate(vec2(0.0, (1.0 - e) * 16.0));
                let mut child = ui.child_ui(content, egui::Layout::top_down(egui::Align::Min));
                child.set_opacity(e);

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(&mut child, |ui| match self.step {
                        Step::Source => self.view_source(ui, now),
                        Step::Protections => self.view_protections(ui, now),
                        Step::Machines => self.view_machines(ui, now),
                        Step::Targets => self.view_targets(ui, now),
                        Step::Build => self.view_build(ui, now),
                        Step::Run => self.view_run(ui, now, hovering_file),
                    });
            });

        if hovering_file && self.step != Step::Run {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("dropveil"),
            ));
            let r = ctx.screen_rect();
            painter.rect_filled(r, Rounding::ZERO, t::alpha(t::CANVAS, 0.6));
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                "Release to load the capsule",
                t::font(t::H_TITLE),
                t::SKY,
            );
        }

        // The application's pulse: a thin blacklight comet circling the window,
        // slow while idle and visibly faster while a build runs. Drawn last so
        // it sits above every panel, and never over the licence gate - which is
        // the one screen where nothing should distract from a decision.
        if self.eula_accepted {
            let (speed, intensity) = if self.building {
                (0.68, 1.30)
            } else if self.build_progress >= 1.0 {
                (0.16, 0.85)
            } else {
                (0.115, 0.78)
            };
            // Ease between speeds so a build starting or finishing accelerates
            // smoothly rather than jumping.
            let shown = ctx.animate_value_with_time(egui::Id::new("pulse_speed"), speed, 0.9);
            let shown_i = ctx.animate_value_with_time(egui::Id::new("pulse_intensity"), intensity, 0.9);
            self.pulse_phase += (now - self.pulse_last) * shown as f64;
            self.pulse_last = now;
            w::perimeter_pulse_at(ctx, self.pulse_phase, shown_i);
        }

        self.draw_modal(ctx);
        self.draw_toast(ctx, now);
    }
}

/// The application icon, compiled in so there is no file to lose.
///
/// Used for the window, the taskbar and the dock. The macOS bundle and the
/// Windows executable get their own copies from `assets/icon` at packaging
/// time, because those platforms read an icon from the bundle rather than from
/// the running process.
fn load_icon() -> egui::IconData {
    let bytes = include_bytes!("../assets/icon-256.png");
    match image_from_png(bytes) {
        Some(icon) => icon,
        // An icon is cosmetic: failing to decode it must never stop the client
        // from starting.
        None => egui::IconData { rgba: vec![0; 4], width: 1, height: 1 },
    }
}

/// Minimal PNG decode via the `image` crate if present, else a 1x1 fallback.
fn image_from_png(bytes: &[u8]) -> Option<egui::IconData> {
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width, height })
}

fn main() -> eframe::Result<()> {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(load_icon())
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([940.0, 640.0])
            .with_title("NYEDArch"),
        ..Default::default()
    };
    eframe::run_native("NYEDArch", opts, Box::new(|_cc| Box::<App>::default()))
}
