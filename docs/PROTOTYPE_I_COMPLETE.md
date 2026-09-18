# Prototype-I — COMPLETE

**Status: feature-complete and verified. Implementation is closed.**

From here Prototype-I changes only for cosmetic and interface work. Anything
touching the security model, the package format, the key hierarchy or the
authorization flow belongs to Prototype-II.

---

## What Prototype-I is

A working implementation of the idea in `PHILOSOPHY.md`: data that protects
itself once it leaves every system built to protect it. A capsule is a
self-contained executable carrying its payload, the rules for opening it, and
the logic to enforce them.

## The central invariant, and how it is enforced

> The payload key is composed from a contribution by every enabled protection.
> A missing contribution cannot be synthesised by patching a branch, because
> there is no branch — there is missing key material.

Verified by experiment, not assertion: the authorization check was deleted from
the source and the capsule rebuilt. It failed *earlier* than the deleted check
and for a different reason. See `CASE_STUDIES.md` CS-01.

## Delivered

| Area | State |
|---|---|
| Protections | Machine and passphrase mandatory; location and time optional. AND across enabled, OR across trusted machines |
| Cryptography | XChaCha20-Poly1305, Argon2id, HKDF-SHA-512, domain-separated composition, versioned formats |
| Package | Streaming, chunked, authenticated; whole header bound into every chunk; attacker-controlled lengths bounded before allocation |
| Fingerprinting | Adaptive per platform; salted digests only; Strong on Linux and macOS, Medium on Windows |
| Location | Native provider, then a browser consent flow. Never typed in, never an IP lookup, poor accuracy refused |
| Time | Recurring daily windows, day-invariant, behind an interface a server clock can replace |
| Destruction | Three redundant layers; survives the helper being killed; every outcome reported, never assumed |
| Anti-RE | Constant-shape authorization, tamper accumulator, anti-hooking, stripped and LTO'd everywhere |
| Remote build | Encrypted source transport, sealed secrets, provenance-checked artifacts, artifact deleted after retrieval |
| Clients | CLI and desktop at full parity through one shared pipeline |
| Key storage | Platform keystore with a disclosed fallback; lock-serialised initialisation |

## Verified on real hardware

Not merely compiled — executed on Linux, macOS and Windows runners: the full
suite, fingerprint capture, seal → build → run → byte-identical extraction,
denial without output, client key stability, build scripts, installer, and the
macOS `.app` bundle.

**the automated test suite, zero failures, zero warnings.**

## What is deliberately not here

| Not implemented | Why |
|---|---|
| Licence server, accounts, MFA, revocation, telemetry | Prototype-II. Several of these have nowhere to live inside a capsule running alone on a hostile machine |
| TPM / Secure Enclave key operations | No runner has usable secure hardware. Capability detection and fail-closed policy are implemented |
| macOS location prompt | Needs a Mac with a person present to grant permission |
| Control-flow flattening | Needs a different toolchain; a change of scope, not a workaround |

## Known weaknesses, carried forward honestly

1. **`S_machine` is extractable from a held capsule.** The machine protection
   raises effort; it is not a standalone boundary. Hardware-backed identity is
   the designated correction.
2. **Local clock.** Whoever holds the machine can set it. The time protection is
   a recurring policy control, not a tamper-proof expiry.
3. **Location and time are low-entropy policy factors**, never entropy sources.
4. **Destruction is best effort.** It cannot promise physical erasure and never
   reaches a copy made beforehand.
5. **Plaintext must exist at extraction**, on a machine the attacker may control.

None of these is hidden, and none is described as solved.

## The standard this was held to

- Fail closed, always.
- No silent degradation — a weaker substitution is visible to the user.
- Cryptography is the boundary; anti-tamper raises cost and never carries the
  guarantee.
- Claims match reality, including the inconvenient ones.
- Legality is a requirement, not a filter applied afterwards.

## Next

1. Interface refactors from testing.
2. Prototype-I marked **do not touch**.
3. Prototype-II begins: the licence server, and the guarantees that need one.
