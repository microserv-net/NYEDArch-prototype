# NYEDArch Capsule ↔ Server Protocol

> DESIGN DOCUMENTATION ONLY. Nothing here is implemented.

## 1. Identities

| Identity | Origin | Purpose |
|---|---|---|
| Capsule id | Minted at build, embedded | Which capsule is asking |
| Package commitment | Digest of the sealed package | Detects payload substitution |
| Runtime commitment | Already exists in Prototype-I | Binds package to runtime |
| Build id | ULID from the build record | Correlates to provenance |
| Session id | Server-issued, single use | Replay resistance |

## 2. Flow

```
capsule                                   server
  |-- hello(capsule_id, build_id, proto_ver) ------->|
  |                                                  | look up capsule, licence
  |<-- challenge(nonce, server_time, policy_ver) ----|
  |                                                  |
  | build attestation over:                          |
  |   nonce || capsule_id || package_commitment      |
  |   || runtime_commitment || client_time           |
  |   || categorized_environment                     |
  |-- authorize(attestation, signature) ------------>|
  |                                                  | verify, evaluate policy,
  |                                                  | risk-score, decide
  |<-- grant(S_effective, policy, exp, signature) ---|
  |                                                  |
  | local protections -> compose key -> AEAD open          |
  |-- ack(session_id, outcome_category) ------------>|
```

## 3. Replay and freshness

- Every authorization is bound to a **server-issued nonce**; a captured exchange
  cannot be replayed.
- Grants carry a short expiry; a grant is single-use and session-bound.
- The server records used nonces for their validity window, so a duplicate is
  rejected rather than merely improbable.
- `S_effective` is session-derived (see the authorization document), so replaying
  a captured grant on a later run produces the wrong contribution.

## 4. Response authenticity

Grants are signed with a capsule-authorization signing key whose public half is
embedded in the capsule at build time. A capsule verifies the signature before
using any material.

Note the layering honestly: signature verification is a branch and could be
patched. That is *why* the grant carries key material rather than a boolean — a
patched verifier that accepts a forged grant still yields the wrong share, hence
the wrong key. Signature checking is defense in depth; the key share is the
boundary.

## 5. Time handling

The server supplies authenticated time in the challenge. The capsule compares it
to local time and:

- uses **server time** for the time protection and for grant expiry;
- reports the observed skew as a categorized telemetry signal;
- fails closed when skew exceeds a configured bound, since large skew indicates
  either clock manipulation or a broken client.

Network delay is real and bounded honestly: server time is accurate to within the
round-trip, so the time protection's tolerance window must exceed plausible RTT. A
tolerance smaller than network jitter would produce spurious denials.

## 6. Failure handling

| Condition | Capsule behaviour |
|---|---|
| No connectivity | **Fail closed.** Explicit "cannot reach authorization service" |
| Licence expired/revoked | Fail closed, generic user-facing message |
| Signature invalid | Fail closed |
| Nonce replayed / grant expired | Fail closed |
| Skew beyond bound | Fail closed |
| Server error (5xx) | Fail closed, with retry guidance; never "assume authorized" |

There is deliberately no code path in which an unreachable or erroring server
results in access.

## 7. Message discipline

The capsule sends identifiers and **categories**, never raw sensitive values: no
passphrase material, no raw fingerprint signals, no raw coordinates. See the
privacy document for the categorization scheme.
