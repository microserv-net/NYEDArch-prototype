# NYEDArch Key Management and Secrets

> DESIGN DOCUMENTATION ONLY. Nothing here is implemented.

## 1. Inventory

| Key | Purpose | Storage | Rotation | Compromise impact |
|---|---|---|---|---|
| **Capsule key shares (`S_server`)** | One authorization factor per capsule | HSM/KMS-wrapped in the isolated share store | Not rotatable per capsule (bound at build) | **Severe** — one factor for the affected capsules |
| Share-wrapping key | Wraps every share at rest | HSM/KMS, never exported | Scheduled, with re-wrap | Severe if combined with store access |
| Grant signing key | Signs authorization grants | HSM/KMS | Scheduled, with overlap | Forged grants — mitigated because grants carry key material, not booleans |
| Release signing key | Signs distributed binaries | Offline/HSM, strictly controlled | Rare, ceremonial | Severe — malicious updates |
| Audit chain-anchor key | Signs audit chain heads | HSM/KMS | Scheduled | Undetectable audit rewriting |
| Session/token keys | Sessions and access credentials | KMS + cache | Frequent | Session forgery |
| Service credentials | Service-to-service auth | Secret manager, short-lived | Frequent | Lateral movement |

**No master secret.** No key protects everything, and no role can retrieve more
than one class.

## 2. Share store controls

Because the share store is the highest-value asset in the system:

1. Shares are wrapped by HSM/KMS; the application never holds an unwrapped share
   at rest.
2. **There is no bulk export path.** Not restricted by policy — absent by design.
3. Unwrapping is per-capsule, authorized, rate-limited, and audited.
4. Separation of duties: infrastructure operators cannot unwrap; the service that
   can unwrap cannot be administered by the same role.
5. Replicas inherit identical protection.
6. Restore is proven by rehearsal, since share loss is unrecoverable.

## 3. Rotation with an unrotatable key

`S_server` is bound into a capsule's key composition at build time and therefore
**cannot be rotated** without invalidating the capsule. This is an honest
structural limitation, not an oversight.

Consequences:

- The wrapping key can rotate freely (re-wrap in place); the share cannot.
- A suspected share-store compromise cannot be remediated by rotation. The
  response is revocation and re-issue: affected capsules are revoked and rebuilt
  from source data by the customer.
- Customers must therefore be told that capsules are re-creatable artifacts, not
  archival originals. **Sealing is not a backup**, and the EULA already says so.

## 4. Compromise response

| Compromised | Immediate | Recovery |
|---|---|---|
| Share store | Suspend share release | Revoke affected capsules; customers rebuild |
| Grant signing key | Revoke key, rotate, invalidate outstanding grants | Capsules must obtain new grants; embedded verification keys limit exposure to that key's cohort |
| Release signing key | Halt distribution, publish advisory | Ceremonial re-key, re-sign, rollback protection |
| Audit anchor key | Rotate, re-anchor | Events since last anchor cannot be re-proven — stated, not hidden |
| Service credential | Revoke, rotate | Investigate lateral movement |

## 5. Separation of duties

Privileged operations — key ceremonies, share-store access changes, revocation of
a customer's entitlement — require two authorized people. Break-glass access
exists, is time-boxed, alarms on use, and is reviewed afterwards without
exception.
