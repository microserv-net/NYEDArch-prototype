# ADR-0006 — Build script portability

**Status:** accepted
**Category:** IMPLEMENTATION
**Trigger:** `unbound variable` errors reported from `build.sh`.

## Root cause

`build.sh` runs under `set -euo pipefail`. Argument parsing used:

```bash
for arg in "$@"; do ...
```

Under `nounset`, **bash 3.2 treats an empty `"$@"` as an unbound variable** and
aborts. Bash 3.2 is still the default `/bin/bash` on macOS, so the script failed
immediately there when invoked with no arguments — the most common invocation.

## Four further portability defects found in the same review

Rather than fix only the reported symptom, the script was audited for GNU-only
assumptions. All four would have failed on macOS and some on minimal Linux
images:

| Defect | Problem | Fix |
|---|---|---|
| `for arg in "$@"` under `set -u` | Aborts on bash 3.2 with no arguments | Positional `while [ "$#" -gt 0 ]` loop with `${1:-}` |
| `sha256sum` | Absent on macOS, which ships `shasum` | Detect either; skip checksums with a warning if neither exists |
| `bc` | Not installed on many minimal images | Sum test counts with `awk` |
| `tar --null -T -` | GNU-only; BSD tar rejects it | `cp -R` plus explicit removal of `target/` and `.git` |
| `sort -z` / `xargs -0` | GNU extensions | Plain `find | sort` with a `read` loop |

## A second failure, from the same class

After the first fix, `build.sh` still aborted on the reporter's machine:

```
./build.sh: line 47: B<bytes> unbound variable
```

The banner used box-drawing characters directly after a variable reference:

```bash
printf '%s\n' "$B┌────────┐$RST"
```

Some bash builds treat high-bit bytes as **identifier characters**, so `$B`
followed by a UTF-8 box character is parsed as a variable named `B` plus those
bytes -- which is unset. It did not reproduce under bash 5 with a UTF-8 locale,
which is why the first pass missed it.

Two changes, because one alone would have left the risk:

1. Every variable reference is braced: `${B}`, not `$B`.
2. `build.sh` is now **pure ASCII** (verified: zero bytes above 0x7F). This also
   removes a second defect -- the banner and status glyphs would have rendered
   as mojibake in a C-locale terminal.

The same audit was applied to printed output in the Rust binaries. The installer
and client emitted `✓`, `✗`, and em-dashes, which display as mojibake in the
legacy code page cmd.exe uses. All printed strings are now ASCII; doc comments
are untouched, since they are read in an editor rather than rendered by a
console.

## Verification

- `build.sh` with no arguments, with each flag, and with an unknown flag.
- `build.sh` under `LC_ALL=C`, with no `TERM`, and non-interactive.
- Zero non-ASCII bytes in `build.sh` and `build.bat`, and none in any printed
  string in the installer, client, or generated capsule.
- Generated `SHA256SUMS` verifies with **both** `sha256sum -c` and
  `shasum -a 256 -c`, so the file is consumable on either platform.
- Full installer lifecycle re-tested against the regenerated release: verify,
  install, use with the media deleted, uninstall clean.

## Note

`build.bat` could not be executed — no Windows host was available. It is written
to mirror `build.sh` stage for stage and should be treated as unverified until
run; the `certutil` checksum loop is the most likely place for a defect.


## Addendum — the benchmark step could block forever

`build.sh` ran the client's benchmark with stdout redirected but stdin
inherited. The client gates on the licence, so on a machine where the licence
had not yet been accepted the build hung indefinitely at a prompt nobody was
watching. Found by running the script on a machine whose acceptance record had
been cleared during testing.

Fixed by redirecting stdin from `/dev/null` for that step, so an unaccepted
licence produces a warning and the build continues instead of stalling.
