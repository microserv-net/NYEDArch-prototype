# NYEDArch — Fingerprint Specification

## 1. Purpose

A fingerprint identifies a machine state well enough that the same machine
reproduces the same identifier, while exposing no raw hardware identifiers.

## 2. Adaptive collection

Platforms expose different identifiers, so NYEDArch does **not** define a fixed
field schema (spec §7). Each platform contributes whatever trustworthy signals
are actually available, each tagged with a strength:

| Strength | Meaning | Examples |
|---|---|---|
| `Strong` | Firmware/hardware/TPM-backed | DMI product UUID, board serial, TPM presence, macOS IOPlatformUUID, Windows csproduct UUID |
| `Medium` | OS install identity | `/etc/machine-id`, Windows MachineGuid |
| `Weak` | Present but not machine-unique | CPU model, hostname |

### Platform status

- **Linux** — implemented and exercised. DMI values require read permission;
  when unreadable they are simply absent.
- **macOS** — `ioreg` IOPlatformUUID and CPU brand. Real code; requires macOS.
- **Windows** — MachineGuid and csproduct UUID. Real code; requires Windows.

## 3. Identifier derivation

Raw values are never stored. Each signal is reduced to
`SHA-256(domain ‖ "|sig|" ‖ name ‖ "|" ‖ raw)`.

Signals are sorted by name for determinism, then the identifier is derived from
all signals at or above a threshold: `Medium` when any Medium-or-better signal
exists, otherwise `Weak`.

**Graceful degradation with disclosure (spec §9).** A machine with no strong
signals still produces a usable fingerprint, and the record retains the full
signal inventory with strengths, so a weaker capture is visible rather than
silently substituted for a strong one.

## 4. Machine-state semantics

The fingerprint reflects machine state at capture. Motherboard replacement,
firmware changes, or OS reinstallation may legitimately change it, and the
capsule will then deny that machine. This is intended. Recovery for changed
machines is `FUTURE — LICENSE SERVER`.

## 5. Multiple fingerprints are OR

A package may trust several fingerprints; any one match authorizes. Each trusted
fingerprint receives its **own independent** `S_machine` secret, so extracting
one capsule's record does not reveal another machine's material.

The creator's fingerprint is always included (spec §12).

## 6. `.nyfp` portable records

Versioned, non-human-readable, and HMAC-SHA512 authenticated:

```
[u16 version][u32 body_len][body][64-byte MAC]
```

The body carries the fingerprint, its signal inventory, labels, creation time,
and app version. **Labels are metadata, not authorization secrets** — but they
are inside the MAC, so editing `HR` to `Administrator` invalidates the record.
Verified by test (tamper and wrong-key both rejected).

## 7. Authorization role, stated honestly

The fingerprint selects a high-entropy secret; it never *becomes* a key. An
attacker who unseals a capsule's policy record obtains those secrets — so the
machine protection constrains convenience and scope, and is not a standalone
confidentiality boundary. See `ANTI_RE_ANALYSIS.md §3`. Hardware-attested
binding is the strengthening path and is not implemented.

## 8. Why import, export, labelling and search exist

The fingerprint management facilities are deliberately built to serve two very
different scales with one mechanism.

**Individual.** A person owns one or two machines and adds them directly. Labels
are barely needed.

**Enterprise.** The trusted set is a policy decision across many machines, and
labels become the means of expressing that policy — `HR`, `Bangalore`,
`Employee-A`, `Finance`, `Laptop`. Search and multi-tag selection (ANY or ALL
semantics) exist so an operator can assemble a trusted set from thousands of
registered machines without hand-picking identifiers.

### Worked example: HR employee records

An organisation seals employee records into a capsule whose trusted fingerprint
set contains:

- the registered company machines of HR personnel, and
- the single machine of the employee the record belongs to.

The location protection is set to the office premises, and the time protection to working
hours.

The result composes as:

```
(HR machine OR that employee's machine)
    AND passphrase
    AND inside the office
    AND during working hours
```

The record therefore cannot be opened outside the office, outside working hours,
or on any machine outside that set — **including by the organisation's own
executives and administrators**, who are simply not in the fingerprint list.

This addresses insider and outsider exfiltration in a single construction. A
server-mediated DRM system typically cannot achieve the insider half, because the
administrator who controls the policy server can usually grant themselves access.
In NYEDArch the constraint is cryptographic and travels with the capsule.

**Honest scope:** this restricts *opening the capsule*. It does not prevent an
authorized HR user, having legitimately opened the record inside the office
during working hours, from then copying what they see. NYEDArch controls access
to the sealed artifact, not what an authorized human does afterwards.

FUTURE — NYEDArch LICENSE SERVER INTEGRATION: centralized revocation would allow
the organisation to disable a capsule after an employee leaves, which Prototype-I
cannot do.
