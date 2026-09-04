# NYEDArch — Final Adversarial Review

Phase 10. Assumption throughout: **the attacker knows everything** about this
architecture and holds the capsule.

Each question is answered with the actual outcome, and where it was tested, the
test is named. Where the answer favours the attacker, it says so.

---

## 1. Can they extract the payload key?

**No — not from the capsule alone.** The key is HKDF output over the
domain-separated contributions of every enabled protection. The capsule contains
`K_bootstrap` and `C_runtime` and nothing else secret.

They must still supply: the passphrase (Argon2id, 64–256 MiB memory-hard) and,
where enabled, the authorized region and time window.
*Tests: `invariant.rs`, `authorization.rs::wrong_passphrase_is_denied_and_leaks_nothing`.*

## 2. Can they bypass fingerprint authorization?

**Partially, and this is the weakest link — stated plainly.** Unsealing the
policy record with the embedded `K_bootstrap`/`C_runtime` yields the
`S_machine` secret for that capsule. So for a capsule they already hold, the
machine protection is defeatable by analysis.

It still forces per-capsule reverse engineering, and it does **not** help against
the passphrase protection. Closing it requires TPM/Secure Enclave sealing so
`S_machine` never exists in extractable form. **Not implemented.**
*See `ANTI_RE_ANALYSIS.md §3`.*

## 3. Can they patch one branch?

**Yes, and it does not help.** Demonstrated: the `BindingValidation` comparison
was removed from the runtime source and rebuilt with a transplanted package.
Execution advanced one state and failed at `BootstrapPolicy`, because the
policy-seal subkey derives from the runtime's own commitment.
*See `KEY_HIERARCHY.md §2`.*

## 4. Can they force manual location or time?

**No.** No manual entry path exists on either side. The builder refuses to seal
a location protection when no provider is present rather than accepting typed
coordinates (verified: the CLI exits with an explanation). The runtime fails
closed when its provider is absent.
*Tests: `location_ward_fails_closed_without_provider`.*

**But:** the time protection reads the local clock, which the attacker controls. A
capsule with only machine+passphrase+time can have its time protection satisfied by
setting the system clock. The time protection is a policy control, not an expiry.
Authenticated time is `FUTURE — LICENSE SERVER`.

## 5. Can they replace the fingerprint list?

**No, not usefully.** The policy record is AEAD-sealed with AAD binding the
header context. Editing it fails authentication. Re-sealing it requires
`K_bootstrap` — and even then, inserting their own fingerprint id would pair it
with a *new* `S_machine` that does not reproduce the payload key.

## 6. Can they replace the runtime, or transplant a payload?

**No.** Runtime A + Package B fails at binding validation, and still fails at
bootstrap with that check removed (§3). Copying the sealed payload elsewhere
fails AEAD authentication because `runtime_binding` is bound into both the HKDF
`info` and the AAD.
*Tests: `transplanted_package_is_denied`, `invariant.rs` transplant case.*

## 7. Can they bypass integrity?

**The advisory layers, yes; the authoritative layer, no.** Environment probes and
the package commitment are defeatable. AEAD authentication is not, without the
key. This is why the probes are explicitly not the boundary.

## 8. Can they recover plaintext from temporary files?

**No temporary plaintext is created.** Decrypted chunks stream directly into
their destination files; nothing is staged. On any failure the partial output
directory is removed. Every negative test asserts the output directory is empty.
*Tests: all 15 in `authorization.rs` via `assert_denied`.*

## 9. Can they exploit logs?

**No passphrase, key, raw coordinate, or raw hardware identifier is ever
logged.** The location protection prints only the tolerance, never coordinates. The
fingerprint prints a truncated opaque digest. Failure output is one generic line.

**Resolved.** This was a real oracle: denial latency was ~2.0 ms for a
binding/machine failure against ~95–105 ms for a passphrase failure, a ~50x gap
readable in a single run, which told an attacker whether they had cleared the
machine protection.

Authorization is now constant-shape - every protection is evaluated, key
derivation always runs, failed protections contribute random decoys, and every
denial fails at authenticated decryption. Measured after the change: ~105 ms
versus ~117 ms, about 1.1x and within noise. Six different denial causes are
asserted by test to produce identical state traces. See
`ANTI_RE_ANALYSIS.md §5.1`.

