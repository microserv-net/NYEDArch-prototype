# NYEDArch — Anti-Reverse-Engineering Analysis

Status: normative for crypto version 1.

**NYEDArch is not unbreakable, and this document does not claim it is.** Every
mechanism below is bypassable by a competent analyst given the binary, time, and
tooling. The objective, stated in the specification's own words, is to
**raise the cost of analysis and bypass** — not to make it impossible.

---

## 1. Threat model

The attacker possesses the capsule executable and full knowledge of this
architecture (Kerckhoffs). They can copy, disassemble, debug, patch, instrument,
hook, snapshot memory, run repeatedly, manipulate the clock, run in a VM, and
inspect every syscall and file the capsule touches.

They do **not** possess: the passphrase, an authorized machine, or (when those
protections are enabled) the authorized region or time window.

## 2. The load-bearing claim

Everything in this document is secondary to one property:

> The payload key is composed from the contributions of every enabled
> authorization factor. A missing or wrong contribution yields a different key,
> and AEAD authentication fails.

**Consequence.** An attacker who defeats every mechanism in §4 still faces an
Argon2id-hardened passphrase factor and a per-machine secret they do not have.
Anti-RE buys time; cryptography is what actually protects the payload.

**Verified, not asserted.** The binding comparison was patched out of the
runtime source and rebuilt with a transplanted package. Result:

```
IntegrityCheck → BindingValidation → BootstrapPolicy → FailClosed
```

The patched branch was passed. Extraction still failed. See
`KEY_HIERARCHY.md §2`.

---

## 3. What an attacker recovers from a capsule, immediately

Stated plainly, because pretending otherwise would be dishonest:

| Recoverable | Consequence |
|---|---|
| `K_bootstrap`, `C_runtime` (embedded constants) | Can unseal the policy record |
| The policy record: trusted fingerprint **digests** + `S_machine` secrets | Obtains the machine factor for capsules they already hold |
| Package salt, id, policy flags, Argon2 parameters | Public by design |
| All code paths and state machine | Assumed public |

**Therefore machine authorization is not a standalone confidentiality
boundary.** For a capsule in hostile hands, the machine protection constrains *where*
it is convenient to use and forces the attacker to work from the capsule itself;
it does not stop them alone. The passphrase protection is what remains, and it is
deliberately memory-hard.

Closing this gap requires hardware-attested identity (TPM sealing / Secure
Enclave), so `S_machine` cannot be extracted by reading the binary. That is the
designated strengthening path and is **not** implemented in this prototype.

---

## 4. Implemented mechanisms, and how each is defeated

| # | Mechanism | Threat mitigated | Bypass | Residual value |
|---|---|---|---|---|
| 1 | Contribution-based key composition | Branch patching for payload access | None known short of obtaining real factors | **Primary. Load-bearing.** |
| 2 | Runtime commitment in policy-seal subkey | Package/runtime transplantation | Extract both constants from the target runtime | Forces per-capsule work; blocks bulk transplantation |
| 3 | AEAD AAD binding (context, chunk index, terminal flag) | Chunk reorder, truncation, header edits, section confusion | None without the key | Strong |
| 4 | `BindingValidation` state | Casual transplantation | Patch the comparison | Low alone — intentionally backed by #2 |
| 5 | Deterministic package commitment | Casual payload swapping | Recompute after edit | Low alone — backed by #3 |
| 6 | Tamper accumulator (debugger/timing/injection probes) | Casual dynamic analysis | Patch probes, hide debugger, strip markers | **Low.** Not key material, not a gate. Cost only |
| 7 | Per-build diversification (nonce, constants, generated source) | Write-once universal unpacker | Re-derive per capsule | Moderate: defeats scaling across capsules |
| 8 | Generated (not shipped) runtime source | Static template extraction from the client | Recover one capsule's source by decompilation | Moderate |
| 9 | Release profile: LTO, `codegen-units=1`, `strip`, `panic=abort` | Symbol-assisted analysis | Standard reversing | Low; raises effort |
| 10 | Generic failure message | Authorization oracle | Differential timing/state analysis | Moderate — see §5 |
| 11 | Protection-specific code omission | Attack surface | — | Capsule carries no code for disabled protections |
| 12 | `Zeroizing` on all secret material | Post-run memory scraping | Live memory capture mid-run | Moderate; see §6 |
| 13 | Fail-closed cleanup of partial output | Plaintext salvage after denial | — | Verified by 15 integration tests |
| 14 | Builder capability excluded from the runtime | Repurposing a capsule as a packer | — | Verified: `builder` feature off in shipped binary |

## 5. Known weaknesses (not mitigated in this prototype)

