# Security test case studies and attacker model

Two things live here:

1. **Case studies** — the security-critical tests written as structured records
   rather than a list of names, so a reviewer can see what was actually done and
   what it proves.
2. **The attacker model** — how someone holding only a finished capsule would
   approach it, and what each class of attacker gets.

Every executed result carries evidence. Anything not executed is marked
**PROPOSED EXPERIMENT** and is never presented as a result.

---

# Part 1 — Case studies

## CS-01 — Patching the authorization branch

| Field | Value |
|---|---|
| **Category** | Authorization / anti-tamper |
| **Scenario** | An attacker holds a capsule, finds the authorization check, and removes it — the classic first attempt against any licence-style gate |
| **Setup** | Linux, release capsule, source available to the attacker for the purposes of the experiment (a strictly stronger position than reality) |
| **Preconditions** | Wrong machine; correct passphrase supplied |
| **Action** | The runtime/package binding check was deleted from the source and the capsule rebuilt |
| **Intended behaviour** | Removing the check must not produce plaintext, because the check is not what protects the payload |
| **Acceptance** | Extraction fails and the output directory is empty |
| **Observed** | Execution failed at `BootstrapPolicy` — earlier than the deleted check, and for a different reason: the composed key was wrong |
| **Artifact state** | Survived (not one-shot) |
| **Plaintext exposure** | **No** |
| **Status** | **PASS** |
| **Interpretation** | The branch is not the boundary. The payload key is composed from each protection's contribution, so removing a check removes nothing: the 32 bytes it would have supplied are still missing, and no amount of patching invents them |

## CS-02 — The timing oracle

| Field | Value |
|---|---|
| **Category** | Side channel |
| **Scenario** | An attacker measures how long a denial takes to learn *which* protection failed |
| **Setup** | Linux, release capsule, wall-clock timing over repeated runs |
| **Action** | Compare denial latency for a binding failure against a passphrase failure |
| **Observed (before)** | ~2.0 ms versus ~95–105 ms — a **~50x** gap, readable in a single run without statistics |
| **Interpretation (before)** | A genuine oracle. The generic "Authorization failed." message was undermined: an attacker learned whether they had cleared the machine protection, and therefore whether a passphrase search was worth mounting |
| **Remediation** | Constant-shape authorization: every protection is evaluated, key derivation always runs, and a failed protection contributes a **random decoy** instead of returning early |
| **Observed (after)** | ~105 ms versus ~117 ms — about **1.1x**, inside run-to-run noise |
| **Status** | **PASS** (fixed) |
| **Evidence** | Six denial causes asserted to produce identical state traces, plus the measurements above |
| **Residual** | An intermediate fix kept an `authorized` flag and still leaked, because flag failures stopped at payload authentication while a wrong passphrase ran on to decryption. The flag was removed from the control flow entirely |

## CS-03 — Bit-flip sweep across the artifact

| Field | Value |
|---|---|
| **Category** | Crypto / integrity |
| **Scenario** | An attacker edits the capsule byte by byte looking for a field that is parsed but not authenticated |
| **Action** | Every byte in a sealed package altered; extraction attempted after each |
| **Acceptance** | No modification yields plaintext, and none causes a crash |
| **Observed** | No unauthenticated byte anywhere. Two defects were found and fixed on the way there |
| **Defect 1** | `compression` and `chunk_size` were neither authenticated nor key inputs. A commitment over the entire header is now bound into every chunk |
| **Defect 2** | A tampered chunk count reached `Vec::with_capacity` and requested **51,539,607,576 bytes**, aborting the process. An abort is not a refusal — it is a denial of service that bypasses the fail-closed path. Lengths are bounded before any allocation |
| **Status** | **PASS** (after remediation) |
| **Interpretation** | Sampling is not enough. A sampled sweep passed on all platforms while an exhaustive one found the gap |

## CS-04 — Package transplantation

| Field | Value |
|---|---|
| **Category** | Binding |
| **Scenario** | Move a payload from one capsule into another, hoping the second's authorization is easier to satisfy |
| **Action** | Runtime A + package B; and A's container spliced with B's tail |
| **Observed** | Refused. Two independent mechanisms: an explicit binding check, and the runtime commitment folded into the policy-seal subkey |
| **Plaintext exposure** | **No** |
| **Status** | **PASS** |
| **Interpretation** | Even with the binding check deleted (CS-01) the subkey still differs, so the transplant fails cryptographically rather than by validation |

## CS-05 — Destruction under a hostile helper kill

| Field | Value |
|---|---|
| **Category** | Deletion |
| **Scenario** | A one-shot capsule extracts successfully; the attacker immediately kills the process that was going to scrub it |
| **Setup** | Real GitHub runners: Linux, macOS, Windows |
| **Action** | Run the capsule, kill detached helpers, wait, inspect the path |
| **Observed** | Destroyed on all three. Linux engaged a transient systemd job **and** a detached shell; Windows a self-removing scheduled task **and** a detached PowerShell; macOS completed in-process |
| **Artifact state** | Destroyed |
| **Status** | **PASS** |
| **Evidence** | A second hard link to the same inode proves the bytes were *overwritten*, not merely unlinked |
| **Residual** | Not a guarantee against an attacker who removes the scheduled job as well, cuts power mid-scrub, or copied the file first. `Delegated` is reported distinctly from `Removed`, because the requesting process cannot observe completion |

