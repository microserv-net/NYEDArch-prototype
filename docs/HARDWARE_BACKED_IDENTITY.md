# Hardware-backed machine protection

**Status: capability detection and policy — IMPLEMENTED AND TESTED.
Per-platform key operations — DESIGNED — NOT IMPLEMENTED.**

Tracked as item D1 in `PENDING_DOCUMENTATION_WORK.md`.

## The weakness being corrected

In the software-only design the per-machine factor secret `S_machine` lives
inside the capsule, encrypted under the policy-record key. Anyone who holds the
capsule and recovers the bootstrap key recovers `S_machine` as well. Machine
protection therefore raises effort but is **not a standalone confidentiality
boundary** — a limitation the dossier has always recorded.

## The design

Stop putting a recoverable secret in the capsule. Encrypt each machine's factor
secret to a key that lives inside that machine's secure hardware:

```
enrolment (once, on the trusted machine)
    secure hardware creates a non-exportable keypair
    the .nyfp record carries only the PUBLIC half
        │
build (on the creator's machine)
    S_machine is encrypted to that public key
    the capsule carries only the ciphertext
        │
runtime (on the trusted machine)
    the hardware performs the private-key operation
    S_machine is recovered and contributes to the payload key
```

An attacker holding the capsule then has ciphertext whose private key never left
the trusted machine. That is a boundary rather than an effort multiplier.

## What it must never claim

Secure hardware does not make machine identity unspoofable. It makes the private
key non-exportable *through supported interfaces*. It does not defend against:

- someone in possession of the machine while it is unlocked;
- physical attack on the part;
- a compromised kernel that can ask the hardware to perform the operation;
- a **virtual** TPM, whose state usually travels with the VM image.

## Capability states

Detection distinguishes four states, and only one of them is usable:

| State | Meaning |
|---|---|
| `Available` | TPM 2.0 present, version 2, and openable by this user; or an Apple Secure Enclave |
| `PresentButUnusable` | Reported but disabled, un-owned, not ready, or not permitted to this user |
| `Absent` | No hardware root |
| `Virtualized` | A VM — treated as no hardware root, because a virtual TPM's state can be copied |

`PresentButUnusable` exists because a half-initialised TPM protects nothing, and
counting it as present would produce exactly the false assurance this work is
meant to remove. On Linux the check opens `/dev/tpmrm0` for read/write: a device
node that cannot be opened is unusable, not available. On Windows all three of
`TpmPresent`, `TpmReady` and `TpmEnabled` must hold.

## Policy

| Policy | Hardware usable | Hardware not usable |
|---|---|---|
| `preferred` (default) | use it | continue, **and warn** |
| `required` | use it | **refuse the build** |

`nyedarch seal ... --hardware required` on the command line; the desktop client
uses `preferred` and reports any downgrade in its activity log.

The refusal is deliberate and its reason says why: a capsule built without
hardware backing carries a recoverable machine secret *while appearing to have
hardware protection*. Falling back silently would be worse than not offering the
feature at all.

The fallback warning states the consequence rather than the condition — not
"no hardware found", but that the machine secret is recoverable by someone
holding the capsule. A test asserts the warning contains that consequence.

## Verification status

The runners available for testing have **no usable secure hardware**, which was
established by probing rather than assumed:

| Platform | Probe result |
|---|---|
| `ubuntu-latest` | no `/dev/tpm*`, no sysfs TPM, no tpm2-tools |
| `windows-latest` | `TpmPresent: False`; Platform Crypto Provider present but "device not ready" |
| `macos-14` | `Apple M1 (Virtual)`, `VirtualMac2,1`, SIP disabled |

So the **positive** path cannot be verified on CI. What CI *can* verify is the
half that matters most for safety, and does: that a machine without usable
hardware refuses under `required`, never silently downgrades under `preferred`,
and that virtualisation is not mistaken for a hardware root.

**Requires real hardware to verify:** successful hardware-backed authorization,
behaviour after hardware replacement, secure-storage reset, and extraction
attempts. Those are listed in D1 and remain `DESIGNED — NOT IMPLEMENTED` until
they run on a machine with a real TPM or Secure Enclave.

## Open questions for the key operations

- **Linux:** TPM 2.0 via a persistent primary key. Requires tpm2-tools or a TSS
  binding; neither is currently a dependency.
- **Windows:** a Platform Crypto Provider key through NCrypt, reachable from
  PowerShell without adding a crate.
- **macOS:** Secure Enclave keys need the Security framework, so this shares the
  Objective-C surface already introduced for CoreLocation.
- **Recovery:** hardware replacement invalidates the machine factor by design.
  The recovery path is re-enrolment — and in Prototype-II, the verified recovery
  workflow. This is the same trade-off as the fingerprint changing when a
  machine changes, and must be documented for users before they rely on it.
