#!/usr/bin/env bash
#
# NYEDArch Framework - one-command build.
#
#   ./build.sh                 build everything, run tests, produce ./release
#   ./build.sh --skip-gui      skip the desktop GUI (needs system display libs)
#   ./build.sh --skip-tests    build only (not recommended)
#   ./build.sh --skip-bench    skip the benchmark run
#   ./build.sh --clean         remove previous build artifacts first
#
# Produces ./release/ containing everything intended for an end user.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RELEASE="$ROOT/release"

SKIP_GUI=0; SKIP_TESTS=0; SKIP_BENCH=0; CLEAN=0

# Parsed with a positional loop rather than `for arg in "$@"`: under `set -u`,
# bash 3.2 (still the default /bin/bash on macOS) treats an empty "$@" as an
# unbound variable and aborts before doing anything.
while [ "$#" -gt 0 ]; do
  case "${1:-}" in
    --skip-gui)   SKIP_GUI=1 ;;
    --skip-tests) SKIP_TESTS=1 ;;
    --skip-bench) SKIP_BENCH=1 ;;
    --clean)      CLEAN=1 ;;
    -h|--help)    sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown option: ${1:-} (try --help)" >&2; exit 2 ;;
  esac
  shift
done

# ----------------------------------------------------------------- output ----
if [ -t 1 ]; then
  B=$'\033[1m'; DIM=$'\033[2m'; GRN=$'\033[32m'; YLW=$'\033[33m'; RED=$'\033[31m'; RST=$'\033[0m'
else
  B=""; DIM=""; GRN=""; YLW=""; RED=""; RST=""
fi
step()  { printf '\n%s==> %s%s\n' "${B}" "$1" "${RST}"; }
ok()    { printf '    %s[ok]%s %s\n' "${GRN}" "${RST}" "$1"; }
warn()  { printf '    %s[!]%s %s\n' "${YLW}" "${RST}" "$1"; }
fail()  { printf '    %s[X]%s %s\n' "${RED}" "${RST}" "$1"; }
die()   { fail "$1"; exit 1; }

# Kept ASCII on purpose. A variable reference followed directly by a non-ASCII
# byte can be parsed as part of the variable NAME on some bash builds, which
# produced an "unbound variable" abort here. Braces are used everywhere for the
# same reason.
printf '%s\n' "${B}==============================================${RST}"
printf '%s\n' "${B}  NYEDArch Framework - build${RST}"
printf '%s\n' "${B}  Not Your Everyday Archive${RST}"
printf '%s\n' "${B}==============================================${RST}"

# --------------------------------------------------------------- toolchain ---
step "Checking toolchain"
command -v cargo >/dev/null 2>&1 || die "cargo not found. Install Rust: https://rustup.rs"
RUSTC_V="$(rustc --version)"
ok "$RUSTC_V"

# Warn (do not fail) if older than the workspace rust-version.
RV_MAJOR=$(rustc --version | sed -E 's/rustc ([0-9]+)\.([0-9]+).*/\1/')
RV_MINOR=$(rustc --version | sed -E 's/rustc ([0-9]+)\.([0-9]+).*/\2/')
if [ "$RV_MAJOR" -eq 1 ] && [ "$RV_MINOR" -lt 82 ]; then
  warn "workspace targets Rust >= 1.82; this toolchain is older and may fail"
fi
command -v curl >/dev/null 2>&1 && ok "curl present (HTTP transport for remote builds)" \
  || warn "curl not found - remote GitHub builds will not work until it is installed"

# macOS ships `shasum`, not GNU `sha256sum`. Pick whichever exists.
SHA256_CMD=""
if command -v sha256sum >/dev/null 2>&1; then
  SHA256_CMD="sha256sum"
elif command -v shasum >/dev/null 2>&1; then
  SHA256_CMD="shasum -a 256"
fi
[ -n "$SHA256_CMD" ] && ok "checksum tool: ${SHA256_CMD}" \
  || warn "no sha256sum/shasum found - SHA256SUMS will be skipped"

if [ "$CLEAN" -eq 1 ]; then
  step "Cleaning previous artifacts"
  cargo clean --manifest-path "$ROOT/Cargo.toml" 2>/dev/null || true
  [ -d "$ROOT/gui/nyedarch-gui" ] && cargo clean --manifest-path "$ROOT/gui/nyedarch-gui/Cargo.toml" 2>/dev/null || true
  rm -rf "$RELEASE"
  ok "cleaned"
fi

# ------------------------------------------------------------------ tests ----
TEST_SUMMARY="skipped"
if [ "$SKIP_TESTS" -eq 0 ]; then
  step "Running test suite"
  TEST_LOG="$(mktemp)"
  if cargo test --manifest-path "$ROOT/Cargo.toml" >"$TEST_LOG" 2>&1; then
    # awk rather than bc: bc is absent from many minimal images.
    PASSED=$(grep -oE '^test result: ok\. [0-9]+' "$TEST_LOG" \
             | grep -oE '[0-9]+$' | awk '{n+=$1} END {print n+0}')
    TEST_SUMMARY="${PASSED} passed, 0 failed"
    ok "$TEST_SUMMARY"
  else
    tail -30 "$TEST_LOG"
    rm -f "$TEST_LOG"
    die "tests failed - refusing to produce a release from a failing build"
  fi
  rm -f "$TEST_LOG"
