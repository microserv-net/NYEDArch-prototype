# ADR-0011 — Cross-platform verification on real runners

**Status:** accepted
**Category:** SECURITY / IMPLEMENTATION

Several code paths could not be executed in the development environment: the
macOS and Windows fingerprint providers, the native location providers, the
`.app` bundle, `build.bat`, and the platform key storage. They were written,
reviewed and shipped unverified.

They are now executed on real macOS, Windows and Linux runners.

## How, without publishing the source

Public runners are free, but pushing the workspace to a public repository would
have published work intended for patent filing. The project's own encrypted
source transport solved it: the workspace is packed, encrypted, and pushed as a
single blob, with the key held as a repository secret and decrypted only inside
the runner. The repository is public; the source in it is not readable.

Job logs could not be downloaded from this environment, so the workflow commits
a results file back to the repository, which is readable through the contents
API.

## What now runs on each platform

| Check | Linux | macOS | Windows |
|---|---|---|---|
| Full test suite | pass | pass | pass |
| Fingerprint captured | Strong (DMI UUID) | Strong (IOPlatformUUID) | Medium (MachineGuid) |
| Seal, build, run a capsule | identical | identical | identical |
| Wrong passphrase refused, no output | yes | yes | yes |
| Client key stable across separate runs | yes | yes | yes |
| Release build script | `build.sh` | `build.sh` | `build.bat` |
| Installer `--verify` | pass | pass | pass |
| `.app` bundle created and signed | — | yes | — |

A capsule had never before been built or run on macOS or Windows. It works, and
`build.bat` - previously shipped unexecuted - runs.

## Three defects found, all in code that had passed review

### 1. The client key was unstable on Windows (SEVERE)

`master_key()` minted a fresh key whenever the load failed, and the Windows DPAPI
store reported success without round-tripping. Every run therefore produced a
different key, which would have made every `.nyfp` record and every EULA
acceptance from a previous session unverifiable.

Fixed by treating the read-back as authoritative: a store that cannot return
what it was given is not a store.

### 2. Concurrent first-run initialisation raced (SEVERE)

With the read-back in place, macOS and Windows still disagreed. Several starters
each found no key, each minted one, and the last write won - so a record signed
by a loser was unverifiable.

Optimistic adoption narrowed the window without closing it: it converges only if
every read happens after every write, which concurrency does not guarantee.
Minting is now serialised on a lock file, with a stale-lock timeout so a starter
that dies cannot block the client forever. Verified by an eight-thread test and
on the two platforms where it appeared.

### 3. The archive dropped the executable bit

The source-transport format stored no permissions, so an unpacked tree could not
run its own scripts - which is why `build.sh` failed with 127 on the first run.
The format carries a flag byte now (magic bumped to `NYARCHIVE2`) and the
unlocker restores the bit.

## One bundle defect

The install manifest was written to the bundle root rather than under
`Contents/`, where everything an application owns belongs. Found by listing the
bundle on a real Mac. Fixed, with a test asserting nothing can be placed outside
`Contents/`.

## What is still not verified

The macOS location prompt. CoreLocation compiles and the bundle carries the
usage description, but a headless CI runner has no location services and no one
to grant permission. That needs a real Mac with a user in front of it.
