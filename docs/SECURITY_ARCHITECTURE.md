# NYEDArch Security Architecture

## Defense in depth (§76)

| Layer | Mechanism | Where |
|------:|-----------|-------|
| 1 | Package confidentiality (AEAD) | `nyedarch-crypto::aead` ✅ |
| 2 | Machine authorization (fingerprint-selected secret) | `compose` + `nyedarch-fingerprint` |
| 3 | Passphrase (Argon2id) | `nyedarch-crypto::kdf` ✅ |
| 4 | Optional location factor | `nyedarch-crypto::geo` ✅ |
| 5 | Optional time factor | `nyedarch-crypto::timewin` ✅ |
| 6 | Runtime integrity | `nyedarch-runtime` (Phase 6) |
| 7 | Anti-tamper (layered) | `nyedarch-runtime` |
| 8 | Anti-analysis / anti-RE | runtime + generator |
| 9 | Memory hygiene (zeroize) | crate-wide ✅ (in core) |
| 10 | Temporary-data minimization | runtime |
| 11 | Best-effort destruction | runtime |

No single layer is expected to be perfect. Layers 1–5 and 9 exist and are tested
in the core today; 6–8, 10–11 are the hostile-runtime layers built in later
phases.

## The one invariant everything protects (exec §7 / §32)

> The attacker must not obtain the payload plaintext merely by bypassing or
> modifying a conditional branch.

This is guaranteed cryptographically (see `CRYPTO_FORMAT.md`), not by
anti-tamper. Anti-tamper/anti-RE only raise the cost of reaching, observing, or
modifying the boundary — they are **not** the boundary (§4).

## Key hierarchy

```
per-build bootstrap key (runtime-diversified, NOT fingerprint-derived)
      │  seals
      ▼
protected fingerprint-authorization record  { fp_id -> factor_secret }
      │  matched fingerprint selects
      ▼
C_machine ─┐
C_passphrase (Argon2id) ─┤
[C_location] ─┤   HKDF-SHA512(salt, IKM, context= ver|policy|pkg_id|runtime_binding)
[C_time] ─────┘                 │
                                ▼
                         K_payload  ──►  XChaCha20-Poly1305(payload, aad=context)
```

## Fail-closed policy (§29/§60)

Security-critical paths use typed errors, avoid `unwrap/expect/panic`, and never
silently downgrade. On any failed assumption the runtime zeroizes sensitive
state, refuses decryption, and exits — optionally self-destructing. Normal-mode
failure surfaces a single generic "authorization failed" so the runtime is not
an authorization oracle (§51); detailed per-gate results exist only in Creator/
diagnostic mode, which never weakens cryptography (§30/§50).

## Documented limitations (§10, §35, §65 — never "unbreakable")

- Software cannot guarantee secure deletion on SSDs/CoW filesystems; the design
  minimizes reliance on physical deletion by keeping data encrypted until
  authorization and zeroizing keys.
- Memory extraction of a live authorized process cannot be fully prevented;
  zeroization and lifetime minimization reduce but do not eliminate exposure.
- Anti-RE raises analysis cost; it is not a confidentiality guarantee.
- Location/time are low-entropy policy gates, not key-strength sources.

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
