//! Command-line interface definition.
//!
//! The surface is described declaratively so that `--help` is generated from
//! the same source as the parser, rather than maintained beside it. A help text
//! that drifts from the parser is worse than none: it tells the operator
//! something untrue about a security tool.
//!
//! Help text here explains *consequences*, not just syntax. Someone reading
//! `--one-shot` should learn that deletion is best effort, and someone reading
//! `--hardware required` should learn that the build is refused rather than
//! downgraded. That is the difference between documentation and a word list.

use clap::{Parser, Subcommand};

pub const ABOUT: &str = "NYEDArch - Not Your Everyday Archive.

Turns data into a capsule: a self-contained executable that carries its own
payload, the rules for opening it, and the logic to enforce them. A capsule
protects itself once it leaves the machine that made it.";

pub const AFTER_HELP: &str = "\
EXAMPLES
  Seal a folder into a capsule project:
    nyedarch seal ./reports ./reports-capsule 'my passphrase'

  Add the location and time protections:
    nyedarch seal ./reports ./out 'pw' --location 150 --time 14:00

  Trust every machine labelled HR in Bangalore, and this one:
    nyedarch seal ./reports ./out 'pw' --trust-tag HR --trust-tag Bangalore --tag-mode all

  Build the capsule remotely for every target:
    nyedarch build ./out my-github-user nyedarch-builds private

NOTES
  Machine authorization and the passphrase are always required and cannot be
  switched off. Location and time are optional; when enabled they are also
  required. Every enabled protection must pass.

  The passphrase may be given as an argument for scriptability, or left out and
  supplied in NYEDARCH_PASSPHRASE. Prefer the environment variable on a shared
  machine: an argument is visible in the process list to every other user.";

#[derive(Parser, Debug)]
#[command(
    name = "nyedarch",
    version,
    about = ABOUT,
    after_help = AFTER_HELP,
    propagate_version = true,
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Verbose diagnostics, timestamped in IST (UTC+05:30).
    ///
    /// Reports what each step actually did: which provider answered a location
    /// request and how accurate the fix was, how long Argon2id took, how many
    /// bytes were sealed. Off by default, because a tool that narrates
    /// everything trains people to stop reading it.
    ///
    /// Diagnostics confirm behaviour; they never reproduce secrets. Accuracy in
    /// metres, never coordinates. Signal names, never raw identifiers.
    #[arg(long, global = true)]
    pub with_logs: bool,

    /// Print extra detail about this client's own work.
    ///
    /// Diagnostic only: it grants no authority, skips no check, and produces
    /// no weaker capsule.
    #[arg(long, global = true)]
    pub creator: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Seal files into a capsule project.
    #[command(after_help = "\
The output is a Rust project, not a finished capsule. Compile it locally with
stage-capsule.sh, or build it for every platform with `nyedarch build`.

Nothing plaintext is written: the payload is compressed and encrypted in
bounded chunks as it is read.")]
    Seal(SealArgs),

    /// Build a sealed capsule project remotely, for every selected target.
    #[command(after_help = "\
The capsule source is encrypted before it is pushed, whether the repository is
public or private, and the key is stored as a repository secret only the build
runner can read.

The returned artifact is checked against a commitment made before the build
began, then deleted from GitHub: the artifact is the capsule binary, and on a
public repository anyone authenticated could otherwise fetch it.

Requires a token in NYEDARCH_GITHUB_TOKEN or GITHUB_TOKEN.")]
    Build(BuildArgs),

    /// Change a build repository between public and private.
    #[command(after_help = "\
Refused while a build is running, checked against GitHub rather than local
state - the build may have been started elsewhere.

A public repository exposes your build logs and workflow to anyone. It does not
expose the capsule source, which is encrypted either way.")]
    Visibility(VisibilityArgs),

    /// Manage the trusted machine registry.
    Machines {
        #[command(subcommand)]
        action: MachinesAction,
    },

    /// Export this machine as an authenticated .nyfp record.
    #[command(after_help = "\
Give the record to whoever builds a capsule you need to open. It is
authenticated: editing it, including its labels, invalidates it.")]
    Export(ExportArgs),

    /// Run a capsule.
    #[command(after_help = "\
The capsule authorizes itself. This client grants it nothing and never sees its
passphrase.")]
    Run(RunArgs),

    /// Measure this machine: fingerprinting, key derivation, compression.
    Bench,
}

#[derive(clap::Args, Debug)]
pub struct SealArgs {
    /// File or directory to protect.
    pub input: std::path::PathBuf,

    /// Where to write the capsule project.
    pub output: std::path::PathBuf,

    /// Passphrase required to open the capsule.
    ///
    /// Always required; it cannot be disabled. Derived with Argon2id and a
    /// per-capsule salt, and never stored.
    ///
    /// May be omitted, in which case NYEDARCH_PASSPHRASE is used. Prefer that
    /// on a shared machine: an argument is visible in the process list to every
    /// other user, and often lands in shell history.
    pub passphrase: Option<String>,

