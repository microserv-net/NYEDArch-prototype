Before implementing additional crates, perform a focused architecture correction pass. The current architecture is directionally approved, but do not proceed with runtime/package implementation until the following are resolved and documented.

## 1. Define the complete cryptographic key hierarchy

Create `KEY_HIERARCHY.md`.

Explicitly define every cryptographic key/material in the system, including:

* payload/content encryption key,
* passphrase-derived material,
* machine/fingerprint-derived material,
* optional location-derived material,
* optional time-derived material,
* policy/bootstrap protection material,
* GitHub Client→GitHub transport keys,
* GitHub→Client artifact/build keys,
* artifact authenticity/verification keys,
* local keystore protection boundaries.

For every item document:

* origin,
* derivation,
* purpose,
* persistence,
* storage location,
* lifetime,
* rotation/replacement semantics,
* zeroization behavior,
* which component can access it.

Enforce strict separation:

PAYLOAD CRYPTO ≠ AUTHORIZATION CRYPTO ≠ GITHUB TRANSPORT CRYPTO ≠ ARTIFACT AUTHENTICITY ≠ LOCAL KEYSTORE PROTECTION.

No GitHub transport key may accidentally become a payload decryption key.

---

## 2. Prove that authorization is cryptographic rather than boolean-gated

Do not implement a design equivalent to:

`if authorized { obtain static payload key }`.

For each required authorization factor, distinguish between:

* boolean validation result,
* actual cryptographic key material.

The architecture must ensure that bypassing a conditional branch alone does not automatically provide the payload decryption key.

Document exactly how successful machine authorization contributes cryptographic material to the final authorization/decryption path without introducing circular fingerprint bootstrapping.

---

## 3. Resolve the protected fingerprint bootstrap formally

The fingerprint authorization data must remain protected before the fingerprint check, but the fingerprint itself cannot be required merely to decrypt itself.

Document:

* what is public,
* what is protected,
* how protected fingerprint authorization records are unlocked,
* what the attacker can recover from the binary,
* how this does not directly expose the payload,
* how fingerprint authorization becomes cryptographically meaningful.

Do not solve this merely by hiding a static key inside the executable.

---

## 4. Finalize package/runtime binding

Define exactly how a sealed package is bound to its generated runtime/build identity.

The design must be explicit about what happens if an attacker attempts:

* Runtime A + Package B,
* copied sealed payload,
* modified manifest,
* replaced runtime,
* modified build identity,
* artifact transplantation.

Document this in the package format and key hierarchy.

---

## 5. Refine the runtime state machine

Keep the current state machine as the high-level model, but do not prematurely freeze the order until the key hierarchy is finalized.

Add explicit conceptual states for:

* runtime/package binding validation,
* payload-key unwrap/derivation,
* extraction verification.

Security-critical transitions must be tied to the final key hierarchy rather than existing merely as booleans.

---

## 6. Define artifact provenance

The client must verify more than a downloaded hash.

Document how the client establishes that:

> this artifact is the expected output for this specific build ID, target, package commitment, and runtime configuration.

Define:

* build manifest,
* build identity,
* package commitment,
* runtime commitment,
* expected target,
* artifact identity,
* verification/authenticity mechanism.

GitHub remains an untrusted external build environment.

---

## 7. Tighten Builder/Runtime code sharing

The Builder and Runtime should share only carefully reviewed:

* cryptographic primitives,
* stable formats,
* protocol/type definitions where unavoidable.

They must not share:

* GUI,
* GitHub orchestration,
* Builder-only secrets,
* privileged local-client logic,
* unnecessary package-construction functionality.

The Runtime must remain minimal.

---

## 8. Keep `nyeda-crypto` as the smallest auditable confidentiality boundary

Preserve:

* `#![forbid(unsafe_code)]`,
* no I/O,
* no GUI,
* no platform dependency.

Prefer explicit dependency injection/interfaces for randomness where needed so deterministic test vectors remain possible without giving the crypto layer OS-specific responsibilities.

---

After resolving these points, update the architecture documentation before creating additional major implementation crates. Do not silently redesign NYEDA's user-facing requirements; these corrections concern internal security architecture and clarification of existing requirements.