## CS-06 — Client key stability

| Field | Value |
|---|---|
| **Category** | Platform / key management |
| **Scenario** | A user runs the client repeatedly; records signed earlier must keep verifying |
| **Observed (Windows)** | **FAIL** — DPAPI reported a successful store and later returned nothing, so every run minted a fresh key and every previously signed `.nyfp` and licence record became unverifiable |
| **Observed (concurrency)** | **FAIL** — several starters on a fresh machine each minted a key; the last write won and the losers' records were invalid |
| **Remediation** | The read-back is authoritative — a store that cannot return what it was given is not a store. Minting is serialised on a lock file with a stale-lock timeout. Windows DPAPI is disabled in favour of a stable, disclosed restricted file |
| **Status** | **PASS** (after remediation) |
| **Interpretation** | An unreliable keystore is worse than an honest file. Silent key rotation destroys data while appearing to work |

## CS-07 — Encrypted source transport on a public repository

| Field | Value |
|---|---|
| **Category** | Build environment |
| **Scenario** | The build repository is public; what can a stranger read? |
| **Action** | Repository set public, build run, contents inspected |
| **Observed** | Workflow, README, unlocker, and a 179,404-byte blob containing no package magic and no readable source — a nonce followed by ciphertext. All three targets still built |
| **Status** | **PASS** |
| **Residual** | Not protection against the repository owner: whoever controls the repository controls its workflows, and anything a workflow can decrypt a modified workflow can print. The artifact is also the capsule binary, so it is deleted from GitHub after verified retrieval |

---

# Part 2 — Attacker model

The attacker receives **a finished capsule belonging to someone else**. They do
not receive the Builder, the source, internal repositories, or (in Prototype-II)
the License Server.

This section is **analytical modelling**, not observed attacker behaviour. It is
reasoning about incentives and cost, and is labelled as such so it is never
mistaken for evidence.

## What every attacker gets immediately

A single executable. Static analysis yields: the sealed package, the per-build
bootstrap key, the runtime commitment, the policy record structure, and all
code. What it does **not** yield is any protection contribution they cannot
satisfy — and the payload key needs all of them.

## By class

**Casual.** Runs it, is asked for a passphrase, guesses, gets
`Authorization failed.` Learns nothing about which protection failed. Stops.

**Technically skilled.** Finds strings and the embedded package, tries a wrong
passphrase repeatedly, perhaps copies the file first. Argon2id makes each guess
cost ~100 ms, so an online search is hopeless against any real passphrase.

**Intermediate reverse engineer.** Finds the authorization branch and patches it
— the CS-01 experiment. Gets nothing, because the branch was never the boundary.
This is the point where most stop, since the natural next step (iterate) is now
expensive.

**Professional.** Recognises the key-composition structure and works out that
they need the machine secret. On a held capsule, `S_machine` is recoverable —
this is the honest weakness, documented, and the reason hardware-backed identity
is the designated correction. They still need the passphrase, and location and
time if enabled.

**Elite / resourced team.** Extracts `S_machine`, then faces the passphrase
offline. Argon2id at 64 MiB is the wall. If location or time is enabled they must
also reproduce a region and a window. Their realistic path is not cryptanalysis
but the endpoint: the machine, the person, or the moment plaintext exists.

## Two framings that change the economics

### Race against time

An attacker faces several clocks at once: understanding the artifact; before a
live interaction becomes suspicious; before a failure destroys the specimen;
before the owner is notified (Prototype-II); before revocation. Long preparation
away from the artifact is possible — the meaningful live attack is short and
high-risk.

### Failure becomes loss

Normal reverse engineering assumes free retries: a wrong hypothesis costs time,
the artifact is unchanged. Against a one-shot capsule, a successful extraction
consumes the specimen, and destruction now survives the helper being killed.
Self-destruction is therefore an **anti-iteration mechanism**.

It is not an impossibility claim. It does not change the mathematics, and it
never reaches a copy made beforehand — asserted by test, precisely so the claim
cannot quietly inflate.

## The point where plaintext must exist

Extraction writes plaintext to disk. An attacker who wins every other round is
still racing the moment of extraction on a machine they control. Nothing in
Prototype-I changes that, and no claim here suggests otherwise.

## FUTURE — Prototype-II

**Yet to be developed.** Mandatory connectivity would add a factor absent from
the binary, make attempts visible, and allow revocation — turning a stolen
capsule from a permanently useful credential into one that can be switched off.
It cannot recall extracted plaintext, and it cannot see an attacker who never
contacts the server.
