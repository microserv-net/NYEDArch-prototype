# NYEDArch Cryptographic Format & Key Composition (v1)

This document specifies the versioned cryptographic construction implemented in
`nyedarch-crypto`. Everything that feeds key derivation is versioned so future
formats can be rejected safely (§59) and primitives can be migrated (§54).

## Primitives (established, not invented — §54/§68)

| Purpose | Primitive | Crate |
|--------|-----------|-------|
| Passphrase KDF | Argon2id (v0x13) | `argon2` |
| Key composition / expansion | HKDF-SHA512 | `hkdf` + `sha2` |
| Payload AEAD | XChaCha20-Poly1305 | `chacha20poly1305` |
| Constant-time compare | `subtle` | `subtle` |
| Secret hygiene | `zeroize` | `zeroize` |
| CSPRNG | OS getrandom | `getrandom` |

XChaCha20-Poly1305 is chosen over AES-GCM to get a 192-bit nonce (random nonces
are safe) and to avoid depending on hardware AES in a hostile-runtime binary.

## Domain separation

Every derived value is namespaced with a distinct ASCII label
(`version.rs::LABEL_*`), e.g. `nyedarch:v1:factor:passphrase`. Concatenation
without domain separation is explicitly forbidden (§25).

## Factor contributions

Each enabled authorization factor produces a 32-byte contribution, itself
domain-separated before entering composition:

```
C_x = HKDF-SHA512(salt = package_salt, ikm = raw_x, info = LABEL_x)   [32 bytes]
```

| Factor | `raw_x` | Entropy character |
|-------|---------|-------------------|
| Machine (always on) | 32-byte secret selected by the *matched* trusted fingerprint | High |
| Passphrase (always on) | Argon2id(passphrase, package_salt, params) | High (work-hardened) |
| Location (optional) | quantized cell id (`geo::quantize`) | **Low — policy factor only (§21)** |
| Time (optional) | canonical window id (`timewin::window_id`) | **Low — policy factor only** |

Location and time are policy factors: they gate access but are **not** relied on
for key strength. Their low entropy is documented, not disguised (§21/§24).

## The machine factor bootstrap (non-circular — §11/§17)

The forbidden design is "fingerprint needed to decrypt fingerprint." NYEDArch
avoids it:

1. At build time the builder assigns each trusted fingerprint a random 32-byte
   **factor secret** and records the map `fingerprint_id -> factor_secret`.
2. That map (the *protected fingerprint authorization record*) is sealed with a
   per-build **bootstrap key** that is **not derived from any fingerprint**
   (`policy_seal::seal_record`). The bootstrap key belongs to the runtime
   template's protected-fragment/diversification machinery (a later crate), not
   to the payload key path.
3. At runtime: integrity checks → open the sealed record with the bootstrap key
   → capture the live machine fingerprint id → **select** the matching
   `factor_secret`. A non-matching machine selects nothing → no machine
   contribution → no payload key.

So the fingerprint *selects* a high-entropy secret; it is never itself the key.
Extracting the bootstrap key lets an attacker read the record's structure but
**does not** yield any payload key — payload keys additionally require the
passphrase (Argon2id) and any enabled location/time factors. Analyzed further in
`docs/THREAT_MODEL.md` and `ANTI_RE_ANALYSIS.md` (Phase 9).

## Final payload key

```
context = LABEL_PAYLOAD_KEY || crypto_version(LE16) || policy_flags(u8)
                            || package_id(16) || runtime_binding(32)

IKM     = C_machine || C_passphrase || [C_location] || [C_time]   (enabled only, fixed order)

K_pay   = HKDF-SHA512(salt = package_salt, ikm = IKM, info = context)   [32 bytes]
```

Payload sealing:

```
Sealed = XChaCha20-Poly1305(K_pay, nonce = random(24), aad = context, msg = payload)
stored as: nonce(24) || ciphertext||tag
```

### Why bypassing a branch does not help (§32 / exec §7)

`K_pay` is a function of the concatenated **contributions**. A missing or wrong
factor changes `IKM`, so `K_pay` changes, so AEAD authentication fails. There is
no independent `K_pay` sitting behind an `if authorized`. Composition also
**fails closed** if an enabled factor's contribution is absent (it returns an
authorization error rather than deriving a weaker key).

### Runtime binding (§27)

`runtime_binding` and `package_id` are inside both the HKDF `info` and the AEAD
`aad`. A payload copied into a different runtime derives a different key and
fails to authenticate — proven by
`invariant.rs::authorized_open_succeeds_and_binds_runtime`.

### Policy-downgrade resistance

`policy_flags` is inside `context`. An attacker who re-derives claiming a weaker
policy (to drop a required factor) gets a different key — proven by
`invariant.rs::attacker_cannot_downgrade_policy_to_drop_a_factor`.

## Argon2id parameters

Versioned (`kdf::Argon2Params`). Interactive default: **256 MiB, t=3, p=1**.
`validate()` rejects DoS settings (`m_cost > 4 GiB`, or `m_cost < 8*p_cost`,
or zero costs). Parameters are stored in the package header so the runtime
reproduces the exact derivation.

## Geolocation quantization

`geo::quantize` maps an accepted reading to a stable cell id sized to the
configured tolerance (longitude step scaled by `cos(lat)` so cells stay ~tol
meters wide at any latitude), and **rejects** readings whose reported accuracy
exceeds tolerance (§20). The cell id binds the tolerance so different tolerances
never collide. Limitation: a uniform grid has boundary sensitivity; production
target is H3/S2 (`ADR-0003`), swappable without touching the key schedule.

## Time window

`timewin::DailySchedule::window_id` returns the canonical scheduled-slot instant
(not the raw time) when `now` is within tolerance of a slot, checking the
neighbouring local days so windows straddling midnight still match. Every
accepted time in one window yields the same id, so the key reproduces; outside
the window there is no id and the factor fails closed. Prototype uses the local
clock behind the `TimeSource` trait — `FUTURE — LICENSE SERVER` supplies
authenticated time.

---

## Capsule identity (NYEDArch Framework)

### Package magic

```
magic  = "NYARCH"        (6 bytes, ASCII)
version = u16 little-endian
```

The magic changed from the pre-rebrand value when the project became NYEDArch.
Because the format version is checked immediately after the magic, an older
package is rejected cleanly rather than misparsed (spec §59: unsupported
versions must fail safely).

### Capsule file extension

Compiled capsules are named `<name>.nyarch` on **every** operating system.

| Platform | Terminal execution | Reason |
|---|---|---|
| Linux | `./x.nyarch` works | Kernel dispatches on content (ELF header) |
| macOS | `./x.nyarch` works | Kernel dispatches on content (Mach-O header) |
| Windows | **does not work unmodified** | The shell resolves executables via `PATHEXT` |

On Windows the capsule is a valid PE, so `CreateProcess` runs it regardless of
extension — which is why the NYEDArch client's launcher works there. Terminal
use requires adding `.NYARCH` to `PATHEXT`.

Double-click is not expected to work on any OS without a file association. This
is an accepted, documented trade-off of a single cross-platform identity, not an
oversight.
