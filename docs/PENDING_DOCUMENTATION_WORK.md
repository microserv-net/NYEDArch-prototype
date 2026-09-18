> **Prototype-I is complete.** Everything still listed here is either
> Prototype-II design work or blocked on hardware nobody in this project
> currently has. Nothing below is required for Prototype-I.

# Pending documentation work — TO BE DONE

> **Prototype-I is complete** (`PROTOTYPE_I_COMPLETE.md`). Everything remaining
> below is Prototype-II documentation or work that needs hardware or a person,
> and none of it blocks Prototype-I.

Raised by two context documents: the **dossier update specification** (context 5)
and the **zero-day / third-clock** note.

**Status of everything below: TO BE DONE.** It is recorded here so nothing is
lost while implementation work on Prototype-I continues. None of it is started.

Two rules apply to every item:

1. **The License Server is not being built.** Everything relating to it is
   design and documentation only, and must carry the label
   **Prototype-II — yet to be developed** wherever it appears.
2. **Nothing may be presented as implemented that is not.** Status labels are
   mandatory: `IMPLEMENTED AND TESTED`, `IMPLEMENTED BUT NOT FULLY VERIFIED`,
   `DESIGNED — NOT IMPLEMENTED`, `PROPOSED EXPERIMENT`, `KNOWN LIMITATION`,
   `OPEN QUESTION`.

---

## A. Dossier corrections

| # | Item | Notes |
|---|---|---|
| A1 | ~~**Do not undersell destructive failure**~~ **DONE** — Threat Model "Denial behaviour", Adversarial Review "Failure becomes loss", Anti-RE §7a | A security-critical failure can destroy the capsule. Reflect in Threat Model, Denial Behaviour, Adversarial Review, Anti-Tamper, Known Limitations, Red-Team, Prototype-II sections. The attacker model must state: *a failed hostile experiment may consume the attacker's specimen.* |
| A2 | ~~**Precision about deletion**~~ **DONE** — Threat Model "Five distinct states, never conflated" | Separate five distinct things: deletion requested; deletion reported complete; working data removed; OS/filesystem limits that prevent guarantees; remote revocation making a surviving capsule unusable. Never claim physical erasure the implementation cannot deliver. |
| A3 | ~~**Label every License Server mention**~~ **DONE** — woven into Threat Model, Security Architecture and Anti-RE as labelled future-integration sections | `Prototype-II — yet to be developed`, consistently and visibly. |
| A4 | **Remove the first-use licence-key + TOTP workflow** | It is *not* a settled decision and must not be documented as one. Retain only: account-level MFA is mandatory for License Server accounts. Do not invent an enrolment protocol. |
| A5 | **One licence key per user** | Renewal extends the existing relationship; it never creates a new identity. A capsule stays bound to the licence identity that created it. |
| A6 | **Verified capsule recovery after permanent revocation** | Prototype-II. Narrow, auditable, deliberately inconvenient, one unlock per verified capsule, never a master override. |
| A7 | **Land Mine** | Prototype-II. A post-compromise defensive control, *not* retaliation. Document its limits: cannot recall extracted plaintext, cannot erase copies, cannot see an attacker who never contacts the server. |
| A8 | **Final architecture diagram** | The intended licensed system, showing local vs server operations, cryptographic dependencies, destructive-failure paths, telemetry paths, and recovery paths as distinct. Not a generic flowchart. |
| A9 | ~~**Structured test-case format**~~ **DONE** — `CASE_STUDIES.md` Part 1 | 18 fields: Test ID, Category, Scenario, Setup, Preconditions, Action, Intended, Acceptance, Observed, Artifact State, Telemetry, Notification, Plaintext Exposure, Timing, Resources, Platform, Evidence, Status, Interpretation. |
| A10 | **Metrics and graphs** | Only where real measurements exist. No decorative charts, no manufactured numbers. |
| A11 | **Post-compromise lifecycle** | Prototype-II. Keep detection, notification, audit preservation, revocation and lawful investigation as separate concepts. State plainly: extracted plaintext cannot be recalled. |
| A12 | **Hostile to threats, humane to legitimate users** | Extreme restriction for suspicious activity; extreme specificity for verified recovery. |
| A13 | **Preserve the central philosophy** | *From software that protects data, to software that lets data protect itself.* The expanded security material must not turn NYEDArch into conventional DRM. |
| A14 | **Document-wide consistency audit** | Test counts and their arithmetic, weakness counts, section numbering, cross-references, status labels, terminology, benchmark figures. No contradictions about what is implemented. |

## B. Test and research programmes