else
  warn "tests skipped (--skip-tests)"
fi

# ------------------------------------------------------------------ build ----
step "Building workspace (release profile)"
cargo build --release --manifest-path "$ROOT/Cargo.toml"
ok "client and libraries built"

step "Verifying the cryptographic core builds with zero platform surface"
if cargo build --release -p nyedarch-crypto --no-default-features \
     --manifest-path "$ROOT/Cargo.toml" >/dev/null 2>&1; then
  ok "nyedarch-crypto builds with --no-default-features (auditable in isolation)"
else
  die "crypto core failed to build without default features"
fi

GUI_BUILT=0
if [ "$SKIP_GUI" -eq 0 ] && [ -d "$ROOT/gui/nyedarch-gui" ]; then
  step "Building desktop client (this takes several minutes)"
  if cargo build --release --manifest-path "$ROOT/gui/nyedarch-gui/Cargo.toml"; then
    GUI_BUILT=1
    ok "desktop client built"
  else
    warn "desktop client failed to build."
    warn "It needs system display libraries. On Debian/Ubuntu:"
    warn "  sudo apt install libgtk-3-dev libxkbcommon-dev libwayland-dev pkg-config"
    warn "Continuing - the command-line client is fully functional without it."
  fi
elif [ "$SKIP_GUI" -eq 1 ]; then
  warn "desktop client skipped (--skip-gui)"
fi

# -------------------------------------------------------------- benchmarks ---
BENCH_FILE=""
if [ "$SKIP_BENCH" -eq 0 ]; then
  step "Measuring performance on this machine"
  BENCH_FILE="$(mktemp)"
  # stdin from /dev/null: the client gates on the licence, and without this the
  # benchmark step blocks forever waiting for a prompt nobody is watching.
  if "$ROOT/target/release/nyedarch-buildtool" bench >"$BENCH_FILE" 2>&1 </dev/null; then
    ok "benchmarks recorded"
  else
    warn "benchmark run did not complete (the licence may not be accepted yet); continuing"
    BENCH_FILE=""
  fi
fi

# ----------------------------------------------------------------- release ---
step "Assembling release directory"
rm -rf "$RELEASE"
mkdir -p "$RELEASE"/{bin,docs,docs/license-server,docs/decisions}

# Binaries. The client is installed as `nyedarch`.
cp "$ROOT/target/release/nyedarch-buildtool" "$RELEASE/bin/nyedarch"
chmod +x "$RELEASE/bin/nyedarch"
ok "bin/nyedarch (command-line client)"

# The installer sits at the TOP of the release directory, not in bin/, because
# it is the first thing a user runs and should be impossible to miss.
cp "$ROOT/target/release/nyedarch-install" "$RELEASE/install"
chmod +x "$RELEASE/install"
ok "install (run this first)"

if [ "$GUI_BUILT" -eq 1 ]; then
  cp "$ROOT/gui/nyedarch-gui/target/release/nyedarch-gui" "$RELEASE/bin/nyedarch-gui"
  chmod +x "$RELEASE/bin/nyedarch-gui"
  ok "bin/nyedarch-gui (desktop client)"
fi

# Runtime sources, vendored into every generated capsule project.
# Without these the client can seal a package but cannot produce a capsule on a
# machine that has no NYEDArch source tree.
# `cp -R` rather than a tar pipe: GNU tar's --null/-T combination is not
# accepted by the BSD tar that ships with macOS.
mkdir -p "$RELEASE/runtime-src"
for c in nyedarch-crypto nyedarch-core nyedarch-package nyedarch-fingerprint nyedarch-platform nyedarch-runtime; do
  cp -R "$ROOT/crates/$c" "$RELEASE/runtime-src/$c"
  rm -rf "$RELEASE/runtime-src/$c/target" "$RELEASE/runtime-src/$c/.git"
done
ok "runtime-src/ (6 crates, vendored into generated capsules)"

