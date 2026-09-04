# NYEDArch — End User Licence Agreement

**Version 1.0.** You must read and accept this agreement before using NYEDArch.
Acceptance is recorded locally with the EULA version, a timestamp, the
application version, and an integrity tag over that record.

## 1. What NYEDArch does

NYEDArch packages your files into a self-contained executable capsule that unlocks
only when its configured authorization protections are satisfied.

## 2. System information NYEDArch collects and why

To build a machine fingerprint, NYEDArch reads platform identifiers that may
include firmware/board identifiers, platform UUIDs, TPM presence, operating
system install identity, CPU model, and hostname.

**Some of this information is sensitive.** NYEDArch stores only salted
cryptographic digests of these values, never the raw identifiers, and does not
transmit them anywhere. Exported fingerprint records (`.nyfp`) contain digests
and your chosen labels, and are authenticated so they cannot be edited undetected.

## 3. GitHub

**Remote building is part of how NYEDArch works.** Capsules are compiled by
GitHub Actions, so using NYEDArch means NYEDArch creates and operates a
repository in **your** GitHub account, using a token you supply.

- **Private repositories are recommended and are the default.**
- **A public repository exposes your generated runtime source and build logs to
  anyone.** Do not use a public repository for sensitive work.
- Secrets are stored as GitHub Actions repository secrets, encrypted to the
  repository's public key before transmission. They are never committed to Git
  and never printed in workflow output.
- GitHub is treated as an untrusted build environment. Artifacts are verified
  against a build commitment before use.

## 4. Local key storage and the risk of permanent loss

NYEDArch stores private key material using your platform's secure storage
(Windows DPAPI/Credential Manager, macOS Keychain, Linux Secret Service).

> **Loss of required private key material may make previously generated
> artifacts permanently unrecoverable.** There is no recovery mechanism, no
> backdoor, and no ability for anyone — including the developers — to restore
> access. Back up your profile.

## 4a. Capsule files

Finished capsules use the `.nyarch` extension on every operating system. On
Linux and macOS a capsule runs directly from a terminal. **On Windows, typing
`capsule.nyarch` into cmd or PowerShell will not run it**, because Windows
resolves executables by extension; launch it from the NYEDArch client, or add
`.NYARCH` to `PATHEXT`.

The client can launch a capsule you drag onto it. The launched capsule runs as
an independent process and performs its **own** authorization — the client
grants it nothing and never handles its passphrase.

## 5. Location

If you enable the location protection, NYEDArch requests your location through your
operating system's location service, with your permission, at build time and
again when the capsule runs. Coordinates are converted into a quantized region
identifier; raw coordinates are not stored in the capsule or logged. If your
device reports accuracy worse than your configured tolerance, authorization
fails rather than proceeding.

## 6. Time

The time protection uses the local system clock. **The local clock can be changed by
whoever controls the machine.** The time protection is a policy control, not a
tamper-proof expiry mechanism. Authenticated time is planned future work.

The time protection is also **recurring and day-invariant**: a schedule of "every day
at 14:00" permits opening at that time on any day. It does not expire a capsule.

## 7. Self-destruction and deletion limits

If you enable one-shot execution, the capsule attempts to delete itself after a
verified extraction.

> **Software cannot guarantee secure deletion.** On SSDs, journalled,
> copy-on-write, or snapshotted filesystems, data may remain recoverable after
> deletion. NYEDArch's security does not depend on deletion succeeding.

## 8. No guarantee of absolute security

NYEDArch raises the cost and difficulty of unauthorized access. It does **not**
provide absolute security and is not unbreakable. An attacker in possession of a
capsule can analyse it, and can extract some material embedded within it —
see `ANTI_RE_ANALYSIS.md §3` for an explicit account of what is recoverable.

Do not rely on NYEDArch as your only control for information whose disclosure would
cause severe harm.

## 9. Your responsibilities

You are responsible for choosing a strong passphrase, for keeping key material
safe, for choosing appropriate repository visibility, for lawful use, and for
maintaining independent backups of any data you seal.

## 10. Data loss

Sealing does not back up your data. If you lose your passphrase, your authorized
machine, or your key material, **the sealed data is unrecoverable.**

## 11. Lawful use

You may use NYEDArch only for lawful purposes and only on data you are entitled to
protect.

## 12. Warranty

Provided "as is", without warranty of any kind, to the maximum extent permitted
by law.
