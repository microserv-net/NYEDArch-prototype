# FUTURE — NYEDArch LICENSE SERVER INTEGRATION

**Not implemented. Not simulated. No placeholder performs server authentication.**
Prototype-I is a standalone, unlicensed variant for internal reference,
architecture development, and demonstration.

The complete future architecture is documented separately in
[`license-server/`](license-server/) — 16 documents covering identity, licensing,
capsule authorization, protocol, audit, monitoring, key management, distribution,
payments, threat model, incident response, privacy, and sequencing.

## Labelled integration model

Each current limitation is recorded as:

```
CURRENT PROTOTYPE BEHAVIOUR  ->  FUTURE INTEGRATION  ->  WHAT IT CHANGES
```

| Current prototype behaviour | Future integration | What the integration changes |
|---|---|---|
| Local system clock, attacker-controlled | Authenticated server time | The time protection stops being satisfiable by resetting the clock. Network delay still bounds accuracy |
| No revocation once distributed | Centralized revocation via key-share withholding | A stolen capsule can be permanently disabled. Already-extracted plaintext still cannot be recalled |
| Local-only authorization | Server contributes a true key share as a fifth factor | A patched response handler yields the wrong key, not access. Capsules require connectivity |
| No account identity or MFA | Mandatory accounts, mandatory MFA, passkeys preferred | Ownership becomes provable and recoverable. Full account compromise still defeats the failsafe |
| No authenticated audit history | Signed, sequenced, hash-chained lifecycle events | Owners see authorization attempts. An event never sent still leaves no trace |
| No recovery after hardware change | Failsafe: licence + MFA + out-of-band approval | Legitimate users regain access. The failsafe becomes the weakest link in the machine protection |
| `S_machine` extractable from a held capsule | Server factor is absent from the binary entirely | Extraction no longer yields every local factor. Hardware attestation remains the real fix |
| No centralized misuse detection | Cross-account risk scoring | Systematic abuse becomes detectable. Detection stays probabilistic |

## Existing extension points in the code

| Capability | Seam today | Prototype behaviour |
|---|---|---|
| Authenticated time | `nyedarch_crypto::timewin::TimeSource` | `LocalClock`; documented as attacker-controlled |
| Passphrase acquisition | `PassphraseProvider` trait | Local prompt |
| Location acquisition | `LocationProvider` + `nyedarch-platform` | Platform service; fails closed when absent |
| Runtime states | Explicit `State` enum | A remote-authorization state can be inserted |

## Dependency to resolve first

The measured ~50x protection-order timing oracle (`ANTI_RE_ANALYSIS.md` §5.1) should be
fixed **before** a network stage is added in front of it, since a server denial
returns before Argon2id runs and would widen the separation.

## Deliberate non-goals for Prototype-I

No persistent forensic or attempt-tracking system is implemented. Ephemeral
diagnostics exist only for legitimate debugging via creator mode.
