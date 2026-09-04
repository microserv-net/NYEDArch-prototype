# Start here — repository index and Prototype-II handoff

This repository is intended to be **self-sufficient**. Someone beginning
Prototype-II with nothing but this link should be able to reconstruct every
decision, every constraint, and every reason without asking a question.

If something material was decided during Prototype-I and is not written down
here, that is a defect in this document.

---

## Read in this order

| # | Document | Why |
|---|---|---|
| 1 | `docs/PHILOSOPHY.md` | **Read first.** Why NYEDArch exists, the chain reaction it aims for, and the legal boundaries that constrain it. Most hard choices only make sense against this |
| 2 | `docs/PROTOTYPE_I_COMPLETE.md` | What was delivered, what was deliberately left out, and the weaknesses carried forward honestly |
| 3 | `docs/context/` | The **original specifications**, unedited. The authoritative statement of intent |
| 4 | `docs/ARCHITECTURE.md`, `docs/SECURITY_ARCHITECTURE.md` | How it is built and why it holds |
| 5 | `docs/COMPONENT_REFERENCE.md` | Every crate and module, feature by feature |
| 6 | `docs/decisions/` | Thirteen ADRs — the reasoning behind every significant change, including the mistakes |
| 7 | `docs/CASE_STUDIES.md`, `docs/ATTACK_LABORATORY.md` | What was attacked, what happened, what it proves |
| 8 | `docs/PENDING_DOCUMENTATION_WORK.md` | Everything still outstanding, and why each item is blocked or deferred |

## The one idea

An ordinary archive is passive: it waits for a program to open it, and once
copied it protects nothing. Every conventional protection is environmental, and
every one of them ends at the moment data leaves that environment.

> **What if the data protected itself instead?**

A capsule is data that has stopped being passive. Everything else is engineering
in service of that.

## The invariant every change must preserve

> The payload key is composed from a contribution by every enabled protection.
> A missing contribution cannot be synthesised by patching a branch, because
> there is no branch — there is missing key material.

This was verified by experiment, not assertion: the authorization check was
removed from the source and the capsule rebuilt. It failed *earlier* than the
deleted check, for a different reason (`docs/CASE_STUDIES.md`, CS-01).

**If a Prototype-II change would let a patched boolean release the payload, the
change is wrong**, however convenient the server makes it.

---

## Repository layout

```
crates/
  nyedarch-crypto/       the confidentiality boundary: AEAD, KDF, composition
  nyedarch-core/         identifiers, policy records, capsule identity, launching
  nyedarch-package/      package format, sealing, restoring, manifests
  nyedarch-fingerprint/  adaptive machine identity, .nyfp records
  nyedarch-platform/     location providers, hardware capability
  nyedarch-runtime/      authorization state machine, hardening, destruction
  nyedarch-github/       build orchestration against an untrusted CI
  nyedarch-buildtool/    the client: pipeline, CLI, registry, keystore
  nyedarch-installer/    per-user installer, macOS .app bundle
gui/nyedarch-gui/        desktop client (separate workspace)
docs/                    architecture, security, specifications, decisions
docs/context/            the original, unedited specifications
docs/license-server/     Prototype-II design, 16 documents — NOT implemented
docs/decisions/          architecture decision records
.github/workflows/       CI, security invariants, release
build.sh / build.bat     produce the distributable release directory
```

## Which crates ship inside a capsule

This is the column that decides attack surface. A capsule carries only what it
needs to authorize and extract, so builder capability cannot be reached by
someone holding an artifact.

**Ships:** `crypto`, `core`, `package` (restore half), `fingerprint`,
`platform` (location), `runtime`.
**Never ships:** `github`, `buildtool`, `installer`, the GUI.

Feature isolation is enforced by CI against a **generated capsule project**, not
the development workspace — cargo unifies features across a workspace, so the
workspace view says nothing useful about the artifact.

---

## For Prototype-II specifically

### What the licence server is for

Not revenue mechanics. Several guarantees have **nowhere to live** inside a
capsule running alone on a hostile machine: trustworthy time, a durable record
of attempts, owner notification, machine blocking, revocation, and evidence held
somewhere that is not the suspect's own computer. A capsule that works offline is
one that cannot be revoked, cannot report, and cannot warn anyone.

Full design: `docs/license-server/` (16 documents). Settled decisions and open
questions are marked there.

### The seam that must not be removed

The package format already records `crypto_version`, the package format version
and a runtime commitment. That is exactly what a future minimum-version floor
needs to move an issued capsule from `VALID → SECURITY UPDATE REQUIRED →
REVOKED`. **Remove those fields and every capsule issued before Prototype-II is
permanently outside the scheme** (`docs/context/04-zero-day-clock.md`).

### Design rules inherited from Prototype-I

1. Fail closed, always. An ambiguous state denies.
2. No silent degradation — a weaker substitution must be visible to the user.
3. Cryptography is the boundary. Anti-tamper raises cost; it never carries the
   guarantee.
4. Claims match reality, including the inconvenient ones.
5. Legality is a requirement, not a filter applied afterwards.
6. Never build anything whose correctness depends on the server being absent.

### Known weaknesses to carry forward

1. `S_machine` is extractable from a held capsule — hardware-backed identity is
   the designated correction.
2. The clock is local, so whoever holds the machine controls it.
3. Location and time are low-entropy **policy** factors, never entropy sources.
4. Destruction is best effort and never reaches a copy made beforehand.
5. Plaintext must exist at extraction, on a machine the attacker may control.

---

## Verification status

**162 automated tests, zero failures, zero warnings.** Executed — not merely
compiled — on Linux, macOS and Windows: the full suite, fingerprint capture,
seal → build → run → byte-identical extraction, denial without output, client key
stability, build scripts, installer, and the macOS bundle.

The remote build was run end to end against real GitHub: encrypted source push,
sealed secrets, capsules produced for all three targets, provenance checked.

### Not verified, and needing a human

- The macOS location permission prompt — needs a Mac with someone present.
- TPM / Secure Enclave key operations — no CI runner has usable secure hardware.
- Artifact download from this network — the storage host was outside the egress
  allowlist, so that path is covered by tests rather than a live run.

---

## Honest notes for whoever continues

Several defects in this project were found only by **executing** things, never by
reading them: a 51 GB allocation from one flipped byte, an unauthenticated header
region, a client key that silently rotated on Windows and invalidated every
signed record, a destruction helper that unlinked without ever scrubbing, and a
build that attached to the wrong CI run.

Two were found in the tests themselves, and are recorded because a test passing
for the wrong reason is worse than no test.

The lesson is written into the ADRs: **run it, then believe it.**
