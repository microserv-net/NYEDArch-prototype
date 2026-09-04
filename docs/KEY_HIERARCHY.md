# NYEDArch — Key Hierarchy

Status: **normative**. Produced by the architecture correction pass.
Applies to crypto version 1.

This document enumerates every cryptographic key and secret in NYEDArch, and
enforces the separation rule:

```
PAYLOAD CRYPTO  ≠  AUTHORIZATION CRYPTO  ≠  GITHUB TRANSPORT CRYPTO
                ≠  ARTIFACT AUTHENTICITY ≠  LOCAL KEYSTORE PROTECTION
```

No key from one column may become usable in another. The enforcement mechanism
is domain separation: every derivation passes a distinct label through
HKDF-SHA512, so even identical input keying material yields unrelated outputs in
different domains. Labels live in `nyedarch-crypto/src/version.rs`.

---

## 1. Inventory

### 1.1 Payload crypto

#### `K_payload` — payload/content encryption key

| Property | Value |
|---|---|
| Origin | Derived, never stored, never transmitted |
| Derivation | `HKDF-SHA512(salt = package_salt, ikm = ‖ enabled factor contributions, info = context)` |
| Purpose | XChaCha20-Poly1305 sealing of manifest + payload chunks |
| Persistence | **None.** Exists only in RAM during a build or an authorized run |
| Storage | Nowhere. Not in the package, not in the binary, not on GitHub |
| Lifetime | From composition until the last chunk is processed |
| Rotation | Per package. A new build ⇒ new salt, new binding ⇒ unrelated key |
| Zeroization | `Zeroizing<[u8;32]>`, wiped on drop |
| Access | `nyedarch-crypto::compose` only; builder at seal time, runtime after full authorization |

`context` = `LABEL_PAYLOAD_KEY ‖ crypto_version ‖ policy_flags ‖ package_id ‖
runtime_binding`, used as **both** HKDF `info` and payload AAD.

The factor contributions are each domain-separated *before* concatenation, so no
two factors' bytes can be confused or shifted into one another:

```
ikm = HKDF(salt, machine_secret,  LABEL_MACHINE)
    ‖ HKDF(salt, passphrase_key,  LABEL_PASSPHRASE)
    ‖ HKDF(salt, location_cell,   LABEL_LOCATION)    // iff enabled
    ‖ HKDF(salt, time_window,     LABEL_TIME)        // iff enabled
```

Presence must match policy exactly; a mismatch fails closed (`CryptoError::Param`).

---

### 1.2 Authorization crypto

#### `S_machine` — per-fingerprint factor secret

| Property | Value |
|---|---|
| Origin | Fresh 32 bytes from the OS CSPRNG **at build time**, one per trusted fingerprint |
| Derivation | Not derived — minted. Deliberately independent of the fingerprint value |
| Purpose | The machine factor's contribution to `K_payload` |
| Persistence | Inside the sealed `PolicyRecord` in the package |
| Storage | Ciphertext at rest; plaintext only after the record is unsealed |
| Lifetime | Build time; then per-run, from record unseal to key composition |
| Rotation | Per package; a re-issued package mints new secrets |
| Zeroization | Wiped with the decrypted record buffer |
| Access | Builder (mint) and runtime (select-on-match) |

This is the resolution of the bootstrap problem (§3 below): the fingerprint does
not *become* the key, it **selects** a high-entropy secret.

#### `K_bootstrap` — policy bootstrap key

| Property | Value |
|---|---|
| Origin | Fresh 32 bytes per build |
| Derivation | Minted; embedded in the generated runtime |
| Purpose | With `C_runtime`, derives the subkey that seals the `PolicyRecord` |
| Persistence | Embedded as a constant in the generated binary |
| Storage | **Recoverable by an attacker who owns the binary — assumed public to a determined analyst** |
| Lifetime | Life of the capsule |
| Rotation | Per build |
| Zeroization | N/A (static) |
| Access | Runtime only. The builder uses it once, at seal time |

#### `C_runtime` — runtime identity commitment

| Property | Value |
|---|---|
| Origin | Fresh 32 bytes per build (`runtime_binding`) |
| Purpose | Binds a package to exactly one generated runtime |
| Storage | Embedded in the binary **and** recorded in the package header |
| Access | Runtime asserts its own embedded value; the header value is never trusted |

```
K_policy_seal = HKDF(package_salt, K_bootstrap ‖ C_runtime, LABEL_POLICY_SEAL)
```

---

### 1.3 GitHub transport crypto — Channel A (Client → GitHub)

| Property | Value |
|---|---|
| Purpose | Authenticity/confidentiality of build inputs sent to the build environment |
| Private half | Held by the client, in OS-native secure storage |
| Public half | Stored as a GitHub Actions **repository secret** (never committed) |
| Lifetime | Repository lifetime; rotatable without touching any existing capsule |
| Access | Client and the Actions runner |

**This key can never decrypt a payload.** It participates in no `compose`
derivation, and its domain label is disjoint from all payload labels.

### 1.4 Artifact authenticity — Channel B (GitHub → Client)

| Property | Value |
|---|---|
| Purpose | Signing the artifact provenance statement |
| Signing half | Repository secret, used by the runner |
| Verifying half | Held by the client |
| Covered data | `signed_input()` — build id, target, package/runtime/source commitments, artifact digest |
| Access | Runner signs; client verifies |

Channels A and B use **independent keypairs** (spec §42). Reuse is prohibited:
a signing key that also decrypted transport data would let a compromised runner
mint artifacts the client would accept.

### 1.5 Local keystore protection

