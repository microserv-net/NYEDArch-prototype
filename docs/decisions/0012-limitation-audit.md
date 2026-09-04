# ADR-0012 — Audit: limitations that were documented instead of solved

**Status:** accepted
**Category:** SECURITY

Prompted by a fair challenge: *how many things were written up as limitations
when a workaround existed?* Every documented limitation was re-examined and
sorted into "solvable now" and "genuinely blocked".

## Solved in this pass

### 1. A running capsule could not scrub itself

**Was:** "a process cannot open its own running image for writing, so the
capsule is unlinked but its bytes may remain in free space." Documented and left.

**Now:** the scrub is delegated to a process that outlives the capsule, passed
inline to the system shell so nothing is written to disk. Verified at the inode
via a surviving hard link — the bytes really are replaced, not just the name
removed. See `SECURE_DELETION.md`.

Writing it exposed a worse bug in the obvious ordering: unlink-then-delegate
succeeds at the unlink on Unix even for a running image, so the helper arrived to
find nothing and scrubbed nothing, while the user was told the capsule was
destroyed. The overwrite is now attempted first and the file handed over intact.

### 2. Anti-hooking was listed as not implemented

**Was:** named in the spec §31 technique list and recorded as absent.

**Now:** implemented without `unsafe` — instrumentation-named mappings and
writable-and-executable regions from `/proc/self/maps`, plus instrumentation
thread names from `/proc/self/task/*/comm`, folded into the tamper accumulator
rather than branched on. See `ANTI_RE_ANALYSIS.md` §5a.

The compiler caught these probes sitting unused before they were wired in, which
is exactly the failure one of the new tests exists to prevent: a probe whose
output is discarded is decoration.

## Genuinely blocked, with the reason

| Item | Why it cannot be closed here |
|---|---|
| **TPM / Secure Enclave key operations** | No runner has usable secure hardware — established by probing, not assumed. Capability detection and fail-closed policy are implemented and tested; the key operations need a real TPM or Secure Enclave |
| **macOS location prompt** | Needs a Mac with a user present to grant permission. CoreLocation compiles and the bundle carries the usage description |
| **Control-flow flattening** | Needs either a custom codegen pass or an obfuscating toolchain. A real change of scope, not a workaround |
| **Copies made before destruction** | Outside the reach of any user-space program, by definition. The cryptography protects the copy; a test asserts the copy survives rather than implying otherwise |
| **Physical erasure guarantees** | SSD wear-levelling, CoW snapshots and journals are below the filesystem interface. No user-space program can promise this, and NYEDArch does not |

## The standard applied

A limitation is acceptable only when it is a property of the environment rather
than a gap in effort. "The OS will not let a process do this" is a limitation.
"This would take work" is not — and two of the items above turned out to be the
second kind wearing the first kind's clothes.
