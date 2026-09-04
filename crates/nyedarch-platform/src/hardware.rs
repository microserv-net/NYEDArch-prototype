//! Hardware-backed machine protection (context 5 §14, spec §9).
//!
//! # The weakness this exists to correct
//!
//! In the software-only design the per-machine factor secret `S_machine` is
//! stored inside the capsule, encrypted under the policy-record key. Anyone who
//! holds the capsule and recovers the bootstrap key recovers `S_machine` too.
//! The machine protection therefore raises effort — it does not stand alone as
//! a confidentiality boundary, and the dossier says so.
//!
//! The correction is to stop putting a recoverable secret in the capsule at
//! all, and instead encrypt each machine's factor secret **to a key that lives
//! inside that machine's secure hardware**:
//!
//! ```text
//! enrolment (once, on the trusted machine)
//!     secure hardware creates a non-exportable keypair
//!     the .nyfp record carries only the PUBLIC half
//!         │
//! build (on the creator's machine)
//!     S_machine is encrypted to that public key
//!     the capsule carries only the ciphertext
//!         │
//! runtime (on the trusted machine)
//!     the secure hardware performs the private-key operation
//!     S_machine is recovered and contributes to the payload key
//! ```
//!
//! An attacker holding the capsule then has ciphertext whose private key never
//! left the trusted machine's hardware. That is a real boundary rather than an
//! effort multiplier.
//!
//! # What this must never claim
//!
//! A TPM or Secure Enclave does not make machine identity unspoofable. It makes
//! the private key non-exportable *by supported interfaces*. It does not defend
//! against someone with the machine, physical attacks on the part, a compromised
//! kernel that can ask the hardware to perform operations, or a virtual TPM
//! whose state can be copied along with the VM image.
//!
//! # Status
//!
//! This module implements **capability detection and policy**, which is what
//! decides whether a build may proceed. The keypair operations per platform are
//! `DESIGNED — NOT IMPLEMENTED` and are tracked in
//! `docs/PENDING_DOCUMENTATION_WORK.md` item D1.
//!
//! Detection and policy come first deliberately: the dangerous failure is not
//! "hardware missing", it is **hardware missing and nobody noticed**. The rule
//! that a software fallback must never become a silent downgrade is enforced
//! here, and can be tested on machines that have no secure hardware at all.

#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::process::Command;

/// What the machine can actually offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwareBacking {
    /// A usable hardware root: TPM 2.0, or an Apple Secure Enclave.
    Available,
    /// Hardware is reported but unusable — present but disabled, not owned, not
    /// ready, or not permitted to this user. Treated as absent, never as
    /// present: a half-initialised TPM protects nothing.
    PresentButUnusable,
    /// No hardware root at all.
    Absent,
    /// A virtual machine. Called out separately because a virtual TPM's state
    /// can usually be copied with the VM image, so it does not carry the
    /// non-exportability property the design relies on.
    Virtualized,
}

impl HardwareBacking {
    /// Whether the hardware-backed path can actually be used.
    pub fn is_usable(self) -> bool {
        self == HardwareBacking::Available
    }

    pub fn describe(self) -> &'static str {
        match self {
            HardwareBacking::Available => "hardware-backed key storage is available",
            HardwareBacking::PresentButUnusable => {
                "secure hardware is present but not usable (disabled, not owned, or not permitted \
                 to this user)"
            }
            HardwareBacking::Absent => "this machine has no usable secure hardware",
            HardwareBacking::Virtualized => {
                "this machine is virtualised; a virtual TPM's state can usually be copied with the \
                 VM image, so it is not treated as a hardware root"
            }
        }
    }
}

/// What the operator asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwarePolicy {
    /// Use hardware when available; otherwise continue, **and say so**.
    Preferred,
    /// Refuse to build unless hardware backing is usable.
    Required,
}

impl HardwarePolicy {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "preferred" | "prefer" | "auto" => Some(Self::Preferred),
            "required" | "require" => Some(Self::Required),
            _ => None,
        }
    }
}

/// The outcome of applying a policy to a machine's capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HardwareDecision {
    /// Proceed using the hardware-backed machine factor.
    UseHardware,
    /// Proceed without it. `warning` must be shown to the operator: a weaker
    /// substitution that nobody is told about is precisely what the
    /// specification forbids.
    UseSoftware { warning: String },
    /// Refuse. The operator required hardware and it is not usable.
    Refuse { reason: String },
}

