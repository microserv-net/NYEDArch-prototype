# NYEDArch License Server — Overview

> **SCOPE: DESIGN DOCUMENTATION ONLY.**
> No part of the License Server is implemented, scaffolded, stubbed, or simulated.
> Prototype-I remains a standalone, unlicensed variant for internal reference,
> architecture development, and demonstration. Nothing in this document set
> changes the current Rust workspace.

## 1. Why this exists

Prototype-I is deliberately standalone, and that choice creates limits no amount
of local engineering can remove, because each requires an authority outside the
capsule:

- the local clock is attacker-controlled;
- a distributed capsule cannot be revoked;
- there is no account identity, and therefore no MFA and no recovery;
- authorization attempts are invisible to the data owner;
- a legitimate user whose hardware changed is locked out permanently;
- misuse patterns spanning many capsules cannot be detected from inside one.

The License Server is that authority.

## 2. Settled product decisions

These are fixed inputs to the design, not options under discussion.

| Decision | Ruling |
|---|---|
| Connectivity | **Mandatory.** Builder and capsule both fail closed offline. No silent fallback, no grace period |
| Server contribution to the payload key | **A true key share** — not a signed boolean, not gated policy |
| Licence expiry behaviour | **Capsules stop opening.** Entitlement is continuously enforced |
| Free tier | **None.** Terms are 3, 6, and 12 months, renewable |
| MFA | **Mandatory** before an account is operational; passkeys/WebAuthn preferred, TOTP supported |
| Licence key display | **Once only.** Never retrievable in plaintext afterwards |
| Revocation | **Permanent.** Remaining term may transfer to a replacement depending on cause |
| Repeat restrictions | **Exponential escalation** (e.g. 1 week → 1 month → …) |
| Base tier protections | **Machine + passphrase only.** Time and location are higher tiers |
| Telemetry retention | **User-configurable, bounded per tier** |
| Failsafe approval | Individual: account owner. Enterprise: team admin + optional additional reviewers |

## 3. What the server must never be

- It must **never** hold plaintext payloads.
- It must **never** hold user passphrases or passphrase-equivalent material.
- It must **never** be able to open a capsule by itself.

The Prototype-I confidentiality model is preserved and extended, not replaced.
The server adds a factor; it does not become the factor.

## 4. The load-bearing property, carried forward

Prototype-I's central invariant is that the payload key is composed from the
contributions of every enabled factor, so patching a branch yields nothing. This
was proven experimentally by removing the binding check, recompiling, and
observing continued failure.

The server-side design must not weaken this. A capsule that accepts
`{"authorized": true}` and proceeds would destroy the property that makes
NYEDArch worth building. Hence the key-share ruling: the server contributes
material that participates in key composition, so a patched response handler
produces a wrong key rather than access.

## 5. Document set

| Document | Covers |
|---|---|
| `NYEDARCH_LICENSE_ARCHITECTURE.md` | Service decomposition, boundaries, data stores |
| `NYEDARCH_IDENTITY_AND_MFA.md` | Accounts, roles, authentication, recovery |
| `NYEDARCH_LICENSE_AND_ENTITLEMENTS.md` | Tiers, keys, entitlement lifecycle |
| `NYEDARCH_BUILDER_INTEGRATION.md` | Client ↔ server responsibilities |
| `NYEDARCH_CAPSULE_AUTHORIZATION.md` | Key-share model and cryptography |
| `NYEDARCH_CAPSULE_SERVER_PROTOCOL.md` | Wire protocol, replay resistance, failure |
| `NYEDARCH_AUDIT_AND_FORENSICS.md` | Event model, tamper evidence, retention |
| `NYEDARCH_SECURITY_MONITORING.md` | Risk signals, scoring, proportional response |
| `NYEDARCH_KEY_MANAGEMENT.md` | Key inventory, HSM/KMS, rotation, compromise |
| `NYEDARCH_DISTRIBUTION_AND_UPDATES.md` | Signed releases, platform detection, rollback |
| `NYEDARCH_PAYMENTS_AND_PURCHASES.md` | Checkout, webhooks, refunds, chargebacks |
| `NYEDARCH_THREAT_MODEL.md` | Adversaries, assets, attack surface |
| `NYEDARCH_INCIDENT_RESPONSE.md` | Detection through recovery, per compromise class |
| `NYEDARCH_PRIVACY_AND_DATA_MODEL.md` | Data classification, minimization, retention |
| `NYEDARCH_FUTURE_INTEGRATION_ROADMAP.md` | Sequencing from Prototype-I to licensed |

## 6. What a License Server does not fix

Stated at the outset so the rest of the set is read correctly:

- **Endpoint compromise remains.** A compromised Builder machine is compromised.
- **Already-extracted plaintext cannot be recalled.** Revocation prevents future
  authorization; it does not reach data already restored to disk.
- **Self-deletion cannot be cryptographically confirmed** if the capsule dies
  before its final event is acknowledged.
- **Detection is probabilistic.** No heuristic guarantees breach detection.
- **An audit log is not immutable because it is in a database.** Tamper evidence
  requires a specific construction with stated guarantees.
- **Availability becomes a data-availability concern.** Under the key-share
  ruling, service continuity is a precondition for opening capsules.
