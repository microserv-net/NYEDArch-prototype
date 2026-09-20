//! Runtime hardening: anti-tamper and anti-analysis (spec §30/§31).
//!
//! ## What this module is, and is not
//!
//! It is **not** the confidentiality boundary. The payload key is composed from
//! authorization factor contributions; an attacker who defeats every check here
//! still cannot derive it. This module exists to raise the cost of *reaching,
//! observing, and iterating on* that boundary.
//!
//! Every technique below is individually bypassable by a competent analyst with
//! sufficient time. That is stated deliberately, per spec §31/§79: the goal is
//! layered cost, not an impossibility claim.
//!
//! ## An honest split: what can bind cryptographically, and what cannot
//!
//! It is tempting to claim that anti-debug results are mixed into key
//! derivation, so that patching a check yields the wrong key. **That claim would
//! be false, and the design would be broken.** Key material must be exactly
//! reproducible on every legitimate run; debugger state, timing, and loaded
//! libraries are not reproducible. A key derived from them would deny honest
//! users on a loaded laptop and would have to be "corrected" by a fallback —
//! which is precisely the silent downgrade the specification forbids (§36).
//!
//! So this module separates two categories, and does not blur them:
//!
//! **(a) Deterministic commitments — may participate in cryptography.**
//! The embedded package bytes and the per-build diversifier are identical on
//! every run, so they can be, and are, bound into the derivation context and
//! AEAD associated data.
//!
//! **(b) Environment observations — defense-in-depth only.**
//! Debugger presence, timing anomalies, and injection markers are folded into a
//! tamper accumulator used for control-flow diversification and diagnostics.
//! They are deliberately **not** key material, and are **not** consumed as a
//! single `if detected { exit() }` gate either (§31), since one patched branch
//! would defeat that. They raise analysis cost. They do not, and are not
//! claimed to, prevent extraction by an attacker who defeats them.
//!
//! The real bypass-resistance lives in `nyedarch_crypto::compose`: no accumulator
//! value, patched or genuine, produces the payload key without the actual
//! authorization factors.

use core::sync::atomic::{AtomicU64, Ordering};

/// Per-build diversifier, baked in by the generator so two capsules never share
/// hardening state layout or constants.
#[derive(Clone, Copy)]
pub struct Diversifier(pub u64);

/// Accumulates environment observations. Redundant and independent, so that
/// neutralising one probe does not zero the whole accumulator (spec §30:
/// assume the attacker patches a single check).
pub struct TamperAccumulator {
    state: AtomicU64,
}

impl TamperAccumulator {
    pub fn new(d: Diversifier) -> Self {
        Self { state: AtomicU64::new(d.0 ^ 0x9E37_79B9_7F4A_7C15) }
    }

    /// Mix an observation. Non-commutative-ish mixing so ordering matters and a
    /// replayed partial sequence does not reproduce the value.
    fn mix(&self, tag: u64, observed: u64) {
        let mut s = self.state.load(Ordering::Relaxed);
        s ^= tag.rotate_left((observed & 63) as u32);
        s = s.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(31);
        s ^= observed;
        self.state.store(s, Ordering::Relaxed);
    }

    /// Final accumulator value, mixed into key-derivation context.
    pub fn finish(&self) -> u64 {
        self.state.load(Ordering::Relaxed)
    }

    /// Run all environment probes. Each probe contributes regardless of its
    /// result — a "clean" observation is as much an input as a "dirty" one, so
    /// there is no branch that can be forced to the good path.
    pub fn observe_environment(&self) {
        self.mix(0xA1, probe_debugger() as u64);
        self.mix(0xB2, probe_ptrace_self() as u64);
        self.mix(0xC3, probe_timing_anomaly() as u64);
        self.mix(0xD4, probe_environment_markers() as u64);
        // Anti-hooking (spec §31): instrumentation loaded into this process,
        // and threads belonging to instrumentation frameworks.
        self.mix(0xE5, probe_injected_modules());
        self.mix(0xF6, probe_instrumentation_threads());
    }
}

/// Debugger presence via the platform's own reporting.
///
/// Limitation: trivially defeated by patching `TracerPid` reads, by a debugger
/// that hides itself, or by running under emulation. Contributes an input, not
/// a verdict.
fn probe_debugger() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if let Some(rest) = line.strip_prefix("TracerPid:") {
                    let pid: u64 = rest.trim().parse().unwrap_or(0);
                    return if pid == 0 { 0 } else { pid };
                }
            }
        }
        0
    }
    #[cfg(not(target_os = "linux"))]
    {
        // Windows: IsDebuggerPresent / CheckRemoteDebuggerPresent.
        // macOS: sysctl KERN_PROC info P_TRACED flag.
        // Both are real platform calls that must be compiled on those systems;
        // no fabricated value is returned here.
        0
    }
}

