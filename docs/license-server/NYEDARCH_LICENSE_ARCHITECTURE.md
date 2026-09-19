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


---

## The intended licensed system, end to end

> **Prototype-II — yet to be developed.** Nothing below is implemented. It is
> drawn as one picture because the relationships matter more than any single
> box, and because a reviewer should be able to see at a glance which operations
> are local, which need the server, and which paths destroy things.

Four kinds of line, kept visually distinct:

- `───` a cryptographic dependency: the thing downstream cannot proceed without it
- `- ->` telemetry and audit, which never carries payload material
- `═══` a destructive path
- `···` a legitimate recovery path, always narrow and always audited

```text
                        ┌──────────────────────────────┐
                        │      FINAL CAPSULE           │
                        │  (in a hostile environment)  │
                        └──────────────┬───────────────┘
                                       │
                    ┌──────────────────┴──────────────────┐
                    │  local integrity and authenticity   │
                    │  environment / security init        │
                    └──────────────────┬──────────────────┘
                                       │ fails ═══════════════╗
                                       │                      ║
              ┌────────────────────────┴───────────┐          ║
              │   MANDATORY NETWORK DEPENDENCY     │          ║
              │   (Managed mode; no silent         │          ║
              │    fallback to Un-Managed)         │          ║
              └────────────────────────┬───────────┘          ║
                                       │                      ║
   ┌───────────────────────────────────┴───────────────────┐  ║
   │             PROTOTYPE-II LICENCE SERVER               │  ║
   │                                                       │  ║
   │  capsule identity ── account / licence identity       │  ║
   │  server policy evaluation                             │  ║
   │  authenticated time                                   │  ║
   │  revocation state  ── Land Mine                       │  ║
   │                                                       │  ║
   │  emits:  - -> security telemetry                      │  ║
   │          - -> anomaly detection - -> owner notice     │  ║
   │                                                       │  ║
   │  returns: SERVER KEY SHARE (cryptographic material,   │  ║
   │           not a boolean permission)                   │  ║
   └───────────────────────────────────┬───────────────────┘  ║
                                       │ refused ═════════════╣
                                       │                      ║
        ┌──────────────────────────────┴──────────────────┐   ║
        │        LOCAL AUTHORIZATION FACTORS              │   ║
        │  machine ── passphrase ── [location] ── [time]  │   ║
        └──────────────────────────────┬──────────────────┘   ║
                                       │ any fails ═══════════╣
                                       │                      ║
                    ┌──────────────────┴──────────────────┐   ║
                    │   CRYPTOGRAPHIC KEY COMPOSITION     │   ║
                    │   every required contribution, or   │   ║
                    │   no payload key at all             │   ║
                    └──────────────────┬──────────────────┘   ║
                                       │                      ║
                    ┌──────────────────┴──────────────────┐   ║
                    │     AUTHENTICATED DECRYPTION        │   ║
                    └──────────────────┬──────────────────┘   ║
                                       │                      ║
                    ┌──────────────────┴──────────────────┐   ║
                    │   PLAINTEXT (minimum lifetime)      │   ║
                    └──────────────────┬──────────────────┘   ║
                                       │                      ║
                    ┌──────────────────┴──────────────────┐   ║
                    │  cleanup ── optional destruction    │◄══╝
                    └─────────────────────────────────────┘
                              ║ destructive failure may consume
                              ║ the capsule itself
                              ▼

   ···· LEGITIMATE RECOVERY (verified, narrow, one unlock, audited) ····
        support request → identity verified → ownership verified →
        named capsules only → single authorized unlock → audit record
```

### What the picture is meant to make obvious

**The server contributes key material, not permission.** It sits on a
cryptographic dependency line, not beside a branch. Patching a response to say
"authorized" yields nothing, because there was never a boolean to patch — this
is the Prototype-I invariant carried forward rather than replaced.

**Local factors do not become optional.** The server share is an additional
required contribution in Managed mode, not a substitute for the machine,
passphrase, location or time contributions.

**Every failure path leads to the same place**, and that place can destroy the
artifact. There is no branch that fails into a partially-authorized state.

**Telemetry never touches the payload path.** The dashed lines only leave the
server box. Nothing on the decryption path reports upward, and the server never
receives payload material, passphrases, or key material capable of opening a
capsule alone.

**Recovery is a separate, narrow path** — dotted, entered only through verified
support, and audited. It is deliberately not connected to the normal flow,
because a recovery route that touches the ordinary path is a bypass.
