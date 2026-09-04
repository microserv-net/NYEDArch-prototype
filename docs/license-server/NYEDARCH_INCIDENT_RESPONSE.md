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
