# ADR-0007 — GUI/CLI parity, browser location, client hardening

**Status:** accepted
**Category:** ARCHITECTURE / SECURITY

## The defect

The desktop client's build screen was a **simulation**: a timer drove the
progress bar and the log was canned. Nothing was sealed, no output was written,
no GitHub token was requested, and no location was acquired. It was reported by
the user, who reasonably asked why no token or location prompt appeared.

The root cause was architectural, not cosmetic: the sealing pipeline lived
inside the CLI **binary**, so the GUI could not call it, and the gap was filled
with a simulation instead of a refactor. That was the wrong call — an interface
that reports progress must be reporting real work, and in a security product a
fake progress bar is worse than a missing feature.

## Resolution

1. **Pipeline extracted to a library.** `nyedarch-buildtool` is now lib + bin.
   `pipeline::seal_and_generate` is the single implementation; the CLI and GUI
   both call it, so no second path can drift.
2. **Genuine progress.** Stages are reported by the pipeline as work begins
   (`Stage::CapturingFingerprint` … `Stage::Done`), not by a timer. Sealing runs
   on a worker thread because Argon2id is deliberately slow.
3. **Real output.** The GUI asks where to write the capsule project and reports
   the path, byte count and build nonce.
4. **Browser location fallback**, for all platforms — see `ANTI_RE_ANALYSIS.md`
   §10. Native provider first, browser consent flow second, and never a typed
   coordinate or an IP lookup.
5. **Client hardening** — see `ANTI_RE_ANALYSIS.md` §9. Symbols in the shipped
   client dropped from 2,879 to 1.

## Verified

- Loopback protocol driven end to end: page served, **wrong token refused with
  404**, correct token accepted, reading returned.
- Pipeline driven exactly as the GUI drives it: produced a project, which
  compiled to a `.nyarch`, extracted byte-identically, and refused a wrong
  passphrase.
- 61 tests pass, zero warnings; CLI unaffected by the refactor.

## Still outstanding

- **Remote GitHub build from the GUI.** The CLI path is real and API-verified;
  the GUI currently produces the capsule project for local staging. Not claimed
  as done.
- **macOS CoreLocation.** Not implemented; the browser fallback now covers that
  platform.

## Addendum — remote build reaches parity

The GUI now dispatches the GitHub build itself, on the same worker thread as the
seal, so the window stays responsive.

**A blocker had to be removed first.** The libsodium sealed-box sealer sat
behind the `net` feature, which also pulled a full HTTP stack. Since GitHub will
not accept a repository secret that is not encrypted to the repository's public
key, remote build could never have succeeded without it — the client would have
failed closed every time. The sealer now has its own `sealer` feature, on by
default, pulling only `crypto_box`. The placeholder `RequireRealSealer` is gone.

Verified by test: a sealed secret round-trips with the matching private key and
the ciphertext never contains the plaintext; a malformed or wrong-length public
key is refused.

**Credentials.** Owner, repository and token are entered under Targets, or
pre-filled from `NYEDARCH_GITHUB_TOKEN` / `GITHUB_TOKEN` exactly as the CLI does.
The token is held in memory for the session only, is never written to disk, and
reaches curl on stdin so it cannot appear in the process list.

**Failure behaviour.** With remote build enabled but credentials missing, the
build refuses and says so rather than proceeding quietly. If the remote build
fails after a successful seal, the capsule project is kept and the log says it
can be staged locally — a network failure must not destroy completed work.

## Addendum — remaining parity gaps closed

**Licence gating.** The CLI gated on the EULA; the desktop client did not, which
meant one interface enforced a requirement the other ignored. The acceptance
record and its authentication now live in the shared library, and the GUI blocks
the entire window until the licence is accepted - no menus, no steps, no
fingerprint work. Declining closes the window rather than leaving a half-usable
client.

Verified: accepting in the GUI writes the authenticated record, and the CLI then
runs without re-prompting. One gate, one record, one authentication scheme.

**Trusted machine import and export.** These were stubs that only wrote to the
log. Both are now real, using the same `.nyfp` container and key as the CLI:

- Import verifies the record's tag **before** trusting anything in it, refuses a
  duplicate, and refuses an edited record outright.
- Export writes an authenticated record for this machine.
- Imported machines are passed to the sealer **as file paths**, so the pipeline
  re-verifies each record itself rather than trusting what the interface
  believes about it.

Verified: a `.nyfp` exported by one client imports into the other, and flipping
a single byte causes the sealer to refuse it with an authentication error.


## Addendum — native location on all three platforms

| Platform | Implementation | Verified here |
|---|---|---|
| Linux | GeoClue2 via `gdbus`: get client, set `DesktopId`, start, poll for a location object, read latitude/longitude/accuracy | Parsers tested; no GeoClue service in the build environment |
| Windows | `System.Device.Location.GeoCoordinateWatcher` driven through PowerShell - no crate dependency, so no compile risk | Parser tested, including that `DENIED`/`UNKNOWN`/`ERROR` never become a reading |
| macOS | CoreLocation via `objc2-core-location`, polling `CLLocationManager.location` rather than implementing a delegate and run loop | **Not compiled** - no macOS host |

**Design decisions worth recording.**

*Windows through PowerShell rather than a WinRT crate.* It keeps the crate
dependency-free and removes any chance of shipping code that fails to build on a
platform that could not be tested here. The honest failure mode is preserved: if
location is switched off or the app is denied, the watcher never reaches Ready
and the browser flow takes over.

*macOS without a delegate.* The usual pattern needs a delegate object and a live
run loop. Reading `CLLocationManager.location` and polling briefly is a much
smaller surface for the same result and cannot hang - it gives up and returns
`None`.

*macOS behind a default-on feature.* The binding is declared only under
`[target.'cfg(target_os = "macos")']`, so no other platform pulls Objective-C
crates, and `--no-default-features` disables it if it fails to build. This is
the least-verified code in the project and is labelled as such.

**Unchanged:** the browser consent flow remains the universal fallback, and no
provider ever produces a reading from a denial, a missing fix, or an absent
accuracy.


## Addendum — macOS application bundle

`install` now produces a real application bundle on macOS at
`~/Applications/NYEDArch.app`, per-user so no administrator rights are needed.
Other platforms keep the flat release layout unchanged.

```
NYEDArch.app/
  Contents/
    Info.plist                     declares the location usage description
    PkgInfo
    MacOS/NYEDArch                 desktop client (CFBundleExecutable)
    MacOS/nyedarch                 command-line client
    MacOS/install                  installer, so uninstall survives the media
    Resources/runtime-src/         crates vendored into generated capsules
    Resources/docs/                documentation
    Resources/...                  EULA, guide, checksums, dossier
```

**Why this matters.** macOS refuses location to a process with no bundle
carrying `NSLocationWhenInUseUsageDescription`. Installing as a bundle is what
makes CoreLocation reachable at all; run from a bare terminal and location falls
back to the browser consent flow. The plist states what the location is used for
rather than merely claiming a right to it.

**Ad-hoc signing.** The bundle is signed with `codesign --sign -` after layout.
This is **not** Developer ID signing or notarisation and is not a substitute for
them; it gives the bundle a stable code identity so macOS can remember a
permission decision instead of re-prompting or refusing. Failure to sign is a
warning, not an error.

**Supporting changes.**

- `CFBundleExecutable` must match the file in `Contents/MacOS`, so the desktop
  binary is renamed to `NYEDArch` during install.
- `runtime_source_root()` also looks in `../Resources/runtime-src`, which is
  where the vendored crates land inside a bundle.
- The post-install symlink and PATH guidance resolve to `Contents/MacOS` on
  macOS rather than `bin/`.

**Verified.** Four tests cover the plist and the layout mapping: the usage
descriptions are present and explain the purpose, `CFBundleExecutable` matches
the bundle name, the plist is well-formed with balanced dicts, and no item can
land outside `Contents/`. The mapping was also run over a real release directory
to inspect the resulting tree. The Linux install path is unchanged: installed,
sealed a capsule, and uninstalled cleanly afterwards.

**Not verified:** the bundle has never been built or launched on macOS.
