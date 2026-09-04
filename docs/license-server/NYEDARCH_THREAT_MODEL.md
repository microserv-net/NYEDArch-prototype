# NYEDArch License Server — Threat Model

> DESIGN DOCUMENTATION ONLY. Nothing here is implemented.

## 1. Assets, by value

1. **Capsule key shares** — one authorization factor for every capsule issued.
2. **Release signing key** — enables malicious software distribution.
3. Account credentials and authenticators.
4. Grant signing key.
5. Audit integrity and anchor keys.
6. Personal data.

Note the ordering: the release signing key ranks above account credentials,
because compromising it lets an attacker ship a malicious Builder to everyone.

## 2. Adversaries

| Adversary | Capability | Primary goal |
|---|---|---|
| Capsule holder | Full local control, disassembly, patching, instrumentation | Open a capsule they are not authorized to open |
| Licence evader | Legitimate account, patched client | Exceed entitlement, share a licence |
| Account attacker | Phishing, credential stuffing, SIM swap | Take over an account and use the failsafe |
| Network attacker | Interception, replay | Forge or replay authorization |
| Insider (NYEDArch) | Privileged access | Exfiltrate shares or customer data |
| Enterprise insider | Legitimate org access | Exfiltrate data outside policy |
| Supply-chain attacker | Dependency or build compromise | Ship malicious code |
| Infrastructure attacker | Cloud or database access | Bulk theft of shares |

## 3. Attack surface and posture

| Attack | Mitigation | Residual |
|---|---|---|
| Patch the server-authorized branch | Server contributes key material, not a boolean | None specific to this attack |
| Replay a captured grant | Nonce, single-use session, session-derived share | Capture during a *successful* run remains possible |
| Extract the share from memory mid-run | Zeroization, minimal lifetime | Not preventable against live memory access |
| Bulk theft of shares | HSM/KMS wrapping, no bulk export, split duties | Sophisticated insider with two roles compromised |
| Account takeover then failsafe | Mandatory MFA, delay, notification, escalation | Complete compromise (password + MFA + email) defeats it |
| Licence sharing | Concurrency signals, risk scoring | Detection is probabilistic |
| Malicious release | Signing, provenance, rollback protection | Signing key compromise |
| Audit suppression at ingestion | Sequence numbers, gap detection | An event never sent leaves no trace |
| Denial of service against authorization | Redundancy, rate limiting | **Outage means capsules cannot open** |

## 4. The availability threat is a first-class security concern

Under the key-share ruling, an attacker who can deny access to the authorization
service denies users access to their own data. Availability therefore appears in
this threat model rather than only in an operations runbook: DDoS resistance,
multi-region failover, and drain-before-maintenance are security controls here,
not just uptime engineering.

## 5. What remains true regardless

- Endpoint compromise is not solved by a server.
- Data already extracted cannot be recalled.
- A capsule's holder can always analyse it offline; the server changes what they
  can *do* with the analysis, not whether they can perform it.
- Detection is probabilistic; confirmation of breach is never guaranteed.
