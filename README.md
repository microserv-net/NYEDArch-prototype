# NYEDArch — Not Your Everyday Archive

Transforms passive data into an active, self-protecting executable capsule that
carries its own protected data, authorization policy, decryption logic, and
hostile-environment defenses.

**Status: Prototype-I COMPLETE.** the full suite passing on Linux, macOS and Windows, plus an end-to-end remote delivery test on every push, zero warnings, on Rust 1.91.1.
Verified on real Linux, macOS and Windows runners. See `docs/PROTOTYPE_I_COMPLETE.md`.

Implementation is closed: from here Prototype-I changes only for interface work.
Anything touching the security model belongs to Prototype-II.
The full pipeline runs end to end: seal → generate runtime → remote build →
`.nyarch` capsule → authorized extraction.

> NYEDArch raises the cost and difficulty of unauthorized extraction. It is **not**
> unbreakable, and this project never claims otherwise. What an attacker can
> recover from a capsule is stated explicitly in `docs/ANTI_RE_ANALYSIS.md §3`.

## The central invariant

The payload key is composed from the contributions of **every enabled
authorization protection**. Patching `authorized = true` cannot synthesize the missing
32-byte contributions, so it cannot produce the key.

This is verified, not asserted: the binding check was patched out of the runtime
and rebuilt with a transplanted package. It passed the dead branch and still
failed closed one state later.

## Protections

| Protection | Status | Behaviour |
|---|---|---|
| Machine | **always on** | Any trusted fingerprint authorizes (OR); creator always included |
| Passphrase | **always on** | Argon2id, per-package salt |
| Location | optional | Auto-acquired; poor accuracy rejected; no manual entry |
| Time | optional | Recurring daily window; runtime reads its own clock |

Enabled protections combine with AND. Any failure denies, with one generic message.

## Capsules are `.nyarch`

One extension on every OS. On Linux and macOS `./capsule.nyarch` runs from any
terminal. **On Windows the shell resolves executables via `PATHEXT`, so typing
`capsule.nyarch` will not run it** — launch it from the client (which uses
`CreateProcess` and works regardless of extension) or add `.NYARCH` to
`PATHEXT`. Double-click needs a file association and is not assumed to work.

Drag a capsule onto the client to run it. The launched capsule is a fully
independent process that performs its **own** authorization — the client grants
it nothing and never handles its passphrase.

## Remote build is part of the flow

Capsules are compiled by GitHub Actions. The client orchestrates repository
setup, sealed repository secrets, workflow push, dispatch, and artifact
retrieval, and verifies artifact provenance against a manifest it committed to
before dispatch.

```bash
export NYEDARCH_GITHUB_TOKEN=...   # read from env, never argv
nyedarch-buildtool build ./capsule_project <owner> <repo> private
```

## Quick start

```bash
cargo test --workspace

# Seal (prompts for EULA acceptance on first run)
cargo run -p nyedarch-buildtool -- seal ./my_docs ./capsule "passphrase"

# Compile the generated capsule and run it
cd ./capsule && sh ./stage-capsule.sh          # produces <name>.nyarch
NYEDARCH_PASSPHRASE="passphrase" ./nyedarch-*.nyarch ./out

# Or launch it via the client (drag-and-drop equivalent)
nyedarch-buildtool run ./nyedarch-*.nyarch ./out

nyedarch-buildtool bench                        # measured performance
```

Add protections: `--time 14:00 --time-tolerance-min 15 --location 150 --one-shot
--trust alice.nyfp`

## What runs here vs. on your machine

