# ADR-0009 — Live GitHub pipeline verification

**Status:** accepted
**Category:** SECURITY / IMPLEMENTATION
**Trigger:** A real GitHub account was made available for testing.

Until now the remote build had been verified only in parts: the transport
against the live API, the sealed-box sealer cryptographically, and the
orchestration against a mock. A complete authenticated build had never been
observed. It has now, and it found three defects that no amount of local
testing would have surfaced.

## What was run

A capsule was sealed, the project pushed to a private repository, secrets sealed
and stored, the workflow dispatched, and the build followed to completion for
all three targets.

| Target | Result | Artifact |
|---|---|---|
| `x86_64-unknown-linux-gnu` | success | 339,771 bytes |
| `x86_64-pc-windows-msvc` | success | 251,280 bytes |
| `aarch64-apple-darwin` | success | 287,454 bytes |

## Defect 1 — the macOS build did not compile (SEVERE for that platform)

The first run failed on macOS while Linux and Windows passed. The CoreLocation
binding had never been compiled anywhere, and it was wrong in two ways:

1. `nyedarch-platform` carries `#![forbid(unsafe_code)]`. Objective-C calls
   require `unsafe`, and **`forbid` cannot be lifted locally** - so every
   `unsafe` block was a hard error.
2. `CLLocationManager::locationServicesEnabled()` needs a receiver and is
   deprecated; calling it as a free function failed with E0061.

**Resolution.** The unsafe prohibition is now conditional: `forbid` everywhere
except macOS, which uses `deny` plus one narrowly scoped `allow` on the
CoreLocation module. Linux and Windows keep the absolute guarantee. The
deprecated call was removed - authorization status plus the polling loop already
covers "switched off" as well as "refused".

**Verified:** the macOS job now compiles and produces a capsule.

## Defect 2 — the second build into a repository always failed

The specification calls for one persistent repository holding many builds. Every
build after the first failed with HTTP 422, because the contents API rejects an
update that does not carry the file's current blob SHA and the client always
sent `sha: None`.

**Resolution.** `put_or_update` fetches the existing SHA and includes it; a 404
simply means the file is new. Repeat builds into the same repository now
succeed - confirmed by running three.

## Defect 3 — GitHub's error messages were discarded

Failures surfaced as `unexpected status 422` with no explanation, which turned a
one-line fix into a debugging exercise. The client now carries GitHub's own
`message` and `errors` fields through, so a refusal says what was wrong.

## How the macOS error was obtained

The sandbox cannot reach `results-receiver.actions.githubusercontent.com`, so
job logs could not be downloaded. A temporary workflow was pushed that captured
the compiler output and committed it back into the repository, which is readable
through the contents API on an allowed host. The diagnostic workflow and its
output were deleted afterwards; only the NYEDArch workflow remains.

## Credential handling

The test credential was read from the environment for the duration of the run
only. It is not written into any source file, any generated capsule, the release
directory, or the deliverable archive - verified by searching the workspace and
release for the value. The only PAT-shaped string in the tree is a deliberately
fake one in the test that asserts tokens never reach `argv`.

**Recommendation:** treat that credential as exposed and revoke it. Any secret
that has sat in plaintext in shared context should be rotated regardless of how
carefully it was handled afterwards.
