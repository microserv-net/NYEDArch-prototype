# Verifying platform code on hosted runners

How Prototype-I was tested on macOS and Windows without owning either machine,
and without publishing source that is intended for patent filing.

Recorded because it will be needed again — for Prototype-II, for the TPM and
Secure Enclave work, and any time a claim depends on behaviour that cannot be
reproduced on the development machine.

**No credential appears in this document, in the repository, or in any
deliverable.** The token used lived in the environment for the length of a run.

---

## The problem

Several things could be written and reviewed but not *executed*:

- the macOS and Windows fingerprint providers,
- the native location providers,
- the macOS `.app` bundle and CoreLocation,
- `build.bat`,
- platform key storage (Keychain, DPAPI, Secret Service),
- whether a capsule actually builds and runs on each OS.

Shipping unexecuted code and calling it verified would have been dishonest, and
in practice it was also wrong: nearly every one of those areas contained a real
defect that only execution revealed.

## The constraints

1. **Hosted runners are needed** — Linux, macOS and Windows, on demand.
2. **Public repositories get free minutes**; private ones consume a quota.
3. **The source must not be published.** It is intended for patent filing, so a
   public repository full of source was not acceptable.
4. **Job logs could not be downloaded** from the development environment: the
   log endpoint redirects to a host outside the network's egress allowlist.

Constraints 2 and 3 look contradictory. They are not.

## The approach

### Encrypted source in a public repository

NYEDArch's own source-transport mechanism solved its own testing problem. The
workspace is packed into a single archive, encrypted with XChaCha20-Poly1305
under a fresh random key, and pushed as ciphertext. The key is stored as a
**GitHub Actions repository secret**, sealed to the repository's public key, so
only a runner can read it.

```
public repository                  runner
  workspace.sealed      ─────►       build the unlocker
  unlock/**             ─────►       decrypt with $NYEDARCH_SOURCE_KEY
  .github/workflows/**               run the checks
                                     publish results
```

The repository is public — free minutes — and a reader sees an opaque blob and a
small bootstrap crate that holds no secret. Roughly 1 MB of ciphertext.

**This is the same mechanism capsules use for their own build**, which is
convenient: exercising it here also exercises it in production.

### Getting results back when logs are unreachable

Job logs redirect to a blocked host, so the workflow **commits a results file
back into the repository**. The contents API is on an allowed host, so the file
can be read immediately after the run.

```yaml
- name: Publish results
  if: always()
  run: |
    mkdir -p results && { ... } > results/${{ matrix.name }}.txt
    git config user.name nyedarch-ci
    git add results/... && git commit -m "results: ${{ matrix.name }}" || true
    for i in 1 2 3 4 5; do
      git pull --rebase --autostash && git push && break
      sleep $((RANDOM % 8 + 3))
    done
```

Two details that matter: `if: always()` so a failing job still reports, and the
pull-rebase retry loop because three matrix jobs push to the same branch and
will collide.

### Diagnosing a failure you cannot see

When the macOS build failed with no readable log, a temporary workflow captured
the compiler output and committed it back the same way. That produced the exact
errors — a crate-level `forbid(unsafe_code)` that cannot be lifted locally, and
a deprecated CoreLocation call missing its receiver — which were then fixable in
one pass instead of guessed at.

The diagnostic workflow and its output were deleted afterwards.

## Practical notes

**Rotate the key with the workspace.** Every push of a new `workspace.sealed`
needs a new key and a secret update, or the runner decrypts an old archive and
you debug a fixed bug.

**Artifacts are the capsule binary.** They consume Actions storage and, on a
public repository, are downloadable by any authenticated user. Delete them after
retrieval; the client now does this automatically.

**Watch which mechanisms actually engaged**, not just whether the job passed.
The Windows scheduler layer reported nothing for three runs while the job stayed
green, because `schtasks` was failing silently.

**A green matrix is not a green suite.** Steps that continue on error make jobs
succeed while tests inside them fail. Read the published results, not the job
status.

**Beware harness artefacts.** Probing `schtasks /create` from Git Bash fails with
`Invalid argument/option - 'C:/Program Files/Git/create'`, because MSYS
path-translates any argument starting with `/`. That proved nothing about the
product — the Rust code calls `schtasks` directly.

## What this found

Defects that review had passed and that no local test could have caught:

| Platform | Defect |
|---|---|
| macOS | The CoreLocation binding did not compile at all |
| macOS | BSD `wc -c` pads its output, so the deletion fallback unlinked without ever overwriting |
| Windows | The client key changed on every run: DPAPI reported a successful store and returned nothing |
| Windows | `schtasks /tr` truncates past 261 characters and `/z` needs an end boundary, so the destruction scheduler silently never engaged |
| All | Concurrent first-run key initialisation raced; the last writer won and earlier records became unverifiable |
| All | The source archive dropped the executable bit, so an unpacked tree could not run its own scripts |
| Windows | A test false positive: bytes set to `0xFF` without checking the value changed, on a byte that already held `0xFF` |

That last one is worth keeping in mind. Cross-platform testing produces false
positives too, and a finding must be diagnosed before it is believed — it was
recorded as an open defect for one round before an exhaustive sweep proved the
artifact was sound and the test was not.

## Reusing this

1. Create a public repository for verification only.
2. Pack and encrypt the workspace; push the ciphertext and the unlocker.
3. Store the key as a repository secret sealed to the repository's public key.
4. Add a workflow that decrypts, runs checks, and commits results back.
5. Dispatch, poll for completion, read the results file.
6. Delete artifacts afterwards.
7. Rotate the key whenever the workspace changes.

**Credential handling:** keep the token in the environment for the run only.
Never write it to a file in the repository, never pass it in a command line
(NYEDArch's own transport sends it on stdin for this reason), and treat any
token that has been pasted into shared context as exposed and rotate it.