| # | Item | Notes |
|---|---|---|
| B1 | ~~**Adversarial secure-deletion programme**~~ **DONE** — see `SECURE_DELETION.md` | ~24 real cases: file open during deletion, read-only filesystem, permission failure, locking, concurrent readers and writers, rename-before-delete, swap races, partial write, interruption, crash during destruction, copied and renamed capsules, surviving temporary files, platform and filesystem behaviour. For each: was deletion requested, did it succeed, did working material survive, did the capsule remain usable, was failure detectable. Separate **verified** from **best-effort** from **irreducible OS limits**. |
| B2 | ~~**Attack laboratory**~~ **DONE** — see `ATTACK_LABORATORY.md`, 13 executed attacks | Expand the branch-patching experiment into a catalogue: policy flags, runtime binding, transplantation, substitution, manifest tampering, chunk reorder and truncation, ciphertext and header modification, embedded constants, fingerprint records, machine identity, location provider, time source, environment detection, instrumentation, memory, hooks, syscalls, abnormal termination, virtualization, dependency substitution. Executed attacks carry evidence; unexecuted ones are marked **PROPOSED EXPERIMENT**. |
| B3 | ~~**Red-team attacker case study**~~ **DONE** — `CASE_STUDIES.md` Part 2, incl. Race Against Time and Failure Becomes Loss, labelled as modelling | Model seven attacker classes against a *final capsule only* — no source, no Builder, no server. Include the framings **Race Against Time** and **Failure Becomes Loss** (self-destruction as an anti-iteration mechanism, not an impossibility claim), tooling categories at a high level, and attacker psychology **explicitly labelled as analytical modelling, not observed testimony**. |

## C. The third clock — vulnerability lifecycle

From the zero-day note. NYEDArch's own clock starts at launch, and it is distinct
from the attacker's and the defender's.

| # | Item | Notes |
|---|---|---|
| C1 | **Vulnerability Lifecycle, Disclosure and Capsule Migration** | Discovery → triage → severity → patch → server-side vulnerability marking → owner notification → minimum runtime version enforcement → migration and re-capsulation → old artifact revocation → completion. Prototype-II for the enforcement half. |
| C2 | **Security Update Adoption Gap** | A first-class limitation, stated plainly: *a patched runtime does not retroactively repair already-issued capsules.* Until vulnerable artifacts are migrated or revoked they may remain exposed. |
| C3 | **Version state machine** | An issued capsule moves `VALID → SECURITY UPDATE REQUIRED → REVOKED/BLOCKED`, rather than staying valid forever because it was valid when issued. Requires runtime version, package format, crypto version, policy version, known-vulnerable status, minimum permitted version, revocation state. |
| C4 | **Land Mine as vulnerability containment** | Not only a post-theft control: a known-vulnerable capsule can be forced into migration rather than left silently exploitable. |
| C5 | **Migration cost is a real risk** | 500 capsules issued six months ago are not repaired by updating the Builder. If re-capsulation is expensive the burden scales with the estate, and public disclosure before migration completes hands the attacker a window. |
| C6 | **Vulnerability research function** | Continuous adversarial research, responsible disclosure handling, emergency patching, staged rollout, telemetry on vulnerable versions, enforced migration where risk warrants it. |

**Why C matters for Prototype-I now:** a standalone capsule has *no* way to say
"the artifact you made six months ago is known-vulnerable, stop using it". That
is not a gap the current prototype can close — but the version fields that make
it possible later must exist in the package format from the start, or every
capsule issued before Prototype-II is permanently outside the scheme. Package
format, crypto version and runtime commitment are already recorded per capsule;
**that is the seam C3 will need**, and it must not be removed.

---

## D. Implementation work — the one item here that is *not* documentation

**D1 — Hardware-backed machine protection (TPM / Secure Enclave).**

Context 5 §14 names this the **first engineering task**, ahead of other
implementation changes. It is the correction for a weakness already recorded in
the dossier:

> the per-machine `S_machine` secret is extractable from a held capsule and is
> therefore not a standalone confidentiality boundary.

Required: bind the machine factor to hardware-protected material rather than a
secret stored inside the package; fail closed where the hardware-backed path is
mandatory; never let the software-only fallback become a silent downgrade.

Must document: which hardware root is trusted, which secret is hardware-protected,
what operation is permitted, whether it is exportable, and what happens on
hardware replacement, unavailable hardware, virtual machines, unsupported
hardware, and secure-storage reset — plus the recovery path.

Must not claim TPM or Secure Enclave makes machine identity unspoofable.

Tests required: successful hardware-backed authorization, wrong machine,
transplanted package, hardware replacement, unavailable TPM/Secure Enclave,
permission failure, virtualization, secure-storage reset, extraction attempts,
fallback prevention, cross-platform behaviour.

Anything not implemented stays **DESIGNED — NOT IMPLEMENTED**.


---

## Open items carried forward

| Item | State |
|---|---|
| ~~Unauthenticated artifact byte on Windows~~ | **Resolved — it was a test defect.** `a11` set bytes to 0xFF without checking the value changed; offset 148 already held 0xFF on Windows. An exhaustive sweep (A12) confirms no unauthenticated byte exists |
| ~~Windows DPAPI key storage~~ | **Re-implemented** via .NET `ProtectedData`, the primitive under the cmdlets that failed. The secret crosses on stdin, never argv. `master_key` still verifies the read-back, so an unreliable store falls through to the file rather than losing records |
| ~~Windows destruction layer 2~~ | **Fixed** — engages as `a self-removing scheduled task and a detached PowerShell` |
| macOS location permission prompt | Needs a Mac with a user present |
| TPM / Secure Enclave key operations | No runner has usable secure hardware; detection and fail-closed policy are done |
| Control-flow flattening | Needs a different toolchain; a change of scope, not a workaround |