1. **Timing side channel across protections — MEASURED, then FIXED.**

   The state machine used to evaluate protections in order and return on the
   first failure, so the wall-clock time of a denial revealed how far execution
   reached:

   | Denial stage | Before | After |
   |---|---|---|
   | Binding or machine failure | **~2.0 ms** | ~105 ms |
   | Passphrase failure | ~95–105 ms | ~117 ms |
   | Ratio | **~50x** | **~1.1x, inside run-to-run noise** |

   **The fix.** Authorization is now constant-shape. Every protection is
   evaluated and key derivation always runs before any verdict is returned. A
   protection that fails contributes a *random decoy* of the right shape instead
   of returning early, so the run continues and fails at authenticated
   decryption exactly like every other denial.

   Two details matter. The decoys are random rather than fixed, because a
   constant would be a recognisable marker in memory and a reused one would let
   an attacker confirm which protection failed by watching for it. And there is
   no `if !authorized { fail }` anywhere: an intermediate version kept that flag
   and still leaked, because flag failures stopped at payload authentication
   while a wrong passphrase ran on to decryption. Every run now takes the same
   path.

   **The cost**, accepted deliberately: every denial pays the memory-hard
   passphrase cost. That work is what makes the oracle disappear.

   **Verified** by a test asserting that six different denials — wrong binding,
   wrong passphrase, absent passphrase, wrong region, absent location provider,
   and outside the time window — produce *identical* state traces, plus the
   wall-clock measurements above.

