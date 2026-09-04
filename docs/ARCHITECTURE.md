# NYEDArch Architecture

NYEDArch is two fundamentally different systems that share cryptographic formats
but almost no code (§2, §29 builder/runtime separation).

## A. NYEDArch Builder (trusted machine)

A Rust desktop application installed on the trusted creator's machine. Responsible
for selection → compression → encryption → fingerprint policy → runtime
generation → GitHub build orchestration → artifact verification.

## B. NYEDArch Runtime (hostile machine)

A separately compiled executable produced *per package*. It is **not** the
desktop client and must not require it. Responsible for integrity → protected
policy bootstrap → fingerprint authorization → passphrase → optional location →
optional time → key derivation → decrypt → decompress → restore → cleanup →
optional self-destruction.

```
                TRUSTED                                HOSTILE
   +-----------------------------+        +------------------------------+
   |        NYEDArch Builder        |        |        NYEDArch Runtime         |
   |  files -> compress -> seal  |  --->  |  verify -> authorize -> open |
   |  policy -> runtime-gen      | GitHub |  -> decompress -> restore    |
   +-----------------------------+ build  +------------------------------+
```

## Crate map (planned; `nyedarch-crypto` exists today)

| Crate | Role | Status |
|-------|------|--------|
| `nyedarch-crypto` | Factor composition, AEAD, KDFs, geo/time factors. No I/O, no `unsafe`. | **Live** |
| `nyedarch-core` | Shared types, versioned headers, error taxonomy. | Next |
| `nyedarch-package` | Versioned package format, streaming compress/encrypt pipeline, manifest. | Next |
| `nyedarch-fingerprint` | Adaptive per-OS fingerprint engine, `.nyfp` signed records. | Next |
| `nyedarch-platform` | Win/macOS/Linux abstractions (keystore, TPM/Enclave probes, location). | Next |
| `nyedarch-runtime` | The generated-runtime library: state machine, anti-tamper, cleanup. | Planned |
| `nyedarch-runtime-generator` | Protected-template assembly, per-build diversification. | Planned |
| `nyedarch-github` | Auth, repo, secrets, workflow dispatch, logs, artifact retrieval. | Planned |
| `nyeda-ui` | Original desktop GUI. | Planned |

`nyedarch-crypto` forbids `unsafe` (`#![forbid(unsafe_code)]`) and has no
dependency on any I/O, GUI, or platform crate — keeping the confidentiality
boundary small and auditable.

## Runtime state machine (§73)

```
INITIALIZING -> INTEGRITY_CHECK -> BOOTSTRAP_POLICY
 -> FINGERPRINT_ACQUISITION -> FINGERPRINT_AUTHORIZATION
 -> PASSPHRASE_ACQUISITION -> [LOCATION_ACQUISITION] -> [TIME_ACQUISITION]
 -> KEY_DERIVATION -> PAYLOAD_AUTHENTICATION -> DECRYPTION
 -> DECOMPRESSION -> EXTRACTION -> CLEANUP -> SUCCESS

any failure -> FAIL_CLOSED -> SENSITIVE_STATE_CLEANUP
            -> [OPTIONAL_SELF_DESTRUCTION] -> EXIT
```

Each transition is explicit; failure never recovers into an authorized state
(§28/§29). The `nyedarch-crypto::compose` layer already enforces the KEY_DERIVATION
guarantees; the surrounding states are implemented in `nyedarch-runtime` (Phase 6).

## GitHub as build environment (§38-§45, §71-§72)

GitHub is treated as an untrusted external *build* environment. One persistent
NYEDArch repository holds many builds, each keyed by a ULID. The client orchestrates
commit/Git-Data operations, workflow dispatch, live logs, and artifact
retrieval, then **verifies** every artifact before trusting it. Two independent
directional keypairs (Client→GitHub, GitHub→Client) are used with hybrid
encryption; repository-side private material lives only in GitHub Actions
secrets, never in Git. Plaintext payload is never uploaded — GitHub compiles the
runtime around already-sealed data. (Design documented here; implementation in
`nyedarch-github`, Phase 7.)

## Thermite relationship

Thermite (`microserv-net/thermite-web`) is a **technical reference only** for
the GitHub build-orchestration concept. NYEDArch does not depend on it, copy its
implementation, or reuse its UI. The GitHub concepts are redesigned for NYEDArch's
security model. NYEDArch's UI is independently invented (§1/§11/§46).