| Component | State |
|---|---|
| Crypto, package, fingerprint, runtime, generator, orchestration logic | Built, tested, executed |
| Capsule build + authorized extraction | Verified end to end (debug and release) |
| Desktop client | **Full parity with the CLI**: licence gating, real sealing, protections, machine import/export, native + browser location, remote GitHub build, capsule launching |
| Remote GitHub build | **Verified end to end**: capsules built on real runners for Linux, Windows and macOS |
| Source on GitHub | **Encrypted.** Only a ciphertext blob and a secret-free unlocker are pushed; the key is a repository secret |
| Location | Native on Linux (GeoClue2), Windows (system location service) and macOS (CoreLocation, needs an app bundle), with a browser consent flow as the universal fallback. Never typed in, never an IP lookup |
| Client hardening | Stripped and LTO'd; anti-analysis check in both clients |
| Compression | Automatic, Maximum, Balanced or Fast; build-time effort only |
| Cancellation | A build can be cancelled and leaves nothing partial behind |
| Creator mode | Diagnostics in both clients; grants no authority and skips no check |
| Trusted machines | Shared registry with labels, search, and ANY/ALL tag selection; identical in both clients |
| Destruction | Three redundant layers: in-process, OS one-shot scheduler, detached child. Survives the child being killed; verified at the inode |
| Anti-hooking | Instrumentation mappings, W+X regions and framework threads, folded into the tamper accumulator |
| Hardware-backed machine protection | Capability detection and policy implemented; `--hardware required` refuses rather than downgrading. Key operations designed, not implemented |
| Local key storage | Platform keystore (Keychain / DPAPI / Secret Service); an owner-only file fallback is disclosed, never silent |
| macOS/Windows fingerprint + location | Real cfg-gated code; needs those OSes |
| TPM / Secure Enclave attestation | **Not implemented** — designated strengthening path |

## Documentation

`docs/KEY_HIERARCHY.md` · `ARCHITECTURE.md` · `SECURITY_ARCHITECTURE.md` ·
`THREAT_MODEL.md` · `CRYPTO_FORMAT.md` · `FINGERPRINT_SPEC.md` ·
`ANTI_RE_ANALYSIS.md` · `ADVERSARIAL_REVIEW.md` · `GITHUB_SECURITY.md` ·
`EULA.md` · `FUTURE_LICENSE_SERVER.md` · `DEVELOPMENT.md` · `decisions/`

**Future architecture (design only, nothing implemented):**
`docs/license-server/` — 16 documents covering the planned NYEDArch License
Server: identity and MFA, licensing and entitlements, capsule authorization via
server key share, the capsule↔server protocol, audit, monitoring, key
management, distribution, payments, threat model, incident response, privacy,
and sequencing.

## Both interfaces do the same things

The command-line and desktop clients call the same library, so there is no
second implementation that could drift. Licence gating, sealing, all four
protections, one-shot, trusted machine import and export, native and browser
location, remote GitHub build, capsule launching, and the client anti-analysis
check are available from either.

## Verified on real hardware

Every platform path now runs on a real runner, not just a compiler:

| | Linux | macOS | Windows |
|---|---|---|---|
| Test suite | pass | pass | pass |
| Destruction survives the helper being killed | yes | n/a (confirmed in-process) | yes |
| Fingerprint | Strong | Strong | Medium |
| Seal, build and run a capsule | identical | identical | identical |
| Client key stable across runs | yes | yes | yes |
| Build script and installer | pass | pass | pass |
| `.app` bundle created and signed | — | yes | — |

## Documentation

| Document | Covers |
|---|---|
| `docs/PROTOTYPE_I_COMPLETE.md` | What was delivered, what was not, and the weaknesses carried forward |
| `docs/PHILOSOPHY.md` | Why NYEDArch exists, and the legal boundaries that constrain it |
| `docs/COMPONENT_REFERENCE.md` | Every crate and module, feature by feature |
| `docs/CASE_STUDIES.md` | Structured security test records and the attacker model |
| `docs/ATTACK_LABORATORY.md` | 13 executed attacks against a sealed package |
| `docs/SECURE_DELETION.md` | What destruction does, and what it cannot promise |
| `docs/ANTI_RE_ANALYSIS.md` | Anti-tamper and anti-analysis, with limits stated |
| `docs/PROTOTYPE_I_COMPLETE.md` | What was delivered, what was deliberately left out, and the weaknesses carried forward |
| `docs/CI_TESTING_METHOD.md` | How platform code was verified on hosted runners without publishing source |
| `docs/decisions/` | 13 architecture decision records |

## Known limitations

Read `ANTI_RE_ANALYSIS.md §3` and `§5`. In short: `S_machine` secrets are
extractable from a capsule, so the machine protection is not a standalone
confidentiality boundary; the time protection trusts a local clock; protection-order timing
is a side channel; location/time are low-entropy policy factors, not entropy
sources.
