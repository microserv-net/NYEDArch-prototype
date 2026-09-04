# ADR-0013 — Full security and documentation audit

**Status:** accepted
**Category:** SECURITY / DOCUMENTATION

A systematic pass over the whole system rather than the area last worked on.
Findings are listed whether or not they were exploitable, because "not currently
reachable" is a property of today's code and not of tomorrow's.

## Findings and fixes

### 1. Secret material could reach a formatter (LATENT)

`PolicyEntry` derived `Debug` and holds `factor_secret` — the per-machine
contribution to the payload key. `PolicyRecord` derived `Debug` over a vector of
them. Any `{:?}` in an error, a debug log, or a panic message would have printed
every trusted machine's secret in full.

Nothing formatted them today. That is not a defence: §57 forbids secret material
reaching a formatter at all, and a later edit adding `dbg!` would have leaked
silently.

**Fixed** with hand-written redacting `Debug` impls — the fingerprint id shows as
`<opaque>`, the secret as `<redacted>`, labels still shown because they help
diagnose, and the record shows only an entry count. A test asserts the secret
appears in neither hex nor decimal form.

### 2. Help text described behaviour that did not exist

The CLI help said the passphrase could be supplied in `NYEDARCH_PASSPHRASE`
"which does not appear in the process list". That variable is read by the
*capsule*, not the client — the client only accepted a positional argument, so
following the advice would have failed.

A help text that drifts from the parser is worse than none in a security tool:
it tells the operator something untrue about how their secret is handled.

**Fixed** by making the behaviour real: the passphrase argument is now optional
and falls back to the environment, with a clear refusal if neither is present. A
test asserts the parser accepts the omission the help promises.

### 3. §60 violations in capsule-side code

`restore.rs` used `.unwrap()` on a file handle and the fingerprint MAC used
`.expect("hmac key")`. A panic inside a capsule is a denial of service in a
hostile environment and its message can disclose paths.

**Fixed** — both return typed errors. "Cannot fail in practice" is exactly the
reasoning that leaves an `expect` in a security path.

### 4. Dead code in a security-critical comparison

`Fingerprint::matches` carried an unused `PhantomData<U32>` and a bare comment.
Removed, and replaced with an explanation of *why* the comparison is written the
way it is.

## Verified as already correct

Recorded so a later reviewer does not have to re-derive them:

| Property | Finding |
|---|---|
| Fingerprint id comparison | Constant time — XOR accumulate over all 32 bytes, no early return |
| `.nyfp` MAC verification | Uses `verify_slice`, which is constant time |
| Trusted machine selection | Iterates **every** entry with no early exit, so timing does not reveal which machine matched or how many were scanned |
| Passphrase handling | Never written to disk, never logged, never placed in an environment variable by us |
| Capsule dependency isolation | The generated capsule does not enable the `builder` feature, so package-construction capability cannot be reached from an artifact |
| Crypto crate boundaries | Still no I/O, no platform code, no GUI; randomness injected |
| Overclaims in documentation | Every occurrence of "unbreakable" is a denial, not a claim |
| License Server references | All labelled `Prototype-II — yet to be developed` |

## A known interaction, stated rather than hidden

`panic = "abort"` means destructors do not run on a panic, so `Zeroizing` values
are **not** wiped if the process aborts. The process dies immediately and the OS
reclaims its pages, and the security paths are written not to panic — but the
interaction is real and is recorded here rather than left for someone to
discover.

The alternative, unwinding, would let a panic propagate through partially
completed authorization state, which is worse. Abort is the deliberate choice.

## Documentation audit

- One stale test count corrected.
- Every document referenced by another exists.
- Every claim in `COMPONENT_REFERENCE.md` was checked against the source rather
  than written from memory: domain-separation labels, `PolicyRecord::select`,
  the DEFLATE codec id, `window_id_for_slot`.
- Structured case studies and the attacker model added (`CASE_STUDIES.md`), with
  the attacker section labelled as analytical modelling rather than evidence.
- Future-integration sections woven into the Threat Model, Security Architecture
  and Anti-RE analysis, each stating what changes **and what still remains**.
