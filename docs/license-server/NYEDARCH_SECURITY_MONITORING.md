# NYEDArch Security Monitoring and Misuse Detection

> DESIGN DOCUMENTATION ONLY. Nothing here is implemented.

## 1. Pipeline

```
signal -> risk evaluation -> correlation -> risk category -> proportional response
```

No single heuristic triggers an automatic penalty. A user is never banned because
one rule fired.

## 2. Signals

Account: impossible travel, anomalous login patterns, repeated MFA failures,
repeated recovery attempts, credential-stuffing indicators, unusual session
patterns.

Licence: repeated activation attempts, concurrent use suggesting sharing,
abnormal build volume.

Capsule: excessive authorization failures, repeated fingerprint mismatches,
repeated tamper signals, unusual execution frequency, environment anomalies,
large clock skew.

## 3. Interpreting signals honestly

Several signals have benign explanations, and the design must assume so:

| Signal | Benign explanation |
|---|---|
| Impossible travel | VPN, corporate proxy, roaming |
| Repeated fingerprint mismatch | The user legitimately upgraded hardware |
| Large clock skew | A misconfigured machine, not an attack |
| Concurrent activation | A user with a laptop and a desktop |
| Tamper signal | Security software, virtualisation, or a debugger attached for unrelated work |

Prototype-I already documents that its environment probes carry false-positive
risk on virtualised and loaded machines. That reasoning carries forward: a signal
raises a score, it does not convict.

## 4. Proportional response

| Category | Response |
|---|---|
| Low | Record only |
| Elevated | Notify the user |
| High | Step-up authentication on the next sensitive action |
| Severe | Temporary restriction, with notification |
| Critical | Entitlement suspension pending review |
| Confirmed abuse/fraud | Permanent revocation, after human review |

**Permanent revocation is never automatic.** It requires human review, because it
is irreversible and the customer must repurchase.

## 5. Exponential escalation

Repeat restrictions on the same licence escalate:

```
1st: ~1 week    2nd: ~1 month    3rd: longer, plus mandatory review
```

The purpose is to make brute-force and systematic abuse progressively more
expensive while leaving a first-time mistake recoverable.

**Trade-off, stated:** escalation punishes a user whose environment repeatedly
trips a detector for innocent reasons. Appeal must be genuinely available, and
escalation state must be visible to the user so it is never a silent countdown.

## 6. Avoiding a surveillance product

NYEDArch protects data belonging to its users; it must not become a tool that
watches them.

- Telemetry is categorized, never raw identifiers or coordinates.
- Detection exists to protect the account, not to profile behaviour.
- Enterprise admins see organisational security events, **not** individual
  employee activity beyond what security requires.
- Retention is user-configurable within tier bounds, and shorter is the default.

If a monitoring feature cannot be justified as protecting the customer's data, it
does not belong in the product.
