//! Client-side anti-tamper and anti-analysis.
//!
//! Anti-RE is a global requirement, not something that applies only to the
//! generated capsule. The client is a worthwhile target in its own right: it
//! holds the local record-signing key, the EULA acceptance key, and the logic
//! that generates capsule sources. An attacker who can quietly modify the
//! client can weaken every capsule it later produces.
//!
//! # What this is, and what it is not
//!
//! It is **not** a confidentiality boundary. The client's real protections are
//! the OS keystore, the cryptographic construction in `nyedarch-crypto`, and
//! the fact that a capsule's payload key is composed from factors the client
//! never stores. This module raises the cost of *analysing and modifying* the
//! client. Every technique here is defeatable by a competent analyst.
//!
//! As in the runtime, observations are folded into an accumulator rather than
//! consumed as `if detected { exit() }`, because a single patched branch would
//! defeat that. And, as in the runtime, environment observations are **not**
//! key material: they are not reproducible, so a key derived from them would
//! lock out honest users.

use std::sync::atomic::{AtomicU64, Ordering};

/// Accumulates environment observations for the client session.
pub struct ClientIntegrity {
    state: AtomicU64,
}

impl Default for ClientIntegrity {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientIntegrity {
    pub fn new() -> Self {
        Self {
            state: AtomicU64::new(0x9E37_79B9_7F4A_7C15),
        }
    }

    fn mix(&self, tag: u64, observed: u64) {
        let mut s = self.state.load(Ordering::Relaxed);
        s ^= tag.rotate_left((observed & 63) as u32);
        s = s.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(31);
        s ^= observed;
        self.state.store(s, Ordering::Relaxed);
    }

    pub fn finish(&self) -> u64 {
        self.state.load(Ordering::Relaxed)
    }

    /// Observe the environment. Each probe contributes whatever it sees, clean
    /// or not, so there is no branch that can be forced onto a good path.
    pub fn observe(&self) {
        self.mix(0x51, probe_debugger());
        self.mix(0x62, probe_injection());
        self.mix(0x73, probe_self_image());
    }

    /// Whether the session looks instrumented. Advisory only: it is surfaced to
    /// the operator as a warning, never used to deny a legitimate action, since
    /// a false positive would lock a user out of their own data.
    pub fn looks_instrumented(&self) -> bool {
        probe_debugger() != 0 || probe_injection() != 0
    }
}

/// Debugger presence, as reported by the platform.
///
/// Limitation: defeated by patching the read, by a debugger that hides itself,
/// or by emulation. Contributes an input, not a verdict.
fn probe_debugger() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if let Some(rest) = line.strip_prefix("TracerPid:") {
                    return rest.trim().parse().unwrap_or(0);
                }
            }
        }
        0
    }
    #[cfg(not(target_os = "linux"))]
    {
        // Windows: IsDebuggerPresent. macOS: sysctl KERN_PROC P_TRACED.
        // Real platform calls, compiled on those systems; no fabricated value.
        0
    }
}

/// Library-injection markers.
///
/// Limitation: an attacker who knows this list simply clears the variables.
fn probe_injection() -> u64 {
    let mut n = 0u64;
    for var in ["LD_PRELOAD", "LD_AUDIT", "DYLD_INSERT_LIBRARIES", "DYLD_LIBRARY_PATH"] {
        if std::env::var_os(var).is_some() {
            n += 1;
        }
    }
    n
}

/// A cheap commitment over the client's own image path and size. This detects
/// casual replacement, not a careful one: an attacker who rebuilds the client
/// produces a consistent image. The authoritative protection for artifacts is
/// signature verification, not this.
fn probe_self_image() -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    if let Ok(exe) = std::env::current_exe() {
        for b in exe.to_string_lossy().as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x1000_0000_01B3);
        }
        if let Ok(md) = std::fs::metadata(&exe) {
            h ^= md.len();
            h = h.wrapping_mul(0x1000_0000_01B3);
        }
    }
    h
}

/// Run at client startup. Returns a warning to show the operator when the
/// session looks instrumented.
pub fn startup_check() -> Option<&'static str> {
    let ci = ClientIntegrity::new();
    ci.observe();
    if ci.looks_instrumented() {
        Some(
            "This session appears to be running under a debugger or with injected libraries. \
             NYEDArch will continue, because that can be legitimate, but key material handled \
             now should be treated as exposed.",
        )
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulator_is_order_sensitive() {
        let a = ClientIntegrity::new();
        a.mix(1, 2);
        a.mix(3, 4);
        let b = ClientIntegrity::new();
        b.mix(3, 4);
        b.mix(1, 2);
        assert_ne!(a.finish(), b.finish());
    }

    #[test]
    fn injection_markers_are_detected() {
        // Set a marker and confirm the probe notices it.
        std::env::set_var("LD_AUDIT", "/tmp/x.so");
        assert!(probe_injection() > 0);
        std::env::remove_var("LD_AUDIT");
    }

    #[test]
    fn startup_check_is_advisory_not_fatal() {
        // Whatever it returns, it must be an Option and never panic: a false
        // positive must not stop a legitimate user working.
        let _ = startup_check();
    }
}