    /// Require the capsule to be opened within this many metres of here.
    ///
    /// The location is acquired from the operating system, or from your browser
    /// with permission - never typed in, and never from an IP lookup. A reading
    /// less accurate than this value is refused rather than accepted.
    #[arg(long, value_name = "METRES")]
    pub location: Option<u32>,

    /// Require the capsule to be opened inside a daily time window.
    ///
    /// Recurring, not an expiry: 14:00 means every day at 14:00.
    #[arg(long, value_name = "HH:MM", value_delimiter = ',')]
    pub time: Vec<String>,

    /// Width of the time window, in minutes either side.
    #[arg(long, value_name = "N", default_value_t = 15)]
    pub time_tolerance_min: u32,

    /// Timezone offset from UTC, in minutes.
    #[arg(long, value_name = "N", default_value_t = 0, allow_negative_numbers = true)]
    pub tz_offset_min: i32,

    /// Destroy the capsule after a verified extraction.
    ///
    /// Best effort. Software cannot guarantee erasure on SSDs or copy-on-write
    /// filesystems, and it never reaches a copy made beforehand.
    #[arg(long)]
    pub one_shot: bool,

    /// Trust an additional machine from its .nyfp record. Repeatable.
    #[arg(long, value_name = "FILE.nyfp")]
    pub trust: Vec<std::path::PathBuf>,

    /// Trust machines from the registry carrying this label. Repeatable.
    #[arg(long, value_name = "LABEL")]
    pub trust_tag: Vec<String>,

    /// Whether a machine needs any selected label or all of them.
    #[arg(long, value_name = "MODE", default_value = "any", value_parser = ["any", "all"])]
    pub tag_mode: String,

    /// Filter registry machines by id prefix or label text.
    #[arg(long, value_name = "TEXT", default_value = "")]
    pub trust_search: String,

    /// How hard to compress. Effort only; it never changes the format.
    #[arg(long, value_name = "MODE", default_value = "automatic",
          value_parser = ["automatic", "maximum", "balanced", "fast"])]
    pub compression: String,

    /// Whether hardware-backed machine protection is required or preferred.
    ///
    /// `required` refuses the build when no usable TPM or Secure Enclave is
    /// present, rather than producing a capsule that appears hardware-protected
    /// but carries a recoverable machine secret.
    #[arg(long, value_name = "POLICY", default_value = "preferred",
          value_parser = ["preferred", "required"])]
    pub hardware: String,
}

#[derive(clap::Args, Debug)]
pub struct BuildArgs {
    /// The capsule project produced by `seal`.
    pub project: std::path::PathBuf,
    /// GitHub user or organisation that owns the build repository.
    pub owner: String,
    /// Build repository name. Created if it does not exist.
    pub repo: String,
    /// Repository visibility. Private is strongly recommended.
    #[arg(default_value = "private", value_parser = ["private", "public"])]
    pub visibility: String,
}

#[derive(clap::Args, Debug)]
pub struct VisibilityArgs {
    pub owner: String,
    pub repo: String,
    #[arg(value_parser = ["private", "public"])]
    pub visibility: String,
}

#[derive(clap::Args, Debug)]
pub struct ExportArgs {
    /// Where to write the record.
    pub output: std::path::PathBuf,
    /// Labels to attach. Metadata, not secrets - but covered by the record's
    /// authentication tag, so they cannot be edited afterwards.
    pub labels: Vec<String>,
}

