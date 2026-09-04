# NYEDArch Identity, Roles, MFA, and Recovery

> DESIGN DOCUMENTATION ONLY. Nothing here is implemented.

## 1. Principal types

| Principal | Can do | Cannot do |
|---|---|---|
| Unauthenticated visitor | Browse product information, begin purchase | Reach any account or capsule data |
| Registered account (MFA incomplete) | Complete MFA enrolment, complete purchase | Build capsules, download software, authorize capsules |
| Authenticated account | Manage own profile, sessions, authenticators | Anything requiring an entitlement |
| Licence owner | Build, register, authorize, revoke own capsules; view own audit | Reach other tenants |
| Team admin (enterprise) | Manage org members, org capsules, approve failsafes | Read payload data (none exists server-side) |
| Support | View account status, entitlement state, ticket metadata | **View audit detail, capsule contents, or key material** |
| Security operations | Investigate flagged accounts under approval workflow | Act unilaterally without a second approver |
| System administrator | Operate infrastructure | Access unwrapped key shares |

Support is deliberately weak by default. Elevation is time-boxed, tied to a
ticket, requires the customer's involvement, and is itself audited.

## 2. Mandatory MFA

An account is **not operational** until at least one authenticator is enrolled.
Registration and payment may precede enrolment; capability does not.

Order of preference, by resistance to phishing:

1. **Passkeys / WebAuthn** — preferred; origin-bound and phishing-resistant.
2. **Hardware security keys** — for enterprise and privileged roles.
3. **TOTP** — supported for interoperability with standard authenticator apps.

No proprietary NYEDArch mobile app is required for basic MFA. TOTP alone is
supported but never presented as equivalent: it is phishable, and the interface
should say so plainly rather than implying parity.

Privileged roles (team admin, security operations, system administration)
**require** a phishing-resistant factor; TOTP alone is insufficient for them.

## 3. Recovery — the highest-value target

With mandatory MFA, recovery is where account takeover will be attempted. "Forgot
password" must never become an MFA bypass.

Layers:

| Mechanism | Role |
|---|---|
| Recovery codes | Issued once at enrolment, displayed once, stored hashed |
| Second authenticator | Users are prompted to enrol two; strongly encouraged |
| Delayed recovery | High-risk recovery takes effect only after a waiting period |
| Cancellation window | Any registered contact can cancel a pending recovery during the delay |
| Notification | Every registered channel is notified at initiation, not just at completion |
| Step-up verification | Proof of prior possession where available |
| Manual review | Reserved for high-risk cases; requires two operators |

**Trade-off, stated:** a delay protects against silent takeover but harms a
legitimate user in a hurry. That is the correct direction for a product whose
purpose is protecting data, and the delay is disclosed at enrolment so it is not
a surprise at the worst moment.

## 4. Capsule failsafe (hardware-change unlock)

Distinct from account recovery. This is the path for a legitimate user whose
machine changed, or who is on a trusted machine that was never enrolled.

**Individual accounts:** the account owner approves, via valid licence + MFA
code + out-of-band email approval.

**Enterprise accounts:** the **team admin** approves, with **optional additional
reviewers** configurable per organisation. An organisation handling regulated
data can require two or three; a small team can require one.

In all cases the local protections still apply. The failsafe substitutes for the
*machine* protection only — the passphrase is still required, and location/time remain
in force where enabled. It is not a master key.

| Control | Purpose |
|---|---|
| Rate limiting with exponential escalation | Repeated failsafe attempts become progressively more expensive |
| Full notification | Every registered contact learns of the attempt |
| Audit as a first-class security event | Failsafe use is always visible to the owner |
| Bounded validity | An approval authorizes one unlock, not a standing exemption |

**Honest risk:** the failsafe is, by construction, the weakest link in the
machine protection. A sufficiently complete account compromise — password, MFA, and
email — defeats it. The counterweights are notification, delay, and escalation,
not a claim of impossibility.

## 5. Sessions

Short-lived access credentials with rotating refresh, bound to device where
supported. Session listing and immediate revocation are available to the user.
Revocation is effective within seconds, not at next expiry, because "revoke my
sessions" is a panic action and must behave like one.
