//! GitHub Actions workflow generation (spec §44). The workflow compiles the
//! generated runtime source for a chosen target and uploads the artifact. It
//! reads only repository secrets — no secret is ever echoed (spec §48: secrets
//! are masked; we never `echo` them).

/// Supported initial targets (spec §68).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    WindowsMsvc,
    MacosAppleSilicon,
    LinuxGnu,
}

impl Target {
    pub fn runs_on(self) -> &'static str {
        match self {
            Target::WindowsMsvc => "windows-latest",
            Target::MacosAppleSilicon => "macos-14",
            Target::LinuxGnu => "ubuntu-latest",
        }
    }
    pub fn rust_target(self) -> &'static str {
        match self {
            Target::WindowsMsvc => "x86_64-pc-windows-msvc",
            Target::MacosAppleSilicon => "aarch64-apple-darwin",
            Target::LinuxGnu => "x86_64-unknown-linux-gnu",
        }
    }
}

/// Generate the workflow YAML. `runtime_dir` is the path in the repo holding the
/// generated runtime crate.
pub fn workflow_yaml(targets: &[Target], runtime_dir: &str) -> String {
    let mut matrix = String::new();
    for t in targets {
        matrix.push_str(&format!(
            "          - {{ os: {}, target: {} }}\n",
            t.runs_on(),
            t.rust_target()
        ));
    }
    format!(
        r#"name: nyedarch-capsule-build
on:
  workflow_dispatch: {{}}
permissions:
  contents: read
jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
{matrix}    runs-on: ${{{{ matrix.os }}}}
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust (latest stable, chosen automatically)
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{{{ matrix.target }}}}

      # The capsule source is not stored in this repository in the clear. Only
      # this unlocker is, and it holds no secret: the key comes from a
      # repository secret and exists only in this runner's memory.
      - name: Build the unlocker
        working-directory: {runtime_dir}/unlock
        run: cargo build --release

      - name: Decrypt the capsule source
        working-directory: {runtime_dir}
        env:
          NYEDARCH_SOURCE_KEY: ${{{{ secrets.NYEDARCH_SOURCE_KEY }}}}
        shell: bash
        run: |
          if [ -z "$NYEDARCH_SOURCE_KEY" ]; then
            echo "The NYEDARCH_SOURCE_KEY repository secret is missing." >&2
            echo "Without it the capsule source cannot be decrypted." >&2
            exit 1
          fi
          ./unlock/target/release/nyedarch-unlock capsule.sealed .

      - name: Build capsule (release, hardened profile)
        working-directory: {runtime_dir}
        run: cargo build --release --target ${{{{ matrix.target }}}}

      - name: Stage capsule as .nyarch
        shell: bash
        working-directory: {runtime_dir}
        run: |
          mkdir -p ../out
          bin="target/${{{{ matrix.target }}}}/release/nyedarch-capsule"
          if [ -f "$bin.exe" ]; then bin="$bin.exe"; fi
          # One extension on every OS (spec: NYEDArch capsule identity).
          out="../out/nyedarch-${{{{ github.run_id }}}}-${{{{ matrix.target }}}}.nyarch"
          cp "$bin" "$out"
          # Executable bit. Preserved by upload-artifact v4 on Unix runners;
          # Windows has no execute bit, so this is a no-op there by design.
          chmod +x "$out" || true
          ls -l "$out"

      # The decrypted source must not outlive the build step, so that a later
      # step or a cached workspace cannot expose it.
      - name: Remove the decrypted source
        if: always()
        shell: bash
        working-directory: {runtime_dir}
        run: |
          rm -rf src vendor Cargo.toml Cargo.lock capsule.nyeda || true

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: nyedarch-capsule-${{{{ matrix.target }}}}
          path: out/*.nyarch
          if-no-files-found: error
"#
    )
}