#[derive(clap::Args, Debug)]
pub struct RunArgs {
    /// The .nyarch capsule to run.
    pub capsule: std::path::PathBuf,
    /// Where to extract. The capsule chooses if omitted.
    pub output: Option<std::path::PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum MachinesAction {
    /// List trusted machines, optionally filtered.
    ///
    /// This machine always appears: a capsule always trusts its creator, so
    /// hiding it would misrepresent what a build will contain.
    List {
        /// Filter by id prefix or label text.
        #[arg(default_value = "")]
        query: String,
        /// Filter by label. Repeatable.
        #[arg(long, value_name = "LABEL")]
        tag: Vec<String>,
        /// Whether a machine needs any selected label or all of them.
        #[arg(long, default_value = "any", value_parser = ["any", "all"])]
        mode: String,
    },
    /// Verify and add a machine record to the registry.
    Add {
        /// The .nyfp record to import. Refused if it fails authentication.
        file: std::path::PathBuf,
    },
    /// Remove a machine from the registry.
    Remove {
        /// Unambiguous id prefix.
        id: String,
    },
    /// Replace a machine's labels.
    Label {
        /// Unambiguous id prefix.
        id: String,
        /// The new labels, replacing any existing ones.
        labels: Vec<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// clap validates the definition itself: conflicting names, bad defaults,
    /// and invalid value parsers are caught here rather than at runtime.
    #[test]
    fn the_command_definition_is_valid() {
        Cli::command().debug_assert();
    }

    /// `--with-logs` is available on every subcommand, and off unless asked.
    #[test]
    fn verbose_logging_is_opt_in_and_global() {
        let plain = Cli::try_parse_from(["nyedarch", "bench"]).expect("parses");
        assert!(!plain.with_logs, "verbose output must be opt-in");

        for args in [
            vec!["nyedarch", "--with-logs", "bench"],
            vec!["nyedarch", "seal", "i", "o", "p", "--with-logs"],
            vec!["nyedarch", "machines", "list", "--with-logs"],
        ] {
            let cli = Cli::try_parse_from(&args).expect("parses");
            assert!(cli.with_logs, "{args:?} should enable verbose logging");
        }
    }

    #[test]
    fn seal_parses_a_full_invocation() {
        let cli = Cli::try_parse_from([
            "nyedarch", "seal", "in", "out", "pw",
            "--location", "150", "--time", "14:00", "--one-shot",
            "--trust-tag", "HR", "--tag-mode", "all",
            "--compression", "maximum", "--hardware", "required",
        ])
        .expect("should parse");
        match cli.command {
            Command::Seal(a) => {
                assert_eq!(a.location, Some(150));
                assert_eq!(a.time, vec!["14:00"]);
                assert!(a.one_shot);
                assert_eq!(a.tag_mode, "all");
                assert_eq!(a.compression, "maximum");
                assert_eq!(a.hardware, "required");
            }
            other => panic!("wrong subcommand: {other:?}"),
        }
    }

    /// A typo must not silently select the looser option. `--tag-mode` and
    /// `--hardware` both have a safe and a permissive value, and guessing wrong
    /// would either trust more machines or drop a protection.
    #[test]
    fn invalid_values_are_rejected_not_defaulted() {
        for args in [
            vec!["nyedarch", "seal", "i", "o", "p", "--tag-mode", "either"],
            vec!["nyedarch", "seal", "i", "o", "p", "--hardware", "optional"],
            vec!["nyedarch", "seal", "i", "o", "p", "--compression", "extreme"],
            vec!["nyedarch", "build", "p", "o", "r", "sortof-private"],
        ] {
            assert!(
                Cli::try_parse_from(&args).is_err(),
                "{args:?} should have been rejected"
            );
        }
    }

    #[test]
    fn multiple_time_slots_and_repeated_tags_are_accepted() {
        let cli = Cli::try_parse_from([
            "nyedarch", "seal", "i", "o", "p",
            "--time", "02:00,14:00",
            "--trust-tag", "HR", "--trust-tag", "Finance",
        ])
        .expect("should parse");
        match cli.command {
            Command::Seal(a) => {
                assert_eq!(a.time, vec!["02:00", "14:00"]);
                assert_eq!(a.trust_tag.len(), 2);
            }
            other => panic!("wrong subcommand: {other:?}"),
        }
    }

    /// The passphrase may come from the environment instead of the process
    /// list. The help text promises this, so the parser must allow it - a help
    /// text that describes behaviour the parser lacks is a lie in a security
    /// tool, and this one was exactly that until an audit caught it.
    #[test]
    fn the_passphrase_may_be_omitted_for_the_environment() {
        let cli = Cli::try_parse_from(["nyedarch", "seal", "in", "out"]).expect("should parse");
        match cli.command {
            Command::Seal(a) => assert!(a.passphrase.is_none()),
            other => panic!("wrong subcommand: {other:?}"),
        }
    }

    #[test]
    fn machines_subcommands_parse() {
        let cli = Cli::try_parse_from([
            "nyedarch", "machines", "list", "bangalore", "--tag", "HR", "--mode", "all",
        ])
        .expect("should parse");
        match cli.command {
            Command::Machines { action: MachinesAction::List { query, tag, mode } } => {
                assert_eq!(query, "bangalore");
                assert_eq!(tag, vec!["HR"]);
                assert_eq!(mode, "all");
            }
            other => panic!("wrong subcommand: {other:?}"),
        }
    }

    /// Running with no arguments must show help rather than doing something.
    #[test]
    fn bare_invocation_shows_help() {
        let err = Cli::try_parse_from(["nyedarch"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand);
    }

    /// Help must explain consequences. These phrases are the ones an operator
    /// needs before choosing an option, not decoration.
    #[test]
    fn help_explains_consequences_not_just_syntax() {
        let mut cmd = Cli::command();
        let help = cmd.render_long_help().to_string();
        assert!(help.contains("cannot be switched off") || help.contains("always required"));

        let seal = Cli::command()
            .get_subcommands()
            .find(|c| c.get_name() == "seal")
            .cloned()
            .expect("seal exists");
        let seal_help = seal.clone().render_long_help().to_string();
        assert!(seal_help.contains("Best effort"), "one-shot must state its limits");
        assert!(seal_help.contains("refused"), "location accuracy must state it refuses");
        assert!(seal_help.contains("never from an IP lookup"));
    }
}