# Documentation.
cp "$ROOT"/docs/*.md "$RELEASE/docs/" 2>/dev/null || true
cp "$ROOT"/docs/decisions/*.md "$RELEASE/docs/decisions/" 2>/dev/null || true
cp "$ROOT"/docs/license-server/*.md "$RELEASE/docs/license-server/" 2>/dev/null || true
cp "$ROOT/README.md" "$RELEASE/" 2>/dev/null || true
[ -f "$ROOT/docs/EULA.md" ] && cp "$ROOT/docs/EULA.md" "$RELEASE/EULA.md"
# The workflows travel with the release so the repository can be reconstructed.
if [ -d "$ROOT/.github" ]; then
  mkdir -p "$RELEASE/github-workflows"
  cp -R "$ROOT"/.github/workflows/*.yml "$RELEASE/github-workflows/" 2>/dev/null || true
  ok "github-workflows/ (ci, security, release)"
fi

if [ -d "$ROOT/docs/ui" ]; then
  mkdir -p "$RELEASE/docs/ui"
  cp "$ROOT"/docs/ui/*.png "$RELEASE/docs/ui/" 2>/dev/null || true
fi
ok "docs/ ($(ls "$RELEASE/docs"/*.md 2>/dev/null | wc -l) documents, \
$(ls "$RELEASE/docs/license-server"/*.md 2>/dev/null | wc -l) future-architecture)"

for f in NYEDArch_Technical_Documentation.docx NYEDArch_Technical_Documentation.pdf; do
  [ -f "$ROOT/$f" ] && cp "$ROOT/$f" "$RELEASE/" && ok "$f"
done

[ -n "$BENCH_FILE" ] && cp "$BENCH_FILE" "$RELEASE/BENCHMARKS.txt" && rm -f "$BENCH_FILE"

# ------------------------------------------------------------- build record --
GIT_REV="$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo 'not a git checkout')"
{
  echo "NYEDArch Framework - build record"
  echo "================================="
  echo
  echo "Built:      $(date -u '+%Y-%m-%d %H:%M:%S UTC')"
  echo "Host:       $(uname -s) $(uname -m)"
  echo "Toolchain:  $RUSTC_V"
  echo "Source rev: $GIT_REV"
  echo "Tests:      $TEST_SUMMARY"
  echo "Desktop UI: $([ "$GUI_BUILT" -eq 1 ] && echo 'built' || echo 'not built')"
  echo
  echo "Install"
  echo "-------"
  echo "  ./install              install for the current user (no admin rights needed)"
  echo "  ./install --verify     check integrity without installing"
  echo "  ./install --uninstall  remove a previous installation"
  echo
  echo "Contents"
  echo "--------"
  echo "  install           installer (start here)"
  echo "  bin/nyedarch      command-line client (seal, build, run, export, bench)"
  [ "$GUI_BUILT" -eq 1 ] && echo "  bin/nyedarch-gui  desktop client with drag-and-drop capsule launcher"
  echo "  runtime-src/      runtime crates vendored into generated capsules"
  echo "  docs/             architecture, security, threat model, crypto format, EULA"
  echo "  docs/license-server/  future architecture - design only, not implemented"
  echo "  docs/decisions/   architecture decision records"
  echo "  SHA256SUMS        checksums for everything in this directory"
  echo
  echo "Verify"
  echo "------"
  echo "  sha256sum -c SHA256SUMS      # or: shasum -a 256 -c SHA256SUMS"
  echo
  echo "Note: these checksums confirm the files arrived intact. They are not a"
  echo "substitute for signed releases, and they prove nothing if fetched from"
  echo "the same place as the files themselves."
} > "$RELEASE/BUILD_INFO.txt"
ok "BUILD_INFO.txt"

# ------------------------------------------------------------ quick-start ----
cp "$ROOT/release-assets/START_HERE.md" "$RELEASE/START_HERE.md"
ok "START_HERE.md"

# ---------------------------------------------------------------- checksums --
step "Generating checksums"
if [ -n "$SHA256_CMD" ]; then
  # A plain read loop: `sort -z` and `xargs -0` are GNU extensions that BSD
  # userland does not provide. Paths inside the release contain no newlines.
  ( cd "$RELEASE" \
      && find . -type f ! -name SHA256SUMS | LC_ALL=C sort > /tmp/.nyedarch_files.$$ \
      && : > SHA256SUMS \
      && while IFS= read -r f; do
           $SHA256_CMD "$f" >> SHA256SUMS
         done < /tmp/.nyedarch_files.$$ \
      && rm -f /tmp/.nyedarch_files.$$ )
  ok "SHA256SUMS ($(wc -l < "$RELEASE/SHA256SUMS" | tr -d ' ') files)"
else
  warn "SHA256SUMS skipped (no checksum tool available)"
fi

# ------------------------------------------------------------------ summary --
step "Done"
printf '    Release directory: %s%s%s\n' "${B}" "$RELEASE" "${RST}"
printf '    Size: %s   Files: %s\n' "$(du -sh "$RELEASE" | cut -f1)" "$(find "$RELEASE" -type f | wc -l)"
printf '    Tests: %s\n' "$TEST_SUMMARY"
[ "$GUI_BUILT" -eq 0 ] && printf '    %sDesktop client not included - see messages above%s\n' "${YLW}" "${RST}"
printf '\n    Give the whole %srelease/%s directory to the end user.\n' "${B}" "${RST}"
printf '    They should read %sSTART_HERE.md%s first.\n\n' "${B}" "${RST}"