| Property | Value |
|---|---|
| Purpose | Protecting client-held private material at rest |
| Mechanism | Windows DPAPI / macOS Keychain / Linux Secret Service |
| Lifetime | Until the user removes the profile |
| Loss semantics | **Loss of this material may make previously generated artifacts unverifiable.** Stated in the EULA and first-run wizard (spec §51) |

### 1.6 Non-keys

`package_salt`, `package_id`, `runtime_binding` (header copy), policy flags and
Argon2 parameters are **public**. They are integrity-protected as AAD but carry
no confidentiality.

---

## 2. Authorization is cryptographic, not boolean-gated

The forbidden design is:

```rust
if authorized { obtain_static_payload_key() }   // NOT NYEDArch
```

For each factor NYEDArch distinguishes the *validation result* from the *key
material*:

| Factor | Boolean validation | Cryptographic contribution |
|---|---|---|
| Machine | did a trusted fingerprint match? | `S_machine`, selected by the match |
| Passphrase | (none — no verifier is stored) | Argon2id output |
| Location | accuracy ≤ tolerance? | quantized cell id |
| Time | inside the window? | canonical window id |

The payload key is a function of the contributions. A missing or wrong
contribution yields a different key, and AEAD authentication fails. There is no
code path where a boolean grants access to a key that exists independently of
that boolean.

**Note on the low-entropy factors.** Location and time are *policy* factors, not
entropy sources — a determined attacker who knows the policy can enumerate cells
and windows. They are documented as such and are never claimed to add
meaningful entropy. Their security value is that they must be satisfied
*in addition to* the machine and passphrase factors.

### Demonstrated, not asserted

Neutralising the binding comparison in the runtime source and rebuilding
(transplanting package B into runtime A) yields:

```
IntegrityCheck → BindingValidation → BootstrapPolicy → FailClosed
```

The patched branch is passed; the unseal still fails, because
`K_policy_seal` is derived from the runtime's own `C_runtime`. The attacker
gains one state transition and no plaintext. Covered by
`nyedarch-crypto/tests/invariant.rs` and the inline `policy_seal` tests.

---

## 3. The fingerprint bootstrap, formally

**Public** (attacker recovers from the binary): `K_bootstrap`, `C_runtime`,
package salt/id, policy flags, KDF parameters, all code.

**Protected**: the `PolicyRecord` — trusted fingerprint ids, their
`S_machine` secrets, and labels.

**Unlock path**: `K_policy_seal = HKDF(salt, K_bootstrap ‖ C_runtime, label)`.
No fingerprint is required to unseal the record, so there is no circularity.

**What the attacker gets by unsealing it**: the *hashed* fingerprint ids and the
`S_machine` secrets. This is a real and deliberate exposure, and it is why the
record is not the confidentiality boundary:

- fingerprint ids are salted digests — they do not reveal raw hardware
  identifiers (spec §10);
- `S_machine` alone is useless: `K_payload` also requires the Argon2id
  passphrase contribution, plus location/time when enabled.

**Where fingerprint authorization gets its teeth**: an attacker on an
unauthorized machine can obtain `S_machine` only by extracting it from a capsule
they possess. That does not defeat the passphrase factor. Machine authorization
therefore raises cost and constrains *where* a capsule is usable; it is not, and
is not claimed to be, a standalone confidentiality boundary. Hardware-attested
binding (TPM/Secure Enclave) is the strengthening path and is recorded as
future work rather than implied here.

---

## 4. Package ↔ runtime binding

| Attack | Result | Mechanism |
|---|---|---|
| Runtime A + Package B | Fail at `BindingValidation` | Header commitment ≠ own commitment |
| …with that check patched out | Fail at `BootstrapPolicy` | `K_policy_seal` uses own `C_runtime` |
| Copied sealed payload into another capsule | AEAD auth failure | `runtime_binding` is in HKDF `info` **and** AAD |
| Modified manifest | AEAD auth failure | Manifest is a sealed chunk with tagged AAD |
| Modified build identity in header | Fail at `BindingValidation`, then unseal failure | Same as rows 1–2 |
| Chunk reorder / truncation | AEAD auth failure | AAD binds chunk index and terminal flag |
| Replaced runtime | No `K_bootstrap`, no `C_runtime` | Attacker must forge both |
| Artifact transplantation | Provenance rejection | Signature covers build id, target, all commitments |

---

## 5. Zeroization summary

| Material | Handling |
|---|---|
| Passphrase bytes | `Zeroizing<Vec<u8>>` from acquisition |
| Argon2id output | `Zeroizing<[u8;32]>` |
| Decrypted `PolicyRecord` | `Zeroizing<Vec<u8>>` |
| `S_machine` | Wiped with the record buffer |
| `K_payload` | `Zeroizing<[u8;32]>` |
| Decompressed chunks | `Zeroizing<Vec<u8>>` |
| `K_bootstrap`, `C_runtime` | Static; not zeroizable by design |

**Limitation.** Zeroization is best-effort. It cannot defeat an attacker with
live memory-inspection capability, and does not survive swap, hibernation, or
core dumps. Claimed accordingly (spec §66: no unbreakable claims).

---

## 6. Component access matrix

| Key | Builder | Runtime | Actions runner | Repository |
|---|---|---|---|---|
| `K_payload` | derives at seal | derives after auth | never | never |
| `S_machine` | mints | selects on match | never | never |
| `K_bootstrap` | mints, uses once | embedded | never | never |
| `C_runtime` | mints | embedded | never | in header only |
| Channel A private | holds | never | never | never |
| Channel A public | holds | never | reads as secret | as secret |
| Channel B signing | never | never | uses | as secret |
| Channel B verifying | holds | never | never | never |
| Keystore master | OS-held | never | never | never |

The runtime column is the security-relevant one: a capsule in hostile hands
carries `K_bootstrap` and `C_runtime` and **nothing else**.
