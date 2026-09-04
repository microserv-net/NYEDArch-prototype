# ADR-0004 — Architecture correction pass

**Status:** accepted
**Category:** ARCHITECTURE / SECURITY
**Trigger:** "Updates after first stop" — corrections required before further
implementation crates.

## Context

The correction document required eight items to be resolved and documented. Four
were genuine defects in code already written, not documentation gaps. Recording
what changed and why.

## Findings and resolutions

### §1 Key hierarchy — GAP (documentation)
No consolidated key document existed. Added `docs/KEY_HIERARCHY.md` covering
origin, derivation, purpose, persistence, storage, lifetime, rotation,
zeroization, and component access for every key, with the required separation
rule and an access matrix.

### §2 Authorization must be cryptographic — ALREADY HELD, now proven
`compose::derive_payload_key` was already contribution-based. Added an explicit
adversarial demonstration: the binding comparison was patched out of the runtime
and rebuilt; the transplanted capsule still failed closed one state later.
Documented in KEY_HIERARCHY §2.

### §3 Fingerprint bootstrap — ALREADY HELD, now stated honestly
The select-a-secret construction avoids circularity. The document previously
did not state what an attacker *gains* by unsealing the record. Now stated
plainly: they obtain `S_machine` and hashed ids, which is insufficient without
the passphrase factor. Machine authorization is no longer implied to be a
standalone confidentiality boundary.

### §4 Package/runtime binding — DEFECT, fixed
`runtime_binding` was read from the package header only; the runtime never
asserted its own identity, so binding rested on a single mechanism.

Changes:
- `policy_seal` subkey is now `HKDF(salt, K_bootstrap ‖ C_runtime, label)`.
- The generated runtime embeds `RUNTIME_COMMITMENT` and passes **its own**
  constant — never the header value.
- Added an explicit `BindingValidation` state as defense-in-depth.

Transplantation now fails cryptographically even with the check removed.

### §5 Runtime state machine — GAP, fixed
Added `BindingValidation`, `PayloadKeyUnwrap`, and `ExtractionVerification`.
One-shot destruction is now gated behind verified extraction.

### §6 Artifact provenance — GAP, fixed
Added `nyedarch-github::provenance`: `BuildManifest`, `ArtifactClaim`,
`signed_input()` binding build id + target + package/runtime/source commitments +
artifact digest, and `verify_artifact()`. Replay across builds is prevented
because the signed input is build-specific. Three tests.

### §7 Builder/runtime code sharing — DEFECT, fixed
`nyedarch-runtime` depended on `nyedarch-package`, which exported `build.rs`
(filesystem walking, package construction) — builder capability compiled into
every hostile-environment binary. `build` is now behind a non-default `builder`
feature; only the buildtool enables it.

### §8 `nyedarch-crypto` as smallest auditable boundary — DEFECT, fixed
The crate depended unconditionally on `getrandom`, giving the confidentiality
boundary an OS responsibility. Randomness is now the injected `RandomSource`
trait; `getrandom` is behind a default-on `os-rng` feature. The crate builds
with `--no-default-features` and zero platform surface, and deterministic test
vectors are possible via a test-only fixed source.

`#![forbid(unsafe_code)]`, no I/O, and no GUI dependency are preserved.

## Consequences

- Breaking internal API changes: `policy_seal::seal_record`/`open_record` take a
  runtime commitment; `aead::seal_with` / `Capsule::seal_with` take an RNG.
- No externally observable security behaviour was weakened; binding and crate
  isolation are strengthened. No user-facing requirement changed, so no
  requirement-level approval was needed (execution instructions §45).