/// Self-trace occupancy. On Linux a process may be traced only once, so a
/// successful self-attach implies no debugger currently holds the slot.
///
/// Limitation: this is intentionally NOT performed via `unsafe` ptrace here —
/// the crate forbids unsafe code, so we infer from the same status interface.
/// A dedicated build may implement the stronger variant behind an audited
/// unsafe block.
fn probe_ptrace_self() -> u64 {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/self/stat")
            .ok()
            .and_then(|s| s.split_whitespace().nth(2).map(|f| f.as_bytes()[0] as u64))
            .unwrap_or(0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

/// Coarse timing observation across a fixed workload. Single-stepping or heavy
/// instrumentation inflates this dramatically.
///
/// Limitation: noisy on loaded or virtualised hosts, which is exactly why it is
/// quantized coarsely and mixed rather than compared to a threshold — a false
/// positive must not deny a legitimate user outright.
fn probe_timing_anomaly() -> u64 {
    let start = std::time::Instant::now();
    let mut acc: u64 = 0x243F_6A88_85A3_08D3;
    for i in 0..2048u64 {
        acc = acc.wrapping_mul(6364136223846793005).wrapping_add(i);
    }
    let ns = start.elapsed().as_nanos() as u64;
    // Bucket to the nearest power-of-two magnitude: stable across normal
    // machines, wildly different under single-stepping.
    let magnitude = 64 - ns.leading_zeros() as u64;
    magnitude ^ (acc & 0xF)
}

/// Well-known analysis-environment markers.
///
/// Limitation: an attacker who knows this list simply removes the markers.
/// Contributes an input; never a standalone verdict.
/// Look for instrumentation loaded into this process.
///
/// Inline API hooking and dynamic instrumentation have to get code into the
/// address space, and on Linux that is visible in `/proc/self/maps`. Two things
/// are counted: mappings whose names match known instrumentation, and
/// **writable-and-executable** regions, which normal loaded code does not need
/// but trampolines and JIT-based hooking engines do.
///
/// Limitations, stated because they matter: a hooking engine that unmaps itself
/// or renames its mappings evades the name check; a legitimate JIT (a scripting
/// runtime loaded into the host) produces W+X regions of its own; and this is a
/// Linux-specific view. It contributes an observation, never a verdict - the
/// value is folded into the accumulator rather than branched on, so patching a
/// single comparison does not neutralise it.
fn probe_injected_modules() -> u64 {
    #[cfg(target_os = "linux")]
    {
        let Ok(maps) = std::fs::read_to_string("/proc/self/maps") else { return 0 };
        let mut score = 0u64;
        for line in maps.lines() {
            let lower = line.to_ascii_lowercase();
            for marker in ["frida", "gum", "gdb", "ltrace", "valgrind", "pintool", "dynamorio"] {
                if lower.contains(marker) {
                    score += 16;
                }
            }
            // Permissions are the second field, e.g. "rwxp".
            if let Some(perms) = line.split_whitespace().nth(1) {
                if perms.contains('w') && perms.contains('x') {
                    score += 1;
                }
            }
        }
        score
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

/// Dynamic instrumentation frameworks run their own threads, and thread names
/// are readable without any privileged interface.
///
/// Defeated by renaming the thread, which is why it is one input among several.
fn probe_instrumentation_threads() -> u64 {
    #[cfg(target_os = "linux")]
    {
        let Ok(rd) = std::fs::read_dir("/proc/self/task") else { return 0 };
        let mut score = 0u64;
        for entry in rd.flatten() {
            if let Ok(name) = std::fs::read_to_string(entry.path().join("comm")) {
                let n = name.trim().to_ascii_lowercase();
                for marker in ["gum-js", "gmain", "frida", "pool-frida"] {
                    if n.contains(marker) {
                        score += 8;
                    }
                }
            }
        }
        score
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

fn probe_environment_markers() -> u64 {
    let mut n = 0u64;
    for var in ["LD_PRELOAD", "LD_AUDIT", "DYLD_INSERT_LIBRARIES"] {
        if std::env::var_os(var).is_some() {
            n += 1;
        }
    }
    n
}

/// Integrity commitment over the embedded package.
///
/// The runtime recomputes a digest of the package bytes it carries and mixes it
/// into the accumulator. Because the payload AEAD already binds the header
/// context, this is a *cheap early* consistency signal rather than the
/// authoritative check — the authoritative check is AEAD authentication, which
/// cannot be patched away without the key.
pub fn package_commitment(package: &[u8]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in package {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01B3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulator_is_deterministic_per_build_in_a_stable_environment() {
        let a = TamperAccumulator::new(Diversifier(7));
        a.mix(1, 2);
        a.mix(3, 4);
        let b = TamperAccumulator::new(Diversifier(7));
        b.mix(1, 2);
        b.mix(3, 4);
        assert_eq!(a.finish(), b.finish());
    }

    #[test]
    fn ordering_matters() {
        let a = TamperAccumulator::new(Diversifier(7));
        a.mix(1, 2);
        a.mix(3, 4);
        let b = TamperAccumulator::new(Diversifier(7));
        b.mix(3, 4);
        b.mix(1, 2);
        assert_ne!(a.finish(), b.finish(), "replayed out-of-order probes must differ");
    }

    #[test]
    fn different_builds_diverge() {
        let a = TamperAccumulator::new(Diversifier(1));
        let b = TamperAccumulator::new(Diversifier(2));
        a.observe_environment();
        b.observe_environment();
        assert_ne!(a.finish(), b.finish(), "per-build diversification must hold");
    }

    #[test]
    fn package_commitment_detects_modification() {
        let p = b"sealed-package-bytes";
        let mut q = p.to_vec();
        q[3] ^= 0xff;
        assert_ne!(package_commitment(p), package_commitment(&q));
    }
}

#[cfg(test)]
mod hooking_tests {
    use super::*;

    /// The probes must run and return without privileged interfaces.
    #[test]
    fn injection_probes_are_total() {
        let _ = probe_injected_modules();
        let _ = probe_instrumentation_threads();
    }

    /// They must contribute to the accumulator, not sit unused. A probe whose
    /// output is discarded is decoration.
    #[test]
    fn injection_probes_reach_the_accumulator() {
        let d = Diversifier(0xDEAD_BEEF);
        let a = TamperAccumulator::new(d);
        a.mix(0x91, 0);
        let baseline = a.finish();

        let b = TamperAccumulator::new(d);
        b.mix(0x91, 17); // as if instrumentation had been seen
        assert_ne!(baseline, b.finish(), "the observation must change the state");
    }

    /// On a clean Linux process the module probe should not be dominated by
    /// instrumentation markers. W+X regions may legitimately exist, so this
    /// asserts the marker component specifically stays absent.
    #[test]
    #[cfg(target_os = "linux")]
    fn a_clean_process_shows_no_instrumentation_markers() {
        let maps = std::fs::read_to_string("/proc/self/maps").unwrap_or_default();
        let lower = maps.to_ascii_lowercase();
        for marker in ["frida", "dynamorio", "pintool"] {
            assert!(!lower.contains(marker), "unexpected {marker} in this test process");
        }
    }
}

#[cfg(test)]
mod accumulator_use_tests {
    /// The accumulator's value must be *used*.
    ///
    /// It was computed from six probes and then dropped - `let _diversified =
    /// acc.finish();`. Anti-analysis that cannot change anything is the
    /// security theatre the specification forbids (§75), and it is worse than
    /// having none, because it reads as a defence.
    ///
    /// This asserts the value reaches something, and that what it reaches is
    /// diversification rather than a gate: a discarding binding would fail the
    /// first check, and `if diversified` deciding success would fail the second.
    /// Comment lines are stripped before searching.
    ///
    /// The commentary above the fix quotes the old code as an explanation, so a
    /// naive search matched the documentation and failed on correct code - the
    /// same self-matching trap the capsule prompt tests hit.
    fn code_only(src: &str) -> String {
        src.lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_tamper_accumulator_is_not_discarded() {
        let src = code_only(include_str!("lib.rs"));
        let dropped = format!("let _{} = acc.finish()", "diversified");
        assert!(
            !src.contains(&dropped),
            "the accumulator is computed and thrown away"
        );
        assert!(
            src.contains("let diversified = acc.finish();"),
            "the accumulator's value must be bound and used"
        );
        assert!(
            src.contains("decoy_rounds") && src.contains("read_order_flipped"),
            "the value must reach the run's shape"
        );
    }

    /// It must never gate authorization.
    ///
    /// Environment observations are not reproducible - a debugger on a support
    /// call, a VM, a loaded machine - so anything derived from them would
    /// eventually refuse an honest user. That is a worse failure than an
    /// analyst having an easier afternoon.
    #[test]
    fn diversification_never_decides_the_outcome() {
        let src = code_only(include_str!("lib.rs"));
        for forbidden in [
            "if diversified",
            "diversified == ",
            "diversified != ",
        ] {
            assert!(
                !src.contains(forbidden),
                "`{forbidden}` would make an unreproducible signal decide the run"
            );
        }
    }
}
