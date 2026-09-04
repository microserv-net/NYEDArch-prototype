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
