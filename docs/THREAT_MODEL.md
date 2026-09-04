# NYEDArch Threat Model

## Trust boundaries

- **Trusted:** the creator's machine running the NYEDArch Builder at build time.
- **Semi-trusted / external:** GitHub as a build environment — assumed capable
  of reading anything committed to it or printed in logs, so it receives no
  plaintext payload and no unnecessary secrets (§43/§72).
- **Hostile:** every environment the generated Runtime executes in (§3).

## Adversary capabilities (assumed, §3 / §6)

The runtime attacker may possess the executable and be able to copy, disassemble,
debug, patch, instrument, hook, snapshot memory, re-run, manipulate system time,
run in a VM, inspect syscalls and generated files, and modify the environment.
**The attacker is assumed to know NYEDArch's full architecture** (Kerckhoffs).

## Assets

1. Payload plaintext (primary).
2. Trusted-fingerprint authorization material.
3. Passphrase and all derived key material.
4. Per-build bootstrap key.
5. Directional keypairs and GitHub secrets.

## Primary security goal

Unauthorized parties cannot recover payload plaintext without simultaneously
satisfying every enabled factor: matching machine fingerprint **AND** passphrase
**AND** (location) **AND** (time). Failure of any required factor yields no
payload key (§5/§14/§25).

## Attack → mitigation (core, implemented today)

| Attack | Mitigation | Evidence |
|--------|-----------|----------|
| Patch `authorized = true` | Key is a function of factor contributions, not gated by a branch | `invariant.rs` (all cases) |
| Supply wrong/no fingerprint | Wrong/absent `C_machine` → wrong key → AEAD fail; missing enabled factor fails closed | `wrong_fingerprint_secret_denies_payload`, `missing_enabled_factor_fails_closed` |
| Brute-force passphrase | Argon2id memory-hard (256 MiB) per guess | `kdf.rs` |
| Downgrade policy to drop a factor | `policy_flags` bound into key `info` + AAD | `attacker_cannot_downgrade_policy_to_drop_a_factor` |
| Transplant payload into another runtime | `runtime_binding`+`package_id` bound into key + AAD | `authorized_open_succeeds_and_binds_runtime` |
| Tamper ciphertext/header | AEAD authentication over ciphertext and AAD | `ciphertext_tamper_is_detected`, `aead_roundtrip_and_aad_tamper` |
| Spoof coarse GPS | Reject reading when accuracy > tolerance | `geo_rejects_poor_accuracy` |
| Replay outside time window | Window id only within tolerance; else fail closed | `time_window_inside_and_outside` |
| Read bootstrap key from binary | Reveals record structure only; still needs passphrase + optional factors for any payload key | `CRYPTO_FORMAT.md` §bootstrap |

## Attacks executed against the artifact

Twelve attacks are executed as tests rather than described — ciphertext
modification, truncation, header modification, policy downgrade, chunk
reordering, wrong key, transplantation, appended data, degenerate inputs, and a
bit-flip sweep across the whole artifact. Register in `ATTACK_LABORATORY.md`.

The sweep found two real defects: an unauthenticated region of the header, and a
length field that could drive a 51 GB allocation and abort the process. Both are
fixed; an abort is not a refusal.

## Attacks addressed in later phases (documented, not yet mitigated in code)

- Static/dynamic RE of the runtime → Phase 9 layered anti-RE (`ANTI_RE_ANALYSIS.md`).
- Runtime binary patching of integrity checks → layered integrity + fail-closed.
- Memory scraping of a live authorized process → zeroization + lifetime minimization (partial mitigation only; see limitations).
- Malicious GitHub artifact substitution → artifact verification against expected package/runtime identity (Phase 7).

## Denial behaviour: a failed experiment can cost the attacker the specimen

The attacker model must account for something conventional reverse engineering
does not face. A normal workflow assumes unlimited retries: a wrong hypothesis
costs time, the artifact is unchanged, and the next attempt begins immediately.

Under NYEDArch that assumption does not hold for a one-shot capsule. Destruction
is attempted after a **verified extraction**, and it runs through three
redundant layers — in-process, an OS one-shot scheduler, and a detached child —
so killing the helper process no longer prevents it. Self-destruction is
therefore an **anti-iteration mechanism**, and it changes the economics of
analysis rather than the mathematics of the cryptography.

Stated precisely, because the distinction matters:

| | |
|---|---|
| What it does | Removes the specimen from the attacker's hands after use, and raises the cost of repeated live experimentation |
| What it does **not** do | Make the payload recoverable-proof. That is the cryptography's job, and it holds whether or not the bytes survive |
| What it cannot reach | Any copy the attacker made beforehand — asserted by test, not assumed |

## Five distinct states, never conflated

Everything about deletion in this project is reported as exactly one of these.
Collapsing them is how a security claim becomes a lie:

1. **Deletion requested** — the capsule asked for it; nobody has watched it.
2. **Deletion reported complete by the application** — the path was checked and
   is gone.
3. **Application-controlled working data removed** — intermediate extraction
   state, reported per file.
4. **OS or storage limits** — SSD wear-levelling, copy-on-write snapshots and
   journals sit below the filesystem interface. No user-space program can speak
   for them.
5. **Remote revocation making a surviving capsule unusable** — the only
   mechanism that works on an artifact already beyond reach, and it does not
   exist in Prototype-I. **Prototype-II — yet to be developed.**

A delegated destruction reports as state 1, never state 2, because the
requesting process exits before the work completes and cannot observe it.

## Non-goals / out of scope

- License server and everything depending on it (authenticated time, 2FA, remote
  unlock, forensic audit, recovery) — `FUTURE — LICENSE SERVER`.
- Protection of a payload after a *legitimately authorized* user has extracted it.
- Guaranteed secure deletion on arbitrary storage media (best-effort only).
- Any claim of unbreakability.

---

## FUTURE — NYEDArch License Server integration

**Prototype-II — yet to be developed.** Nothing in this section exists today.
It is recorded here so each limitation above states what will change, rather
than trailing off into "fixed later".

| Prototype-I behaviour | Future integration | What it changes | What still remains |
|---|---|---|---|
| Time comes from the local clock | Authenticated server time | The clock stops belonging to whoever holds the machine | Network delay and skew still have to be tolerated honestly |
| A capsule authorizes entirely locally | Server participates in key composition as a further factor | Patching a boolean cannot substitute for material the binary never contained | The server must never hold plaintext or the passphrase |
| An issued capsule is valid forever | Central revocation and a minimum-version floor | A known-vulnerable or stolen capsule can be refused | It cannot recall plaintext already extracted |
| Attempts leave no durable record | Authenticated capsule event reporting | An attacker leaves a trace they do not control | Nothing is reported by a capsule that never reaches the server |
| Nobody is told about misuse | Owner notification and anomaly detection | Discovery in days rather than never | Detection is probabilistic, and false positives must be recoverable |
| A lost machine means lost data | Verified, narrowly scoped recovery | A legitimate owner is not punished for hardware failure | It must never become a master unlock |

**Why this cannot be built now, rather than merely not yet:** several of these
guarantees have nowhere to live inside a capsule running alone on a hostile
machine. Anything it writes locally, the attacker can delete; any clock it
reads, the attacker can set. See `PHILOSOPHY.md` §4.

**Design rule this imposes on Prototype-I:** never build anything whose
correctness depends on the server being absent. The package format already
carries `crypto_version`, package format version and a runtime commitment, which
is the seam a future minimum-version floor needs. Removing those fields would
put every capsule issued before Prototype-II permanently outside the scheme.
