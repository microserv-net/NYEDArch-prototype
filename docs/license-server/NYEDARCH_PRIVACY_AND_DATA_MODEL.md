# NYEDArch Privacy and Data Model

> DESIGN DOCUMENTATION ONLY. Nothing here is implemented.

## 1. Classification

| Data | Class | Required? | Visibility | Retention |
|---|---|---|---|---|
| Email address | Identifying | Required | User | Account lifetime |
| Authenticator registrations | Security-sensitive | Required | User | Account lifetime |
| Password verifier | Security-sensitive | Required | Nobody | Account lifetime |
| Licence verifier | Security-sensitive | Required | Nobody | Term + record period |
| Capsule id, package/runtime commitments | Operational | Required | User | Capsule lifetime |
| **Capsule key share** | **Critical** | Required | Nobody | Capsule lifetime |
| Authorization outcome categories | Derived | Required | User | User-configurable, tier-bounded |
| Fingerprint result category | Derived | Required | User | Same |
| Location result category | Derived | Optional | User | Same |
| Clock skew magnitude | Derived | Required | Internal | Short |
| IP address | Identifying | Required (security) | Internal | Short |
| Payment tokens | Financial | Required | Provider | Provider policy |
| **Raw fingerprint signals** | — | **Never collected** | — | — |
| **Raw coordinates** | — | **Never collected** | — | — |
| **Passphrase / payload** | — | **Never collected** | — | — |

## 2. Categorization instead of raw values

The design principle is that the server should learn whether a protection succeeded,
not what the user's machine or whereabouts are.

| Instead of | The server receives |
|---|---|
| Raw hardware serials | `fingerprint: matched` / `no_match` |
| Latitude and longitude | `location: within_region` / `outside_region` / `accuracy_insufficient` |
| Exact local timestamp | `time: within_window` / `outside_window`, plus skew magnitude |
| Environment inventory | `environment: nominal` / `anomalous` |

This is a genuine restriction, not a presentational one: with these categories
the server cannot reconstruct a user's location history or hardware inventory,
even if fully compromised.

**Cost, acknowledged:** categorization reduces forensic richness. An investigator
cannot tell *which* wrong machine attempted access, only that one did. That is
the correct trade for a product sold on confidentiality.

## 3. Retention

**User-configurable, bounded by tier** (settled decision). The user chooses
within limits; shorter is the default. Data past its window is deleted, not
merely hidden.

Fixed floors exist only where security requires them — for example, recent
security events must survive long enough to investigate an active incident — and
those floors are disclosed rather than silently applied.

## 4. Enterprise visibility boundary

An enterprise admin sees organisational security posture: capsule authorization
outcomes, licence state, failsafe approvals, revocations.

An admin does **not** get a behavioural feed on individual employees. In the HR
scenario the product exists partly to constrain insiders, including
administrators. A design that hands admins comprehensive employee surveillance
would undermine the very property being sold.

## 5. Data subject rights

Export and deletion are supported. Deleting an account deletes its key shares,
which **permanently prevents its capsules from opening**. This is stated
explicitly at the point of deletion, with confirmation, because it is
irreversible and users will otherwise not expect it.
