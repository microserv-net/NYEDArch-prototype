# ADR-0008 — Audit findings: local key storage and anti-RE coverage

**Status:** accepted
**Category:** SECURITY

## Why this audit happened

A sweep of the implementation against the specification, rather than against
memory of what had been built.

## Anti-RE coverage — confirmed present

| Layer | Mechanism |
|---|---|
| Generated capsule | Fat LTO, `codegen-units=1`, `panic=abort`, `strip`; tamper accumulator with debugger, timing and injection probes; deterministic package commitment |
| Client (CLI and desktop) | Fat LTO, `codegen-units=1`, `panic=abort`, `strip` (symbols 2,879 → 1); shared anti-analysis startup check |
| Authorization flow | Constant-shape evaluation; no branch left to patch (ADR-0007 addendum) |
| Per-build diversification | Build nonce and freshly minted constants in every generated capsule |

## Finding 1 — local key material was derived from a constant (SEVERE)

Spec §25/§44 require client private material to be held in platform-native
secure storage. It was instead derived from a string compiled into the binary:

```rust
hkdf_key(b"nyedarch:v1:nyfp:local", b"nyedarch-prototype-local-...", ...)
```

**Impact.** Anyone holding the binary could recompute the key that authenticates
portable `.nyfp` machine records and the EULA acceptance record, and forge
either. The `.nyfp` authentication tag — the thing that stops a record's labels
being edited — was worthless against anyone who had the client.

**Resolution.** A random 32-byte client master key is generated on first run and
stored in the platform keystore: Keychain on macOS, user-scoped DPAPI on
Windows, Secret Service on Linux. Every client secret is derived from it with
domain separation. No new dependency was added; each platform's own tooling is
used, and on Linux the secret reaches `secret-tool` on stdin so it never appears
in a process list.

**The fallback is disclosed, not silent.** A headless machine with no Secret
Service gets an owner-only file, and both clients say so at startup. That is
weaker than a keystore, and the specification's rule is that a weaker
substitution must never be made quietly.

### A second copy of the same defect

The CLI carried its *own* `nyfp_key()` alongside the library's, still using the
constant. So the first fix appeared to work while the CLI kept forging happily.
It was found by rotating the master key and observing that the derived record
key did not change — behaviour, not inspection. Duplicate removed; both clients
now call one implementation.

**Verified:** rotating the master key changes the derived record key; a `.nyfp`
signed under a different master key is refused with an authentication error and
no capsule project is produced; a record under the current key is accepted.

## Remaining gaps found, not yet closed

Recorded rather than quietly dropped:

1. **§12/§48 machine selector** — labels are stored and shown, but there is no
   search, tag filtering, ANY/ALL tag semantics, or "included machines: N"
   summary before building.
2. **§61 build cancellation** — a seal in progress cannot be cancelled.
3. **§50 creator/diagnostic mode** — exists in the capsule runtime, not in the
   clients.
4. **§16 compression modes** — a single codec is chosen; Automatic / Maximum /
   Balanced / Fast are not exposed.
5. **§9 hardware-backed identity** — TPM and Secure Enclave remain the
   designated fix for `S_machine` extractability and are not implemented.

## Addendum — gap 1 closed: the machine selector

Labels were stored but there was no way to search, filter, or select by them, and
no statement of what a build would actually include.

A shared registry now lives in `nyedarch_buildtool::machines`, so both clients
read and write the same set rather than each keeping its own idea of who is
trusted. Records are stored as authenticated `.nyfp` files, one per machine.

| Capability | Command line | Desktop |
|---|---|---|
| List, with filters | `machines list [query] [--tag T]... [--mode any\|all]` | Search box and tag chips |
| Import (verified) | `machines add <file.nyfp>` | Import button |
| Export | `export <file.nyfp> [labels...]` | Export button |
| Relabel | `machines label <id> <label>...` | via export/import |
| Remove | `machines remove <id>` | — |
| Select for a build | `seal ... --trust-tag T --tag-mode all` | Selection carries into the build |
| Included count | printed before sealing | "Included machines: N" |

**Two decisions worth recording.**

*Labels are re-signed locally.* They are metadata, not authorization secrets, but
they sit inside the record's authentication tag so a record cannot be relabelled
by anyone without the client key. Editing therefore re-signs with this client's
keystore key: the tag answers "did this client vouch for this record?", and after
an edit the answer must be yes for the edit too.

*This machine survives every filter.* A capsule always trusts its creator, so
hiding it behind a search would misrepresent what is about to be built. Asserted
by test.

*A record that fails verification is not listed at all*, rather than shown with a
warning: displaying it would imply it could be used, and it cannot.

Removing this machine is refused, because a capsule always includes its creator
and it would reappear immediately.

## Addendum — gaps 2, 3 and 4 closed

**§61 build cancellation.** The pipeline takes a shared cancellation flag,
checked between stages and between chunks. The desktop client shows a Cancel
button while a build runs.

The important part is what cancelling leaves behind: nothing. The capsule project
is now assembled under a `.partial` name and moved into place only when it is
complete, so a cancelled or failed build never leaves something that looks like a
usable project. Verified: after cancelling mid-build, neither the project
directory nor the staging directory exists.

**§50 creator mode.** A diagnostic mode in both clients - `--creator` on the
command line, a switch on the Build screen in the desktop client. It reports the
fingerprint signal inventory with strengths, where the client key is stored,
Argon2id timing, the compression level chosen, and seal timing.

It is diagnostics only, and both clients say so where it is used: it grants no
authority, skips no check, and weakens no capsule it produces. Signal *names and
strengths* are shown, never raw identifiers, which the client does not hold in
the first place.

**§16 compression modes.** Automatic, Maximum, Balanced and Fast, selectable from
both clients. The mode changes only how hard the compressor works; the codec is
recorded in the header either way, so a capsule built at any level opens
identically. Automatic resolves against payload size, because heavy compression
of a small input costs build time for almost nothing. A test asserts every mode
round-trips and that Maximum never produces a larger capsule than Fast.

## A duplicate implementation removed

Switching the command-line client to the shared pipeline revealed that `seal`
still had its **own** copy of the sealing logic, left over from before the
pipeline was extracted. That is the same pattern that caused the keystore defect:
two paths, one of them quietly stale.

It is gone. Both clients now call `seal_and_generate`, which is why cancellation,
compression modes and creator mode arrived in both at once rather than needing to
be written twice. The leftover imports the deletion exposed - `PolicyRecord`,
`compose`, `seal_package`, a private `rnd()` - are a good measure of how much
duplicated surface had accumulated.
