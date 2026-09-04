# NYEDArch Audit and Forensics

> DESIGN DOCUMENTATION ONLY. Nothing here is implemented.

## 1. Four separate classes, not one log

| Class | Audience | Contents | Retention |
|---|---|---|---|
| **User activity** | The account owner | Logins, capsule authorizations, builds, licence events | User-configurable, bounded by tier |
| **Security audit** | Owner + security operations | MFA changes, recovery attempts, failsafe use, revocations | Longer, tier-bounded |
| **Internal security operations** | Security operations only | Detection logic outputs, correlation, investigation notes | Internal policy |
| **Capsule telemetry** | Aggregated | Categorized lifecycle events | Shortest, user-configurable |

Detection logic and thresholds are **not** exposed to users. Publishing them
would tell an attacker exactly what to stay beneath. Users see *that* an event
was flagged, not the rule that flagged it.

## 2. Tamper evidence — precise claims

Events are hash-chained per tenant:

```
H(n) = SHA-256( H(n-1) || canonical_event(n) )
```

Chain heads are periodically signed and anchored.

**What this guarantees:** silent modification or deletion of a past event breaks
the chain and is detectable at verification.

**What this does NOT guarantee:**

- It is not immutability. An attacker with sufficient database access can rewrite
  the chain *and* recompute subsequent hashes. What defeats this is anchoring the
  head somewhere the same attacker does not control.
- It does not prove an event was *recorded* — only that recorded events were not
  altered afterwards. An event suppressed at ingestion leaves no trace in the
  chain.
- Anchoring frequency bounds the window: events since the last anchor are more
  vulnerable than those before it.

Claiming "immutable audit log" because rows sit in a database would be false, and
this document does not make that claim.

## 3. Capsule lifecycle events

Reportable categories: launch attempt, authorization request, authorization
success, authorization failure (by category), fingerprint result category,
location result category, time result, licence failure, integrity/tamper signal,
repeated failure pattern, extraction success, extraction failure, cleanup,
self-destruction initiated, self-destruction reported complete, abnormal
termination.

## 4. The reporting honesty problem

A capsule cannot report an event it never successfully sends. The model must
distinguish, permanently and in the user interface:

| State | Meaning |
|---|---|
| `self_destruction_requested` | The capsule began destruction |
| `self_destruction_reported` | The capsule reported completion **and the server acknowledged** |
| `self_destruction_unconfirmed` | Destruction began; no completion event arrived |

The third state is not a failure of engineering — a capsule that deletes itself
may die before its final message is acknowledged. Presenting "unconfirmed" as
"destroyed" would be a lie told by the product to its user at exactly the moment
they most need the truth.

Reliability mechanisms: durable client-side queue, sequence numbers per session,
idempotency keys, bounded retry with backoff, server acknowledgement, and
explicit gap detection when sequence numbers are missing.

**Limit:** none of this makes reporting guaranteed. A capsule on a machine with
no network, or one killed mid-run, reports nothing. Gaps are shown as gaps.

## 5. Ordering

Events carry a monotonic per-session sequence number and server-assigned receipt
time. Client timestamps are recorded but treated as **claims**, not facts —
Prototype-I already establishes that the local clock is attacker-controlled.
Ordering across sessions uses server time.

## 6. Access control

A user sees their own account and capsules. An enterprise admin sees the
organisation's. **Support sees neither by default** — access to customer audit
detail requires the customer's involvement, is time-boxed, and is itself audited
as a privileged action.
