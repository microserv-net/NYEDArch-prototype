# ADR-0010 — The capsule source is never pushed in plaintext

**Status:** accepted
**Category:** SECURITY

## The problem

The remote build needs the generated capsule source on GitHub, and that source
is not neutral. It contains the sealed package, the per-build bootstrap key and
the runtime commitment. Pushing it in the clear meant:

* a **public** repository handed all of it to anyone;
* a **private** repository exposed it to every collaborator, to anyone later
  granted read access, and to every clone and cached copy;
* it stayed in the repository's history indefinitely.

## The design

Everything except a small, secret-free unlocker is packed into a single archive,
encrypted with XChaCha20-Poly1305 under a fresh random key, and pushed as
ciphertext. The key is stored as a **GitHub Actions repository secret**, sealed
to the repository's own public key, so it is readable only inside a runner.

```
repository:                       runner (build time):
  runtime/capsule.sealed   ---->    build the unlocker
  runtime/unlock/**        ---->    decrypt with $NYEDARCH_SOURCE_KEY
  .github/workflows/...             build the capsule
                                    delete the decrypted source
```

The push dropped from **39 files to 3**.

The unlocker is committed in the clear on purpose. It contains no secret: only
the ability to open an archive given a key it does not hold. Reading it teaches
an observer the archive format and nothing else. It refuses path traversal and
treats any authentication failure as fatal, so a modified archive cannot produce
a subtly wrong source tree.

The workflow removes the decrypted source in an `if: always()` step, so
plaintext does not outlive the build step even when the build fails.

## What this protects, and what it does not

**Protected:** the repository's contents, history and clones no longer carry the
capsule source. Someone with read access sees an opaque blob and a bootstrap
crate.

**Not protected:** the repository owner. This is the stated trust model - whoever
controls the repository controls its workflows, and anything a workflow can
decrypt, a modified workflow can print. The change moves the exposure from
"public by default" to "requires control of the build environment". That is the
honest description and it is not stronger than that.

## Two defects found by running it

1. **The unlocker would not build.** It sits inside `runtime/`, and a
   `Cargo.toml` left there by an earlier build made cargo treat it as a
   non-member of that workspace. The unlocker now declares its own
   `[workspace]`.

2. **Stale files were never removed.** A repository is reused across builds, so
   every earlier build's files accumulated - including, on any repository used
   before this change, a full plaintext capsule source that would have remained
   indefinitely. The client now prunes everything under the runtime prefix that
   the current build does not push. Verified: after a build, the repository
   contains only the workflow, the encrypted blob and the unlocker.

## Verified live

Built on real runners from the encrypted source for all three targets:

| Target | Artifact |
|---|---|
| `x86_64-unknown-linux-gnu` | 339,778 bytes |
| `x86_64-pc-windows-msvc` | 251,294 bytes |
| `aarch64-apple-darwin` | 287,454 bytes |


## Addendum — visibility is a user choice, encryption is not

Encryption does not branch on repository visibility. There is a single push
path, so a private repository gets the same treatment as a public one - which is
correct, because a private repository still exposes its contents to every
collaborator, to anyone later granted read access, and to every clone. A test
asserts that the only files pushed are the ciphertext and the unlocker, that the
sealed package appears nowhere in them, and that the source key is never among
the pushed bytes.

**Verified on a genuinely public repository.** The repository was switched to
public and a build run. What a reader sees:

```
.github/workflows/nyeda.yml
README.md
runtime/capsule.sealed          179,404 bytes of ciphertext
runtime/unlock/Cargo.toml
runtime/unlock/src/main.rs
```

Fetching the blob and inspecting it: no `NYARCH` package magic, no readable Rust
source, nothing but the XChaCha nonce followed by ciphertext. All three targets
still built.

**Visibility control.** Both clients can switch a build repository between
public and private at any time - `nyedarch visibility <owner> <repo>
public|private` on the command line, and a button beside the toggle in the
desktop client. The change is refused while a build is running (spec §38), and
the check asks GitHub whether a run is queued or in progress rather than
trusting local state, because the build may have been started from the other
client or another machine.

Switching to public prints what that exposes: build logs and the workflow become
readable by anyone. The capsule source does not, because it was never pushed in
the clear.