/// Apply a policy to a detected capability.
///
/// Deliberately a pure function of two inputs, so the rule can be tested
/// exhaustively without any hardware present.
pub fn decide(policy: HardwarePolicy, backing: HardwareBacking) -> HardwareDecision {
    match (policy, backing) {
        (_, HardwareBacking::Available) => HardwareDecision::UseHardware,

        (HardwarePolicy::Required, b) => HardwareDecision::Refuse {
            reason: format!(
                "hardware-backed machine protection was required, but {}.\n\nThe build is refused \
                 rather than falling back, because a capsule built without it would carry a \
                 recoverable machine secret while appearing to have hardware protection.",
                b.describe()
            ),
        },

        (HardwarePolicy::Preferred, b) => HardwareDecision::UseSoftware {
            warning: format!(
                "{}. The machine factor will be protected in software only, so a capsule's machine \
                 secret is recoverable by someone who holds the capsule. Machine protection still \
                 raises effort, but it is not a standalone boundary on this machine.",
                b.describe()
            ),
        },
    }
}

// ------------------------------------------------------------- detection ---

/// Detect what this machine offers.
pub fn detect() -> HardwareBacking {
    if is_virtualized() {
        return HardwareBacking::Virtualized;
    }
    detect_native()
}

#[cfg(target_os = "linux")]
fn detect_native() -> HardwareBacking {
    // A TPM is only useful here if the resource-manager device exists *and*
    // this user may open it. A device node we cannot read is unusable, not
    // available.
    let has_device = std::path::Path::new("/dev/tpmrm0").exists()
        || std::path::Path::new("/dev/tpm0").exists();
    if !has_device {
        return HardwareBacking::Absent;
    }
    let major = std::fs::read_to_string("/sys/class/tpm/tpm0/tpm_version_major")
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok());
    if major != Some(2) {
        // TPM 1.2 lacks the primitives this design needs.
        return HardwareBacking::PresentButUnusable;
    }
    match std::fs::OpenOptions::new().read(true).write(true).open("/dev/tpmrm0") {
        Ok(_) => HardwareBacking::Available,
        Err(_) => HardwareBacking::PresentButUnusable,
    }
}

#[cfg(target_os = "windows")]
fn detect_native() -> HardwareBacking {
    // TpmPresent alone is not enough: a present but un-owned or disabled TPM
    // cannot hold a key, so all three must hold.
    let out = Command::new("powershell")
        .args([
            "-NoProfile", "-NonInteractive", "-Command",
            "try { $t = Get-Tpm; \"$($t.TpmPresent) $($t.TpmReady) $($t.TpmEnabled)\" } catch { 'False False False' }",
        ])
        .output();
    let Ok(out) = out else { return HardwareBacking::Absent };
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    let trues = text.split_whitespace().filter(|w| *w == "true").count();
    match trues {
        3 => HardwareBacking::Available,
        0 => HardwareBacking::Absent,
        _ => HardwareBacking::PresentButUnusable,
    }
}

#[cfg(target_os = "macos")]
fn detect_native() -> HardwareBacking {
    // Apple silicon and T2 Macs carry a Secure Enclave. Rosetta and older Intel
    // Macs without T2 do not.
    let out = Command::new("sysctl").args(["-n", "machdep.cpu.brand_string"]).output();
    let brand = out
        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase())
        .unwrap_or_default();
    if brand.contains("apple") {
        return HardwareBacking::Available;
    }
    let bridge = Command::new("system_profiler")
        .arg("SPiBridgeDataType")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase())
        .unwrap_or_default();
    if bridge.contains("t2") {
        return HardwareBacking::Available;
    }
    HardwareBacking::Absent
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn detect_native() -> HardwareBacking {
    HardwareBacking::Absent
}

