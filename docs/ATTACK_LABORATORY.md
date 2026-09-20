# Attack laboratory

**Status: IMPLEMENTED AND TESTED** — 13 executed attacks, item B2 in
`PENDING_DOCUMENTATION_WORK.md`.

Every attack here is **executed**, not proposed. Each takes a genuinely sealed
package, modifies it the way an attacker holding the artifact would, and records
what the implementation does. The property under test throughout:

> modifying the artifact must never yield plaintext — it must yield a refusal,
> and that refusal must not depend on a branch that could be patched away.

Each attack checks two things, because a refusal that still wrote files would be
a failure: whether extraction succeeded, **and** whether the payload marker
reached disk anywhere under the output directory.

## Register

| ID | Attack | Result |
|---|---|---|
| A0 | **Control** — unmodified specimen | Opens, produces payload |
| A1 | Ciphertext bit flip | Refused |
| A2 | Truncation at 90/70/50/20 % | Refused |
| A3 | Header field modification | Refused |
| A4 | Policy-flag downgrade | Refused |
| A5 | Chunk reordering | Refused |
| A6 | Key differing in one bit | Refused |
| A7 | Payload transplantation and splicing | Refused |
| A8 | Appended trailing data | No influence on output |
| A9 | Degenerate inputs (empty, zeroes, magic only) | Refused, no panic |
| A10 | **Bit-flip sweep** across the whole artifact | No single-bit edit anywhere yielded plaintext |
| A11 | Length fields set to maximum | Refused without allocating |
| A12 | **Exhaustive sweep** — every byte altered | No unauthenticated byte anywhere in the artifact |

A0 exists because without it the register would be worthless: a package that
never opened would "resist" every attack trivially.

## Two real defects the laboratory found

### 1. Part of the header was not authenticated

The payload AAD bound `crypto_version`, `package_id`, `runtime_binding` and the
policy flags — but **not** `compression`, `chunk_size`, the salt, or the Argon2
parameters. The bit-flip sweep found a header byte that could be changed while
the package still opened and produced plaintext.

Most of those fields feed key derivation, so tampering with them fails closed by
accident rather than by design. `compression` and `chunk_size` did neither: not
authenticated, not key inputs.

**Fixed** by binding a commitment over the entire serialized header into the
chunk AAD. Any change to any header field, now or in a future version, alters
the AAD and every chunk fails to authenticate.

### 2. A tampered length field could request 51 GB

The chunk count is read from the artifact and was passed straight to
`Vec::with_capacity`. A single flipped byte made the process attempt a
51,539,607,576-byte allocation and abort.

An abort is not a refusal. It is a denial of service, and it bypasses the
fail-closed path entirely — the capsule dies instead of denying.

**Fixed** by rejecting any count larger than the remaining bytes could hold —
every chunk costs at least a 4-byte prefix — before any allocation, and the same
bound now applies to the header length.

## Two defects in the laboratory itself

Recorded because a test that passes for the wrong reason is worse than no test.

**Specimens shared an identity.** Fixed constants meant two "different" capsules
derived the same key, so the transplantation test failed for a reason that had
nothing to do with the product. Identity is now derived from the specimen tag.

**Output directories collided under parallel execution.** Named by nanosecond
timestamp, two tests starting in the same instant shared a directory and one
read another's output, reporting a leak that had not happened. Now an atomic
counter.

## A false positive, and what it cost to be sure

`a11` failed on Windows while passing elsewhere:

```
byte 148 set to 0xFF still opened the package
```

It looked like an unauthenticated byte in the artifact, and was recorded as an
open defect rather than dismissed.

**It was a defect in the test.** `a11` set each byte to `0xFF` without checking
whether that changed anything. On Windows the manifest differs — path handling
produces different bytes — so offset 148 already held `0xFF`. Setting it was a
no-op, the package was unmodified, and it opened correctly.

Diagnosed by adding **A12, an exhaustive sweep**: every byte in the package
altered with `wrapping_add(1)`, which always changes the value. It found **no
unauthenticated byte anywhere**, which is a stronger statement than A10's
sampling or A11's first-160-bytes scan could make.

`a11` now skips bytes already holding the target value. Two lessons kept in the
record:

* a sampled sweep can miss what an exhaustive one finds, and A10 samples;
* a mutation that does not mutate proves nothing, and a test that fails for a
  reason unrelated to the product is worse than no test.

## What this register does not cover

Attacks on the *runtime binary* rather than the package — patching branches,
hooking, debugger attachment — are covered separately in `ANTI_RE_ANALYSIS.md`
and the authorization tests. Attacks requiring the Builder, source, or the
future License Server are out of scope for an attacker holding only a capsule.
