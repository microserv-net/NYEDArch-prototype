# NYEDArch Distribution and Updates

> DESIGN DOCUMENTATION ONLY. Nothing here is implemented.

## 1. Targets

Windows (MSVC, x86_64), macOS (Apple Silicon), Linux (x86_64 gnu).

## 2. Platform detection is convenience, never a security decision

```
detect OS -> detect architecture -> recommend build -> USER CONFIRMS -> verified download
```

Detection uses client-supplied hints, which are trivially spoofed — and it does
not matter, because detection only pre-selects a menu entry. Users can always
choose another platform manually. No security decision derives from detection.

## 3. Authenticity

| Layer | Purpose | Limitation |
|---|---|---|
| Platform code signing | OS-level trust, avoids SmartScreen/Gatekeeper warnings | Certificate compromise; issuance policy varies |
| Signed release manifest | Binds version, platform, digest, release date | Only as good as the signing key |
| Published checksums | Independent verification | Useless if fetched from the same compromised source |
| Release provenance | Links a binary to its build inputs | Requires a trustworthy build pipeline |
| Rollback protection | Client refuses older-than-installed versions | Complicates legitimate downgrade |

Checksums published beside a download are near-worthless if the attacker controls
the page. They matter when fetched over an independent channel — this should be
said plainly rather than treating a checksum column as proof.

## 4. Linux is not uniform

Distributions differ in libc, packaging, and init. Honest handling:

- publish a widely-compatible build with a documented minimum glibc;
- publish per-distribution packages where practical;
- state clearly which distributions are tested versus merely expected to work.

Claiming "Linux support" without qualification would be misleading.

## 5. Updates

Signed manifests, staged rollout, version pinning for enterprise, and a documented
emergency-update path for security fixes. Update checking must not become a
covert channel: it reports version and platform, nothing more.

## 6. Compromise recovery

If a release signing key is compromised: halt distribution, publish an advisory
through independent channels, ceremonially re-key, re-sign current releases, and
require re-verification on next update. Clients that already installed a
malicious build cannot be fixed remotely — that endpoint is compromised, and the
advisory must say so rather than implying an automatic remedy.


---

## Vulnerability lifecycle, disclosure and capsule migration

> **Prototype-II — yet to be developed.**

Launch day starts a clock for NYEDArch itself, not only for attackers. There are
three running at once and they are easy to conflate:

| Clock | Question it asks |
|---|---|
| Attacker | How long can they prepare and execute? |
| Defender | How fast can an owner detect, investigate, revoke, contain? |
| **Ecosystem** | How fast can NYEDArch find a flaw, patch it, ship it, get users to update, and get already-issued capsules re-sealed? |

The third is the one a conventional product never faces this sharply. A SaaS
vendor patches a server and every user is fixed. A capsule is **portable and
long-lived by design**, which is the point of it — and it means a flaw in the
runtime affects artifacts already in other people's hands, over which the owner
has no control.

Updating the Builder does not repair capsules already sealed. A user with 500
capsules created six months ago still has 500 capsules built on the old runtime.

### The intended lifecycle

```text
vulnerability discovered
        ↓
triage and severity classification
        ↓
patch developed
        ↓
new runtime / security generation released
        ↓
server marks affected runtime and package versions vulnerable
        ↓
owners notified
        ↓
minimum acceptable runtime version enforced
        ↓
affected capsules become RESTRICTED: server-backed use requires migration
        ↓
owner re-seals from trusted source data
        ↓
new capsule inherits the corrected runtime
        ↓
old capsule revoked
```

An artifact therefore moves `VALID → SECURITY UPDATE REQUIRED → REVOKED` rather
than staying usable forever merely because it was valid when issued. This is
what a mandatory-online architecture buys that a standalone archive cannot: the
ability to say *"the thing you made in March is now known-vulnerable; stop using
it."*

Land Mine is not only a post-theft control. It is also the mechanism by which a
known-vulnerable artifact can be forced into migration rather than left silently
exploitable.

### The security update adoption gap

**A patched runtime does not retroactively repair already-issued capsules.**
Until vulnerable artifacts are migrated or revoked, they may remain exposed.

This is a first-class operational security problem, not a footnote:

- Between disclosure and deployment there is a window in which the flaw is known
  and the fix is not yet everywhere.
- Users who do not update stay exposed, and the owner of a capsule is often not
  the person holding it.
- Re-sealing cost scales with the number of existing capsules, so the users with
  the most to lose have the most work to do.
- If a serious flaw becomes public before the ecosystem has migrated, the
  attacker has a window measured in however long adoption takes.

**Un-Managed mode cannot be rescued this way.** A capsule that never contacts a
server cannot be told that its generation is vulnerable. The honest design is
version and security-generation metadata the capsule already carries, so it can
recognise that it was built under an older generation and say so — not a claim
that it can learn about flaws discovered after it was sealed.

Documenting this makes the architecture look more mature, not weaker. A system
that claims no adoption gap is a system that has not thought about one.


---

## The vulnerability research function

> **Prototype-II — yet to be developed.** This describes an intended operating
> commitment, not software. It is recorded here because the lifecycle above
> assumes someone performs it, and an architecture that assumes a function
> nobody owns is an architecture with a hole in it.

The migration machinery only helps if flaws are found by the project before they
are found by someone else. That requires continuous work, not a launch-day
audit:

| Activity | Feeds |
|---|---|
| Continuous adversarial research against current runtimes | Discovery |
| Responsible disclosure handling | Triage and severity |
| Emergency patch capability | Patch and release |
| Staged rollout | Release |
| Telemetry on which generations are still in use | Notification, enforcement |
| Enforced migration where risk warrants it | Revocation |

Two honest points. Finding your own flaws first is a goal, not a guarantee —
nobody gets to promise that. And the telemetry that tells you which generations
are still in use is the same telemetry the privacy model constrains, so "know
what is deployed" and "collect the minimum" are in tension and have to be
resolved deliberately rather than by whichever ships first.