/// Best-effort virtualisation detection.
///
/// Not a security control: anything here can be hidden by a hypervisor that
/// wants to. It exists so an operator is told that a virtual TPM is not the
/// same guarantee as a physical one, which is a mistake that is otherwise easy
/// to make.
pub fn is_virtualized() -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Ok(s) = std::fs::read_to_string("/sys/class/dmi/id/product_name") {
            let s = s.to_lowercase();
            for m in ["virtual", "vmware", "kvm", "qemu", "xen", "bochs", "hyper-v"] {
                if s.contains(m) {
                    return true;
                }
            }
        }
        if let Ok(s) = std::fs::read_to_string("/proc/cpuinfo") {
            if s.contains("hypervisor") {
                return true;
            }
        }
        false
    }
    #[cfg(target_os = "macos")]
    {
        let out = Command::new("sysctl").args(["-n", "machdep.cpu.brand_string"]).output();
        let brand = out
            .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase())
            .unwrap_or_default();
        if brand.contains("virtual") {
            return true;
        }
        let model = Command::new("sysctl").args(["-n", "hw.model"]).output();
        let model = model
            .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase())
            .unwrap_or_default();
        model.contains("virtualmac")
    }
    #[cfg(target_os = "windows")]
    {
        let out = Command::new("powershell")
            .args([
                "-NoProfile", "-NonInteractive", "-Command",
                "try { (Get-CimInstance Win32_ComputerSystem).Model } catch { '' }",
            ])
            .output();
        let model = out
            .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase())
            .unwrap_or_default();
        ["virtual", "vmware", "kvm", "hyper-v", "hvm domu"]
            .iter()
            .any(|m| model.contains(m))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point: a machine without usable hardware must never be allowed
    /// to produce a capsule that claims hardware protection.
    #[test]
    fn required_policy_refuses_every_unusable_state() {
        for b in [
            HardwareBacking::Absent,
            HardwareBacking::PresentButUnusable,
            HardwareBacking::Virtualized,
        ] {
            match decide(HardwarePolicy::Required, b) {
                HardwareDecision::Refuse { reason } => {
                    assert!(reason.contains("refused"), "the refusal must explain itself");
                }
                other => panic!("{b:?} must be refused under Required, got {other:?}"),
            }
        }
    }

    /// A fallback is allowed, but it can never be silent.
    #[test]
    fn preferred_policy_falls_back_only_with_a_warning() {
        for b in [
            HardwareBacking::Absent,
            HardwareBacking::PresentButUnusable,
            HardwareBacking::Virtualized,
        ] {
            match decide(HardwarePolicy::Preferred, b) {
                HardwareDecision::UseSoftware { warning } => {
                    assert!(!warning.is_empty());
                    // It must state the actual consequence, not just "no hardware".
                    assert!(
                        warning.contains("recoverable"),
                        "the warning must say what is weaker, not merely that hardware is missing"
                    );
                }
                other => panic!("{b:?} under Preferred should fall back, got {other:?}"),
            }
        }
    }

    #[test]
    fn available_hardware_is_used_under_either_policy() {
        for p in [HardwarePolicy::Preferred, HardwarePolicy::Required] {
            assert_eq!(
                decide(p, HardwareBacking::Available),
                HardwareDecision::UseHardware
            );
        }
    }

    /// A virtual TPM must not be mistaken for a hardware root: its state can
    /// usually be copied with the VM image.
    #[test]
    fn virtualisation_is_not_treated_as_hardware_backing() {
        assert!(!HardwareBacking::Virtualized.is_usable());
        assert!(HardwareBacking::Virtualized.describe().contains("copied"));
    }

    /// Present-but-unusable must not count as available. A half-initialised TPM
    /// protects nothing, and treating it as present would produce exactly the
    /// false assurance this module exists to prevent.
    #[test]
    fn present_but_unusable_is_not_available() {
        assert!(!HardwareBacking::PresentButUnusable.is_usable());
        assert_ne!(HardwareBacking::PresentButUnusable, HardwareBacking::Available);
    }

    #[test]
    fn policy_parses_and_rejects_nonsense() {
        assert_eq!(HardwarePolicy::parse("required"), Some(HardwarePolicy::Required));
        assert_eq!(HardwarePolicy::parse("PREFERRED"), Some(HardwarePolicy::Preferred));
        assert_eq!(HardwarePolicy::parse("optional"), None);
    }

    /// Detection must never panic, whatever the machine looks like.
    #[test]
    fn detection_is_total() {
        let b = detect();
        // Every state is one of the four; the point is that it returned at all.
        assert!(matches!(
            b,
            HardwareBacking::Available
                | HardwareBacking::PresentButUnusable
                | HardwareBacking::Absent
                | HardwareBacking::Virtualized
        ));
    }
}
