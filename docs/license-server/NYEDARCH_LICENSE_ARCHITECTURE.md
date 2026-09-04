# NYEDArch License Server — System Architecture

> DESIGN DOCUMENTATION ONLY. Nothing here is implemented.

## 1. Shape: modular services, not a monolith, not a microservice sprawl

The decomposition is driven by **security isolation**, not by fashion. Two
services exist separately only where a compromise boundary justifies the
operational cost.

| Service | Why it is separate | Handles secrets? |
|---|---|---|
| **Edge / API gateway** | Terminates TLS, rate limits, schema-validates before anything reaches business logic | No |
| **Identity** | Account, session, MFA, recovery. Highest-frequency attack target | Credential verifiers |
| **Entitlement** | Licences, tiers, terms, revocation state | Licence verifiers |
| **Capsule Authorization** | Holds and releases key shares. **The most sensitive service in the system** | Yes — key shares |
| **Audit** | Append-only event ingestion and query | Integrity keys |
| **Telemetry ingestion** | High-volume, untrusted input from capsules | No |
| **Risk** | Correlates signals, scores, recommends response | No |
| **Notification** | Email and out-of-band approvals | No |
| **Distribution** | Signed release artifacts and update manifests | Signing via KMS |
| **Billing bridge** | Payment provider webhooks → entitlement events | No card data |

**Capsule Authorization is isolated above all others.** It is the only service
that can touch key shares, it has its own datastore, its own credentials, and it
is not reachable directly from the public edge — requests reach it only through
the gateway after identity and entitlement checks have already succeeded.

## 2. Trust boundaries

```
[ capsule / builder ]  UNTRUSTED
        |  TLS, authenticated, replay-resistant
[ edge / gateway ]     semi-trusted: validates shape, never authorizes
        |
[ identity ] [ entitlement ] [ risk ]      trusted services
        |
[ capsule authorization ]   HIGH TRUST, isolated, HSM/KMS-backed
        |
[ share store ]             HIGHEST VALUE ASSET
```

## 3. Data stores and separation

| Store | Contents | Separation rationale |
|---|---|---|
| Identity DB | Accounts, credential verifiers, authenticators, sessions | Breach must not yield key shares |
| Entitlement DB | Licences, tiers, terms, revocations | Breach must not yield identity secrets |
| **Share store** | Wrapped capsule key shares | Separate credentials, separate encryption domain, HSM/KMS-wrapped, no bulk export path |
| Audit store | Append-only, hash-chained events | Write-mostly; compromise must be detectable |
| Telemetry store | Categorized capsule events | Lowest sensitivity; short retention |

No single service credential grants access to more than one of these.

## 4. Why the share store dominates the design

Under the key-share ruling, this store holds one authorization factor for every
capsule ever issued. A bulk breach does **not** yield plaintext — the passphrase
and machine factors still stand — but it removes one layer across the entire
installed base simultaneously.

Consequences, treated as requirements rather than recommendations:

1. Shares are wrapped by an HSM or managed KMS; the application never sees an
   unwrapped share at rest.
2. There is **no bulk export path**, for any role, by design rather than policy.
3. Access is per-capsule, authorized, rate-limited, and audited.
4. Replicas inherit the full protection requirement; a replica is not a
   lower-tier system.
5. Restore is proven by rehearsal, because share loss is unrecoverable by
   construction.

## 5. Availability as a correctness property

Because a capsule cannot open while its share is unreachable, availability is
part of the security/product contract:

- multi-region replication with rehearsed failover;
- maintenance drains traffic to standby capacity **before** any change;
- degradation is explicit — a capsule receives a clear "service unavailable,
  cannot authorize" outcome rather than an ambiguous failure.

**Honest limit:** high availability reduces but never eliminates the risk that a
capsule cannot be opened at a given moment. This is an inherent cost of the
key-share ruling and must be disclosed, not engineered away in prose.

## 6. Statelessness and sessions

Application services are stateless; session state lives in a shared cache with
short TTLs. Capsule authorization sessions are single-use and bound to a
challenge, so a captured session cannot be replayed for a second unlock.

## 7. Asynchronous processing

Telemetry ingestion, risk scoring, notification, and audit chaining are
asynchronous behind a durable queue, so that a telemetry backlog can never delay
or block a legitimate authorization. **Authorization is never queued.**
