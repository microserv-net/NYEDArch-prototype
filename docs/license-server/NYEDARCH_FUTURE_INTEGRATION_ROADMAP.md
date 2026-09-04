# NYEDArch Future Integration Roadmap

> DESIGN DOCUMENTATION ONLY. Nothing here is implemented.

## 1. Prototype-I limitation → future integration

For each: current behaviour, the integration, the mechanism, the improvement, and
what still remains.

### Unauthenticated local time
- **Now:** local system clock; attacker-controlled. Documented as a policy
  control, not an expiry mechanism.
- **Integration:** authenticated server time in the challenge.
- **Mechanism:** time protection evaluated against server time; skew bounded; excessive
  skew fails closed.
- **Improvement:** clock manipulation no longer satisfies the time protection.
- **Remains:** network delay bounds accuracy; tolerance must exceed jitter.

### No revocation
- **Now:** a distributed capsule cannot be recalled.
- **Integration:** centralized revocation by withholding the key share.
- **Mechanism:** share release refused; unforgeable, since the value is never sent.
- **Improvement:** stolen capsules can be permanently disabled.
- **Remains:** already-extracted plaintext cannot be recalled.

### No account identity or MFA
- **Now:** no identity concept at all.
- **Integration:** mandatory accounts and MFA, passkeys preferred.
- **Improvement:** ownership is provable; recovery becomes possible.
- **Remains:** complete account compromise defeats the failsafe.

### No authenticated audit
- **Now:** ephemeral local diagnostics only; no forensic infrastructure (a
  deliberate Prototype-I decision).
- **Integration:** signed, sequenced, hash-chained lifecycle events.
- **Improvement:** owners see authorization attempts against their data.
- **Remains:** an event never sent leaves no trace; chaining is not immutability.

### No recovery for changed hardware
- **Now:** hardware change permanently denies that machine.
- **Integration:** failsafe — licence + MFA + out-of-band approval (individual:
  owner; enterprise: admin + optional reviewers).
- **Improvement:** legitimate users regain access without weakening the model.
- **Remains:** the failsafe is the weakest link in the machine protection.

### `S_machine` extractable from a held capsule
- **Now:** the machine protection is not a standalone confidentiality boundary.
- **Integration:** the server factor adds a contribution that is **not** present
  in the binary at all.
- **Improvement:** extracting the capsule no longer yields every local factor.
- **Remains:** hardware attestation (TPM/Secure Enclave) is still the real fix
  and is independent of the License Server.

### Measured timing oracle (~50x)
- **Now:** denial latency reveals which protection failed.
- **Integration:** none by itself — **the network stage makes this worse.**
- **Required work:** constant-time protection evaluation, in Prototype-I, before or
  alongside licensing.
- **Remains:** network timing is inherently observable; the goal is to avoid
  adding a coarser oracle.

### No centralized misuse detection
- **Now:** a capsule cannot see patterns beyond itself.
- **Integration:** cross-account, cross-capsule risk scoring.
- **Improvement:** systematic abuse becomes detectable.
- **Remains:** probabilistic; false positives are real and must be appealable.

## 2. Sequencing

| Stage | Work | Rationale |
|---|---|---|
| 0 | **Constant-time protection evaluation in Prototype-I** | Do not carry a known oracle into a networked design |
| 1 | Identity, MFA, accounts | Everything else depends on identity |
| 2 | Entitlement, payments, distribution | Commercial foundation |
| 3 | Capsule registration and key shares | The core cryptographic change |
| 4 | Capsule authorization protocol | Depends on 1–3 |
| 5 | Audit and telemetry | Needs stable event semantics |
| 6 | Risk and monitoring | Needs audit data to be useful |
| 7 | Enterprise org management | Builds on all of the above |

Stage 0 is deliberately first. Adding a network round trip in front of an
existing timing oracle widens it; fixing the local issue afterwards would mean
shipping a known weakness in a paid product.

## 3. Independent of the License Server

Hardware-attested identity (TPM / Secure Enclave) remains the most significant
outstanding security improvement and does **not** require the server. It should
not be deferred on the assumption that licensing solves it — it addresses a
different weakness.

## 4. Labelling convention

Current documentation uses:

```
FUTURE — NYEDArch LICENSE SERVER INTEGRATION
```

with the pattern: current prototype behaviour → future integration → what the
integration changes. Vague statements such as "shortcoming to be fixed later" are
not used.
