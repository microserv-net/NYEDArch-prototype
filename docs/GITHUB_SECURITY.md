# NYEDArch — GitHub Build Security

## 0. Remote building is mandatory

Compilation of capsules happens on GitHub Actions. This is the intended flow,
not an optional add-on: the client orchestrates repository setup, secrets,
workflow, dispatch, and artifact retrieval as required stages.

**Transport.** The client speaks to the GitHub REST and Git Data APIs over the
system `curl` binary (`CurlTransport`), which is present on Windows 10+, macOS,
and mainstream Linux. This avoids embedding a large async TLS stack in a
security-sensitive client. The credential is written to curl's **stdin** as a
config stream, never passed in `argv` where the local process list would expose
it, and never placed in the environment of the child process. Verified against
the live GitHub API.

A `reqwest`-based transport remains available behind the `net` feature for
environments that prefer it; `net` also supplies `SodiumSealer`, the libsodium
sealed-box implementation GitHub requires for repository secrets. **Without a
real sealer the client refuses to transmit a secret at all** rather than sending
it unprotected.

## 1. Trust posture

GitHub is an **untrusted external build environment**. It compiles the generated
runtime and returns an artifact. It is never trusted to hold plaintext payload
data, and its output is never accepted on the basis that GitHub returned it.

## 1a. The capsule source is pushed encrypted

The build environment never receives the generated capsule source in the clear.
It is packed into one archive, encrypted with XChaCha20-Poly1305 under a fresh
random key, and pushed as ciphertext next to a small unlocker that holds no
secret. The key is a repository secret, sealed to the repository's public key,
so only a runner can read it. The workflow decrypts, builds, and deletes the
plaintext in an `if: always()` step.

This matters most for a public repository, which previously exposed the sealed
package, the bootstrap key and the runtime commitment to anyone. It is not
protection against the repository owner: whoever controls the repository
controls its workflows, and anything a workflow can decrypt a modified workflow
can print. See ADR-0010.

**Stale files are pruned.** A repository is reused across builds, so anything
under the runtime prefix that the current build does not push is deleted first.
Without that, a repository used before this change would have kept a full
plaintext capsule source forever.

## 2. Data minimization (spec §72)

The build environment receives the generated runtime source and the **already
sealed** package. It never receives: plaintext files, the plaintext archive, the
passphrase, raw fingerprint data, raw coordinates, or any payload key.

The payload is encrypted before it ever leaves the client, so a full compromise
of the repository or the runner does not yield plaintext.

## 3. Two independent directional keypairs (spec §42)

| Channel | Direction | Purpose |
|---|---|---|
| A | Client → GitHub | Authenticity of build inputs |
| B | GitHub → Client | Artifact provenance signatures |

**Reuse across directions is prohibited.** A single keypair would let a
compromised runner mint inputs the client accepts, or forge artifacts.

## 4. Secrets

Repository secrets are encrypted client-side with the repository's Curve25519
public key (libsodium sealed box) before transmission — implemented in
`net::SodiumSealer`. They are never committed, never written to repository
files, and never echoed by the workflow (asserted by test).

Ordering is enforced and tested: the repository public key is fetched **before**
any secret is sealed or written.

## 5. Visibility

Both clients can change a build repository between public and private at any
time: `nyedarch visibility <owner> <repo> public|private`, or the button beside
the toggle in the desktop client. The change is refused while a build is running,
and that check queries GitHub rather than trusting local state.

**Encryption does not depend on visibility.** The capsule source is pushed as
ciphertext either way. A private repository is not a substitute: it still exposes
its contents to collaborators, to anyone later granted read access, and to every
clone. Verified on a genuinely public repository - see ADR-0010.

## 5b. Historical note

Private is the default and the recommendation. Public repositories expose
generated runtime source and build logs. Visibility changes are **refused while
a build is running** (spec §43) — enforced in the orchestrator and tested.

## 5a. Artifact naming and permissions

The workflow stages the compiled binary as
`nyedarch-<run_id>-<target>.nyarch` — one extension on every OS — and sets the
executable bit (`chmod +x`). Windows runners have no execute bit, so that step
is a no-op there by design. Only `*.nyarch` files are uploaded.

Artifact archives do not always preserve permissions, so the client restores the
executable bit when launching a capsule.

## 6. Artifact provenance (correction pass §6)

A matching hash proves only that bytes match bytes. The client commits to a
`BuildManifest` **before dispatch**:

- build id (ULID), target
- package commitment, runtime commitment, source commitment
- crypto version

After the build it verifies the returned artifact against that commitment and a
Channel-B signature over `signed_input()`, which covers every field above plus
the artifact digest. Because the signed input is build-specific, a signature
captured from one build cannot be replayed onto another. Any mismatch fails
closed. Three tests cover accept, mismatch, and replay.

## 7. Toolchain

The workflow requests the latest stable Rust (spec §40); the user never selects
a version. **Reproducibility implication:** builds are not bit-reproducible
across toolchain releases. Provenance therefore binds *build identity*, not a
deterministic hash. Pinning can be introduced later without redesign.

## 8. Residual risks

- A compromised GitHub account could substitute source and produce a signed
  artifact. Mitigation is provenance verification plus private repositories.
- Build logs may reveal file *names* from compilation output; they never reveal
  payload contents.


---

## 9. Verified against live GitHub

The full pipeline has been executed end to end against a real account: private
repository created, secrets sealed with a libsodium sealed box and stored,
workflow and generated capsule source pushed, build dispatched, and capsules
produced for all three targets.

| Target | Artifact |
|---|---|
| `x86_64-unknown-linux-gnu` | 339,771 bytes |
| `x86_64-pc-windows-msvc` | 251,280 bytes |
| `aarch64-apple-darwin` | 287,454 bytes |

Two defects only a live run could expose were fixed in the process: the macOS
target did not compile, and every build after the first into the same repository
failed because the contents API requires the current blob SHA on update. See
ADR-0009.

**Note on repository reuse.** The intended model is one persistent repository
holding many builds, so update-in-place is the normal path, not the exception.


---

## 10. Artifact retrieval and provenance (spec §45)

The build environment is untrusted, so "the workflow reported success" is not
evidence that it produced the right binary. After a run completes the client:

1. lists the run's artifacts,
2. **downloads** the artifact,
3. checks that it carries the package commitment made *before* the build began,
4. saves it beside the capsule project only if that check passes.

A mismatch is a refusal (`GhError::Provenance`), not a warning.

**This was a real gap, found by audit.** The orchestrator previously listed
artifacts and stopped, with a comment claiming verification "happens on
download" - there was no download. A comment describing behaviour that does not
exist is worse than a missing feature, because it stops anyone looking.

**What the check proves:** the artifact contains the package this client sealed.

**What it does not prove:** that the rest of the binary was built from our
source. That needs reproducible builds, which are not implemented. The
protection against a malicious build environment therefore remains the
cryptography - a capsule carrying our package cannot open it without the
authorization factors, whoever compiled it.

**Not verified live:** this network cannot reach GitHub's artifact host
(`objects.githubusercontent.com` is outside the allowlist), so the download path
is exercised by tests rather than against the live service.