## 10. Can they exploit GitHub artifacts?

**Not without detection.** The client commits to a build manifest before dispatch
and verifies build id, target, all three commitments, and a Channel-B signature
over the artifact digest. A signature cannot be replayed across builds.
*Tests: 3 provenance tests.*

**Residual:** a fully compromised GitHub account could substitute source and
produce a legitimately signed artifact of attacker-chosen code. Private
repositories and account security are the mitigation.

## 11. Can they extract secrets from the build environment?

**No payload secrets exist there.** The payload is sealed before upload. Actions
secrets are libsodium sealed-box encrypted client-side and never echoed
(asserted by test). GitHub never receives plaintext files, the passphrase, raw
fingerprints, or any payload key.

## 12. Can they reuse a package with another runtime?

Covered by §6 — no.

## 13. Can they interrupt key derivation?

**They can kill the process; that yields nothing.** Derivation is in-memory with
`Zeroizing` buffers and no intermediate persistence. An interrupted run leaves no
partial key and no partial plaintext.

## 14. Can they observe sensitive memory?

**Yes, with live memory access — and this is not claimed otherwise.**
`Zeroizing` minimises secret lifetime but does not defend against a debugger
attached to a running process, swap, hibernation, or core dumps.
*See `ANTI_RE_ANALYSIS.md §6`.*

## 15. Can they forge authorization state?

**Forging the boolean is possible and useless** (§3). Forging the *material* —
32-byte contributions per protection — is the actual requirement, and that is the
cryptographic problem the design reduces to.

---

## Weaknesses found and fixed during this review cycle

| Finding | Resolution |
|---|---|
| `runtime_binding` read only from the header; runtime never asserted its own identity | Policy-seal subkey now binds the runtime's embedded commitment |
| Builder capability (`collect`, `seal_package`) compiled into every capsule | Moved behind a non-default `builder` feature; verified absent from the shipped binary |
| `nyedarch-crypto` had an unconditional OS dependency | Randomness is now injected; crate builds with zero platform surface |
| Time windows embedded the absolute day — a "daily" schedule would have opened on one day only | Made day-invariant; regression test opens a year later |
| Hardening docs claimed anti-debug results fed key derivation | False and unworkable; corrected to separate deterministic commitments from advisory probes |
| No artifact provenance at all | Added build manifest, signed input, verification, replay resistance |

## Outstanding, accepted for the prototype

1. `S_machine` extractability (§2) — needs hardware attestation.
2. Protection-order timing side channel (§9).
3. Local clock trust (§4).
4. Location/time are low-entropy policy factors, never entropy sources.
5. No anti-hooking or control-flow flattening — omitted deliberately rather than
   added as token gestures.


---

## Failure becomes loss

A conventional reverse-engineering workflow values repeated experimentation
because a failed hypothesis costs only time. The artifact is unchanged, and the
next attempt starts immediately.

A one-shot NYEDArch capsule breaks that assumption: destruction runs after a
verified extraction, through three redundant layers, so killing the helper does
not prevent it. The attacker's specimen can be consumed by using it.

**This is an anti-iteration mechanism, not an impossibility claim.** It changes
the economics of analysis; it does not change the mathematics of the
cryptography, and it never reaches a copy made beforehand. Both points are
asserted by test: `a_copy_made_beforehand_survives_and_that_is_expected` exists
precisely so the claim cannot quietly inflate.

## Executed attacks

`ATTACK_LABORATORY.md` records twelve attacks, all executed. The strongest
single statement available from it: **no single-bit edit anywhere in the
artifact produced plaintext.**

It found two defects that review had missed:

1. **An unauthenticated header region.** The AAD covered the key-derivation
   fields but not `compression` or `chunk_size`, so a header byte could be
   changed while the package still opened. Now a commitment over the entire
   header is bound into every chunk.

2. **A length field that could drive a 51 GB allocation.** The chunk count went
   straight to `Vec::with_capacity`, and one flipped byte aborted the process.
   An abort is not a refusal - it is a denial of service that bypasses the
   fail-closed path entirely. Lengths are bounded before any allocation now.

Both were found by executing attacks, not by reading code. That is the argument
for keeping the laboratory rather than the register alone.