2. **`S_machine` extraction**, as detailed in §3.
3. **Environment probes are advisory only** (#6). They are honestly worth little
   against a prepared analyst.
4. **Location/time are low-entropy policy factors.** A determined attacker who
   knows the policy can enumerate cells and windows. They are never claimed to
   add meaningful entropy — only to require additional conditions be met.
5. **Control-flow flattening is not implemented.** Anti-hooking now is — see
   §5a below. Flattening remains listed in
   the specification as candidates; deliberately omitted rather than added as
   token gestures (§75: no security theater).
6. **Local system clock.** The time protection trusts the machine's clock, which the
   attacker controls. Authenticated time is `FUTURE — LICENSE SERVER`.

### 5a. Anti-hooking — IMPLEMENTED

Inline API hooking and dynamic instrumentation have to get code into the address
space, and on Linux that is visible without any privileged interface:

| Probe | Signal |
|---|---|
| `/proc/self/maps` | Mappings named for known instrumentation (Frida, Gum, DynamoRIO, Pin, Valgrind, ltrace, gdb) |
| `/proc/self/maps` | **Writable-and-executable** regions — normal loaded code does not need them; trampolines and JIT-based hooking engines do |
| `/proc/self/task/*/comm` | Threads belonging to instrumentation frameworks |

Both feed the tamper accumulator rather than a branch, so patching a single
comparison does not neutralise them, and no `unsafe` is required.

**Limits, stated plainly.** An engine that unmaps itself or renames its mappings
evades the name check; a legitimate JIT loaded into the host produces W+X
regions of its own; and this is a Linux-specific view — the other platforms fall
back to the environment-marker probe. It contributes observations, never a
verdict.

### 5b. The tamper accumulator was being discarded — FIXED

Found by audit, and worth recording plainly because it is the failure this
document is most at risk of: the accumulator was computed from six probes and
then thrown away (`let _diversified = acc.finish();`).

Every anti-analysis probe fed a number that went straight in the bin. That is
security theatre (§75), and **worse than having none**: in a review it reads as
a defence, so nobody looks again.

**What it does now.** It changes the *shape* of a run: how many decoy rounds the
authorization path performs, and the order of two independent integrity reads.
An analyst single-stepping an instrumented run sees a different trace from an
uninstrumented one, and traces differ between machines, so notes taken on one do
not transfer cleanly to another.

**What it must never do.** Gate anything, or become key material. Environment
observations are not reproducible — a debugger attached during a support call, a
virtual machine, a loaded host — so anything derived from them would eventually
refuse an honest user. That is a far worse failure than an analyst having an
easier afternoon. Two tests enforce this: one asserts the value is bound and
reaches the run's shape, the other that `if diversified` and equality
comparisons against it never appear.

**Honest scope.** This raises the cost of analysis. It stops nobody. The
confidentiality boundary is the composed key, as everywhere else in this
document.

## 6. Memory hygiene limits

`Zeroizing` clears buffers on drop and minimises secret lifetime. It does **not**
defeat: live memory inspection during a run, swap or hibernation files, core
dumps, or a hypervisor. Rust's memory safety does not eliminate forensic memory
recovery, and no such claim is made.

## 7. False-positive risk

Mechanisms #6 and #9 carry false-positive risk on virtualised, instrumented, or
heavily loaded machines. This is why the environment probes are **not** allowed
to deny authorization: a legitimate user on a busy VM must not be locked out of
their own data. Denial is reserved for actual authorization failure.

## 8. Performance impact

Hardening cost is dominated by one deliberate expense — Argon2id, which is
memory-hard *by design* (§27: expensive only where it serves a security
purpose). Probes and commitments are microseconds. Streaming
compress/encrypt is unaffected.

---

## 7a. Destruction as an anti-analysis layer

Self-destruction belongs in this document, not only in the deletion one, because
its security value is about **analysis cost** rather than erasure.

An analyst normally iterates freely against a captured artifact. Against a
one-shot capsule, a successful extraction consumes the specimen, and destruction
now runs through three redundant layers - in-process, an OS one-shot scheduler,
and a detached child - so terminating the helper does not preserve it.

| Property | Status |
|---|---|
| Raises the cost of repeated live experimentation | Yes |
| Survives the helper process being killed | Yes, via the OS scheduler |
| Guarantees the bytes are unrecoverable | **No** - see `SECURE_DELETION.md` |
| Reaches copies made beforehand | **No**, by definition |
| Substitutes for the cryptographic boundary | **No** - the payload is protected whether or not the file survives |

The honest summary: it makes analysis expensive and iteration risky. It does not
make the artifact unbreakable, and it is never described that way.

## 8a. Local key storage

Client secrets - the key authenticating portable `.nyfp` machine records and the
one authenticating the EULA acceptance record - are derived from a random
32-byte master key held in the platform keystore: Keychain on macOS, Secret
Service on Linux.

**Windows uses DPAPI again.** The first attempt went through PowerShell's
`ConvertTo-SecureString` / `ConvertFrom-SecureString`, which did not round-trip
on a real runner: a store reported success and a later load returned nothing, so
every run minted a fresh key and every previously signed record became
unverifiable.

It now calls .NET `ProtectedData` directly - the primitive underneath those
cmdlets, which takes and returns plain bytes rather than carrying SecureString
and console-encoding behaviour. The secret crosses on **stdin** in both
directions, never in a command line where another user could read it from the
process list.

The safety net matters more than the mechanism: `master_key` reads back what it
stored and falls through to the restricted file if the value does not return
identical. A keystore that cannot return what it was given is not trusted,
whatever its documentation says - which is why the original failure cost nothing
this time. Each secret is domain-separated, so
recovering one reveals nothing about another.

This replaced a constant compiled into the binary. Under the old scheme anyone
holding the client could recompute those keys and forge a machine record, which
made the `.nyfp` authentication tag worthless against exactly the person it was
meant to stop. See ADR-0008.

**The fallback is disclosed.** With no Secret Service available the master key
goes to an owner-only file and both clients say so at startup. Anything running
as that user can read it; that is weaker than a keystore and is reported rather
than assumed acceptable.

**Limit.** A keystore protects material at rest against another user or an
offline attacker. It does not protect against malware running as the user while
the client is unlocked.

## 9. The client is hardened too

Anti-RE is a global requirement, not a capsule-only one. The client holds the
local record-signing key, the EULA acceptance key, and the capsule generation
logic, so an attacker who can quietly modify it can weaken every capsule it
later produces.

| Measure | Effect |
|---|---|
| Release profile: fat LTO, `codegen-units=1`, `panic=abort`, `strip=symbols` | Exported symbols in the shipped client went from **2,879 to 1** |
| Client integrity accumulator | Debugger, injected-library and self-image observations, folded rather than branched on |
| Startup advisory | Warns when the session looks instrumented; **never denies an action** |

Both clients run the same check: the CLI prints a warning, the desktop client
writes it into its activity log.

**Deliberately advisory.** A false positive must not lock a user out of their
own data, and virtualised or instrumented machines are legitimate. The check
raises awareness and analysis cost; it is not a gate, and a competent analyst
defeats it.

**What still protects the client properly:** the OS keystore for private
material, the cryptographic construction in `nyedarch-crypto`, and the fact that
a capsule's payload key is composed from factors the client never stores.

## 10. Location providers

Order is always **native first, browser second**. The browser flow is a
fallback, never a replacement.

| Platform | Native provider | State |
|---|---|---|
| Linux | GeoClue2 over D-Bus (`gdbus`), legacy demo client as a second try | Implemented |
| Windows | System location service via `System.Device.Location.GeoCoordinateWatcher` | Implemented, dependency-free |
| macOS | CoreLocation (`CLLocationManager`) | Implemented, **requires an app bundle** - see below |

### The macOS bundling requirement

macOS refuses location to a process with no application bundle carrying
`NSLocationWhenInUseUsageDescription`. A bare command-line binary is denied no
matter how correct the code is. So a bundled, signed NYEDArch gets the native
provider; an unbundled build is denied and the browser flow takes over. This is
a platform rule, not a limitation of the implementation, and it is the reason
the browser flow was built.

`install` creates the bundle for you at `~/Applications/NYEDArch.app`, with the
usage description in place and an ad-hoc signature so macOS can remember a
permission decision. Launching from Applications is what surfaces the prompt;
running the CLI from a bare terminal falls back to the browser flow.

The CoreLocation binding sits behind the default-on `macos-corelocation`
feature, declared only under `[target.'cfg(target_os = "macos")']`, so no other
platform pulls Objective-C bindings. If it fails to build, disabling the feature
falls back to the browser flow and nothing else changes.

**Now verified.** The macOS path was compiled on a real macOS runner via the
GitHub build pipeline and produces a capsule. Doing so found two genuine errors
that had shipped unverified: `forbid(unsafe_code)` made every Objective-C call a
hard error, and a deprecated CoreLocation function was called without its
receiver. Both are fixed (ADR-0009). Unsafe code remains forbidden outright on
Linux and Windows; macOS uses `deny` with one narrowly scoped exception on the
CoreLocation module.

What is still unverified there is *runtime* behaviour: that the permission
prompt appears and a fix is returned on a real Mac with the bundle installed.

### Every provider fails closed the same way

A denial, a missing fix, or an accuracy that is absent, zero or non-finite all
produce no reading. None of them is quietly turned into a position, and the
user's privacy setting is always authoritative.

## 10a. Browser location fallback

Where no native provider exists, location can be acquired through the user's
browser (spec §19 permits this explicitly).

| Control | Purpose |
|---|---|
| Listener bound to `127.0.0.1` on an ephemeral port | Never reachable from another machine |
| 256-bit single-use token in both the page URL and the result path | Another local process cannot post a location by guessing the port |
| Loopback peer check | Defence in depth alongside the bind address |
| One-shot listener with a 120 s deadline | No long-lived local service |
| Reading refused when accuracy is absent or non-finite | "Unknown" is never treated as "good" |

**On IP-derived positions.** A browser may answer from a network estimate rather
than GNSS, and the API does not disclose which. NYEDArch cannot detect the
source — but it enforces the accuracy the user demanded, and network or
IP-derived fixes report accuracy in the thousands of metres, so they fail the
gate for any realistic tolerance. A capsule requiring 150 m can never be
satisfied by an IP lookup.

**Honest limit.** The browser runs on the machine the user controls. This flow
lets an honest user on a Mac or a headless Linux box use the location protection
at all; it does not make the reading trustworthy against its own operator.

---

## FUTURE — NYEDArch License Server integration

**Prototype-II — yet to be developed.** Nothing in this section exists today.
It is recorded here so each limitation above states what will change, rather
than trailing off into "fixed later".

| Prototype-I behaviour | Future integration | What it changes | What still remains |
|---|---|---|---|
| Time comes from the local clock | Authenticated server time | The clock stops belonging to whoever holds the machine | Network delay and skew still have to be tolerated honestly |
| A capsule authorizes entirely locally | Server participates in key composition as a further factor | Patching a boolean cannot substitute for material the binary never contained | The server must never hold plaintext or the passphrase |
| An issued capsule is valid forever | Central revocation and a minimum-version floor | A known-vulnerable or stolen capsule can be refused | It cannot recall plaintext already extracted |
| Attempts leave no durable record | Authenticated capsule event reporting | An attacker leaves a trace they do not control | Nothing is reported by a capsule that never reaches the server |
| Nobody is told about misuse | Owner notification and anomaly detection | Discovery in days rather than never | Detection is probabilistic, and false positives must be recoverable |
| A lost machine means lost data | Verified, narrowly scoped recovery | A legitimate owner is not punished for hardware failure | It must never become a master unlock |

**Why this cannot be built now, rather than merely not yet:** several of these
guarantees have nowhere to live inside a capsule running alone on a hostile
machine. Anything it writes locally, the attacker can delete; any clock it
reads, the attacker can set. See `PHILOSOPHY.md` §4.

**Design rule this imposes on Prototype-I:** never build anything whose
correctness depends on the server being absent. The package format already
carries `crypto_version`, package format version and a runtime commitment, which
is the seam a future minimum-version floor needs. Removing those fields would
put every capsule issued before Prototype-II permanently outside the scheme.
