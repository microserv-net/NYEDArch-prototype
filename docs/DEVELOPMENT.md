# NYEDArch — Development Guide

## Build

```bash
./build.sh          # Linux / macOS — test, build, benchmark, assemble ./release
build.bat           # Windows — same stages
```

Flags (both): `--skip-gui`, `--skip-tests`, `--skip-bench`, `--clean`.
The script refuses to produce a release from a failing test suite.

`build.sh` targets POSIX-portable shell and works with the bash 3.2 that ships
with macOS. It uses `sha256sum` or `shasum`, whichever exists, and avoids
GNU-only flags (see ADR-0006). `build.bat` has not been executed on Windows.

Or drive cargo directly:

```bash
cargo build --workspace
cargo test  --workspace
```

### Release layout

```
release/
  install            platform-independent installer (install.exe on Windows)
  bin/nyedarch       command-line client
  bin/nyedarch-gui   desktop client, when built
  runtime-src/       6 runtime crates, vendored into generated capsule projects
  docs/              architecture, security, EULA, license-server/ (design only)
  START_HERE.md  BUILD_INFO.txt  SHA256SUMS
```

`runtime-src/` is not optional: generated capsule projects vendor those crates so
they compile on machines with no NYEDArch source tree, including the GitHub
Actions runner. The client locates it via `NYEDARCH_RUNTIME_SRC`, then beside or
above its own executable, then the compile-time workspace for development.

Toolchain: latest stable Rust (developed and verified on **1.91.1**; workspace
`rust-version = 1.82`). The early 1.75-era version pins have been removed — see
ADR-0005.

## Auditing the confidentiality boundary

```bash
cargo build -p nyedarch-crypto --no-default-features   # zero platform surface
```

`nyedarch-crypto` forbids unsafe code, performs no I/O, has no GUI or platform
dependency, and takes randomness by injection.

## Crates

| Crate | Role | Ships in capsule |
|---|---|---|
| `nyedarch-crypto` | Key composition, AEAD, KDF, geo/time factors | yes |
| `nyedarch-core` | Ids, policy, protected policy record | yes |
| `nyedarch-package` | Format, streaming pipeline, restore | yes (`builder` feature **off**) |
| `nyedarch-fingerprint` | Adaptive fingerprinting, `.nyfp` | yes |
| `nyedarch-platform` | Location acquisition | yes |
| `nyedarch-runtime` | Authorization state machine, hardening | yes |
| `nyedarch-buildtool` | Sealing/generation CLI | no |
| `nyedarch-github` | Build orchestration, provenance, curl transport | no |
| `gui/nyedarch-gui` | Desktop client (separate workspace) | no |
| `nyedarch-installer` | Per-user installer shipped in the release | no |

`nyedarch-buildtool` is both a library and a binary. The sealing pipeline lives
in the library (`pipeline::seal_and_generate`) so the CLI and the desktop client
share one implementation; there is deliberately no second path for the GUI.

## Prototype CLI

```bash
# Export this machine as a trusted, authenticated record
nyedarch-buildtool export alice.nyfp HR Bangalore

# Seal with every protection
nyedarch-buildtool seal ./secret_docs ./capsule_project "passphrase" \
    --time 14:00 --time-tolerance-min 15 --tz-offset-min 330 \
    --location 150 --trust alice.nyfp --one-shot

cd ./capsule_project && cargo build --release

# Stage it as a .nyarch capsule (same identity the GitHub build produces)
sh ./stage-capsule.sh

# Remote build on GitHub Actions (required stage of the real flow)
export NYEDARCH_GITHUB_TOKEN=...        # read from env, never argv
nyedarch-buildtool build ./capsule_project <owner> <repo> private

# Run a finished capsule as an independent process (CLI form of drag-and-drop)
nyedarch-buildtool run ./nyedarch-<id>.nyarch ./out

# Measured performance
nyedarch-buildtool bench
```

## GUI

```bash
cd gui/nyedarch-gui && cargo run --release
```

Separate from the root workspace: it needs a display and windowing/GPU
dependencies, and is kept out so it cannot destabilise the verified core.

## Testing conventions

Security features are not "done" when they compile. Each needs positive tests,
negative tests, and a leak assertion where plaintext is involved.
`crates/nyedarch-runtime/tests/authorization.rs` is the model: every denial asserts
both refusal and that nothing was written.
