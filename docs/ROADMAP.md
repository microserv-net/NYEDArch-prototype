# NYEDArch Roadmap, Deviations & FUTURE markers

## Phase order (per spec §77 / execution §44)

1. Architecture + threat model — **done** (`docs/`).
2. Crypto/package specification — crypto **done** (`CRYPTO_FORMAT.md` + code); package format next.
3. Workspace + module boundaries — crypto crate live; siblings scaffolded next.
4. Core package/encryption/fingerprint — next.
5. Runtime generation (protected template).
6. GUI (original design).
7. GitHub orchestration.
8. Cross-platform.
9. Hardening (anti-tamper / anti-RE) + `ANTI_RE_ANALYSIS.md`.
10. Adversarial review.
11. Performance benchmarking.
12. Release docs.

## Deviations from the initial specification (with rationale — §1/§74)

- **ADR-0001 — XChaCha20-Poly1305 for the payload AEAD.** Spec left AEAD choice
  open; chose XChaCha over AES-GCM for random-nonce safety and no hardware-AES
  dependency in a hostile-runtime binary. Externally visible behavior unchanged;
  security equal-or-stronger. No approval needed (implementation detail).
- **ADR-0002 — Rust 1.75 build pins.** The offline sandbox toolchain forced
  pinning `base64ct`, `zeroize`/`zeroize_derive`, `cfg-if` below their
  edition2024 releases. On latest stable (the intended CI toolchain, §40) these
  pins relax. Implementation detail; documented.
- **ADR-0003 — Location quantization = documented grid now, H3/S2 later.** Spec
  permits "another defensible spatial quantization system." Implemented a
  deterministic cos-lat-scaled grid with accuracy rejection; production target
  is H3/S2, swappable behind `geo::quantize`. Boundary-sensitivity limitation
  documented rather than hidden.

None of these change NYEDArch's security semantics or mandatory factors, so none
required user approval. Any future change that alters externally observable
security behavior will be raised for approval before implementation.

## FUTURE — LICENSE SERVER (explicitly not implemented)

Seams exist where useful (e.g. `timewin::TimeSource` for authenticated time).
Not implemented, not simulated: license keys, authenticated time, 2FA, remote
authorization, hostile-system unlock, unlock notifications, login-attempt
tracking, forensic audit, recovery mechanisms. See `FUTURE_LICENSE_SERVER.md`
(to be expanded) for the integration contract.

## Remaining spec documents to author as their phases land

`FINGERPRINT_SPEC.md` (Phase 4), `ANTI_RE_ANALYSIS.md` (Phase 9),
`GITHUB_SECURITY.md` (Phase 7), `EULA.md` (Phase 4 UI gating),
`FUTURE_LICENSE_SERVER.md`, `DEVELOPMENT.md`.

---

## Phase completion (final)

| Phase | Status | Evidence |
|---|---|---|
| 1 Architecture & threat model | complete | `ARCHITECTURE.md`, `THREAT_MODEL.md` |
| 2 Crypto/package specification | complete | `CRYPTO_FORMAT.md`, `KEY_HIERARCHY.md` |
| 3 Workspace & boundaries | complete | 8 crates, enforced feature separation |
| 4 Package / encryption / fingerprint | complete | round-trip + fingerprint tests |
| 5 Runtime generation | complete | capsule compiled and executed |
| 6 Desktop client | complete | Full parity with the CLI; licence gate, real sealing, remote build, launcher |
| 7 GitHub orchestration | complete | transport verified against the live API |
| 8 Cross-platform | partial | Linux exercised; macOS/Windows paths are real cfg-gated code |
| 9 Hardening | complete | `ANTI_RE_ANALYSIS.md`, tamper accumulator |
| 10 Adversarial review | complete | `ADVERSARIAL_REVIEW.md`; 6 defects found and fixed |
| 11 Performance | complete | measured; see below |
| 12 Documentation | complete | 14 documents, audited against implementation |

### Measured performance (release, this machine)

| Operation | Result |
|---|---|
| Fingerprint capture | ~70 µs |
| Argon2id 64 MiB t=2 | ~96 ms |
| Argon2id 256 MiB t=3 | ~684 ms |
| Seal throughput (zstd) | ~172 MiB/s |
| Restore throughput (zstd) | ~604 MiB/s |
| Compression ratio (zstd, benchmark data) | ~23.7x |

Argon2id dominates capsule startup **by design** — that is the memory-hard
passphrase cost, not an inefficiency.

### Not implemented (deliberate)

- TPM / Secure Enclave attested identity — the designated fix for `S_machine`
  extractability.
- Authenticated time — `FUTURE — LICENSE SERVER`.
- ~~Constant-time protection evaluation~~ — done; denials are now uniform.
- Constant-time protection evaluation remains outstanding; zstd is now implemented
  and is the default codec (ADR-0005).


---

## Update — parity and hardening pass

| Item | State |
|---|---|
| Sealing pipeline | Extracted to a library; CLI and desktop call the same code |
| Desktop progress | Reports real pipeline stages, on a worker thread |
| Licence gating | Shared record; the desktop client blocks the whole window until accepted |
| Trusted machines | Real `.nyfp` import and export; the sealer re-verifies each record itself |
| Location | Native provider, then browser consent flow over loopback with a single-use token |
| Remote GitHub build | Available from both clients; real libsodium sealed-box secrets |
| Client anti-RE | Stripped + LTO (symbols 2,879 → 1) and a shared anti-analysis check |

### Still open

- macOS CoreLocation is not implemented; the browser flow covers that platform.
- No release-mode desktop binary is shipped from this environment.
- A full authenticated GitHub build has not been observed end to end.
