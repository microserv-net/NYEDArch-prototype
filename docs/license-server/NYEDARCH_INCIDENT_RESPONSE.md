# NYEDArch Incident Response

> DESIGN DOCUMENTATION ONLY. Nothing here is implemented.

Each class below is specified as: **Detection → Containment → Revocation →
Recovery → Notification → Forensics → Key rotation → Restoration.**

## 1. Account compromise
Detection: impossible travel, MFA anomalies, recovery attempts, unusual capsule
authorizations. Containment: suspend sessions, require step-up. Revocation:
invalidate sessions and pending recoveries. Recovery: verified re-enrolment of
authenticators. Notification: all registered channels, including the
now-untrusted one, since the attacker's presence there is itself informative to
the user. Forensics: authenticated event history. Rotation: user credentials.
Restoration: normal service once re-enrolled.

## 2. Lost authenticator
Detection: user-initiated. Containment: none required. Recovery: recovery code or
second authenticator; otherwise delayed recovery with cancellation window.
Notification: mandatory. **Trade-off:** the delay is painful for a legitimate
user and is the price of not creating an MFA bypass.

## 3. Leaked licence key
Detection: anomalous activation. Containment: rate limiting. Revocation: reissue,
invalidating the old key. Recovery: immediate. **Impact is limited by design** —
the key is a bootstrap credential and is useless without account authentication.

## 4. Compromised Builder device
Detection: anomalous build patterns. Containment: revoke device sessions.
Revocation: optionally revoke capsules built during the suspected window.
Recovery: customer rebuilds from source data. **Honest limit:** anything the
attacker already extracted on that device is gone; the server cannot recall it.

## 5. Stolen capsule binary
Detection: authorization attempts from unexpected contexts. Containment: revoke
that capsule's share. Effect: it can never be opened again by anyone, including
the owner. Recovery: owner rebuilds. This is the single strongest capability the
License Server adds over Prototype-I.

## 6. Compromised build environment (GitHub)
Detection: provenance verification failure at the client. Containment: halt
builds, rotate build credentials. Recovery: rebuild with verified toolchain.
Note that provenance checking is **client-side**, so a server compromise does not
disable this defence.

## 7. Server signing key compromise
Detection: anomalous signing, external report. Containment: halt signing.
Revocation: revoke key, publish advisory. Recovery: ceremonial re-key, re-sign
current releases. **Grants signed by the compromised key remain a risk for their
validity window**; short expiry bounds the damage.

## 8. Share store breach
The most severe scenario. Detection: access anomalies, integrity checks.
Containment: suspend share release immediately — this stops capsules opening,
which is disruptive and correct. Revocation: mass revocation of affected
capsules. Recovery: customers rebuild. Notification: prompt disclosure stating
plainly that one authorization factor was exposed and that passphrase and machine
factors were not. Rotation: wrapping keys rotate; **shares cannot** — hence
re-issue.

## 9. Insider compromise
Detection: privileged-action audit, anomaly review. Containment: revoke access,
preserve evidence. Recovery: rotate everything the individual could reach.
Controls: two-person rule, break-glass alarms, no single role reaching two data
classes.

## 10. Payment provider attack
Containment: disable the integration. Effect: entitlement changes pause; existing
entitlements continue. **No card data is held by NYEDArch**, which bounds this
class sharply.

## Communication principle

Disclosure states what is known, what is not yet known, and what the customer
should do. It never claims confirmed containment before containment is
confirmed — for the same reason the capsule never reports "destroyed" when it
means "destruction requested".


---

## Land Mine: revoking a capsule that is already in the wild

> **Prototype-II — yet to be developed.**

A capsule is designed to be portable and long-lived. That is the point of it,
and it is also the problem: once a copy has left, the owner has no way to reach
it. A purely standalone archive can never say *"the artifact you shipped six
months ago should no longer open."*

A mandatory-online architecture can. The intended sequence:

1. The owner suspects or confirms that a capsule has been compromised.
2. The owner recovers their own data from trusted local copies where needed.
3. The owner marks that capsule revoked.
4. The server records the revocation.
5. Later server-backed attempts involving that capsule are refused.
6. The attempt produces security telemetry.
7. Owner notification and monitoring follow the documented model.
8. The artifact stays unusable through the server-backed path.

**This is a post-compromise defensive control, not retaliation.** It denies
future use and it creates evidence. It does nothing to the machine attempting
the use, and nothing punitive is intended or implemented.

### What it cannot do

Stated plainly, because a control whose limits are vague gets trusted past them:

- It **cannot recall plaintext already extracted**. If the payload was opened,
  that data is gone from the owner's control permanently.
- It **cannot erase copies** an attacker has made, of the capsule or of anything
  taken out of it.
- It **cannot observe an attacker who never contacts the server**, which
  includes anyone attacking an Un-Managed capsule.
- It **depends on the artifact attempting a server-backed operation**. An
  attacker who never runs it is invisible to this mechanism, as they should be.

### Why it is still worth having

A compromised capsule does not have to remain a permanently useful stolen
credential. The legitimate owner can turn future use attempts into a security
event — which converts a silent, indefinite exposure into a bounded one that
generates evidence.

---

## Post-compromise lifecycle

> **Prototype-II — yet to be developed.**

Detection, notification, audit preservation, revocation and lawful investigation
are **distinct concepts** and are kept distinct here. Collapsing them is how
security architectures end up claiming that noticing something is the same as
containing it.

```text
suspicious capsule activity
        ↓
server-side event recorded
        ↓
anomaly detection
        ↓
owner notification
        ↓
owner confirms or suspects compromise
        ↓
legitimate copies used for data recovery, if required
        ↓
capsule revoked
        ↓
Land Mine active
        ↓
future server-backed use denied
        ↓
each further attempt produces telemetry
```

The honest limitation, again:

> **If the attacker already extracted plaintext, none of this makes that
> plaintext disappear.**

The value of the post-compromise system is containment, continued denial,
detection and evidence. It is not time travel, and any description of it that
implies otherwise is wrong.
