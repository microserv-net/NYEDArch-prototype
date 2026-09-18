# Component reference

Every component, every module, and what each one is responsible for. Written so
a reviewer can find any behaviour without reading the source first, and so a
claim made elsewhere in the documentation can be traced to the thing that
implements it.

**Ordering:** innermost first. `nyedarch-crypto` depends on nothing NYEDArch
owns; `nyedarch-buildtool` depends on almost everything.

**Convention:** where a design decision is not obvious, the *why* is given, not
only the *what*. Where something is not implemented it says so.

---

## Map

| Crate | Role | Ships in a capsule? | Tests |
|---|---|---|---|
| `nyedarch-crypto` | The confidentiality boundary: AEAD, KDF, key composition, geo and time quantization | **yes** | 10 |
| `nyedarch-core` | Identifiers, policy records, capsule identity, launching | **yes** | 6 |
| `nyedarch-package` | Package format, sealing, restoring, manifests | **yes** (restore half) | 18 |
| `nyedarch-fingerprint` | Adaptive machine identity, `.nyfp` records | **yes** | 2 |
| `nyedarch-platform` | Location providers, hardware capability | **yes** (location) | 17 |
| `nyedarch-runtime` | Authorization state machine, hardening, destruction | **yes** | 43 |
| `nyedarch-github` | Build orchestration against an untrusted CI | no | 20 |
| `nyedarch-buildtool` | The client: pipeline, CLI, registry, keystore | no | 39 |
| `nyedarch-installer` | Per-user installer, macOS bundle | no | 5 |

The "ships in a capsule" column is the one that matters for attack surface: a
capsule carries only what it needs to authorize and extract, so builder-only
capability cannot be reached by someone holding an artifact.

---

## `nyedarch-crypto`

`#![forbid(unsafe_code)]`, no I/O, no platform dependency, randomness injected.
It is deliberately the smallest auditable thing in the project, because
everything else defers its security claims to it.

### `aead` — authenticated encryption

- `seal(key, aad, plaintext) -> Sealed`, `open(key, aad, sealed)`,
  `seal_with(rng, ...)` for deterministic test vectors.
- `Sealed::to_bytes()` / `from_bytes()`: a 24-byte XChaCha20 nonce followed by
  ciphertext and tag.
- XChaCha20-Poly1305 (`AEAD_ID_XCHACHA20POLY1305`). The extended nonce means a
  random nonce per message is safe without a counter, which matters because a
  capsule has no persistent state to count with.

### `kdf` — key derivation

- `hkdf_key(salt, ikm, info) -> Key32`, HKDF-SHA-512.
- `passphrase_key(passphrase, salt, params)`: Argon2id.
- `passphrase_contribution(...)`: the passphrase's contribution to composition,
  domain-separated from the key itself.
- `Argon2Params { m_cost, t_cost, p_cost }`, recorded per capsule in the header
  so a capsule built with different parameters still opens.

### `compose` — key composition

The heart of the central invariant.

- `derive_payload_key(salt, binding, flags, contributions) -> Key32`.
- `payload_aad(binding, flags)`: the authenticated context.
- `Contributions { machine_secret, passphrase_key, location_cell, time_window }`.
- `Binding { package_id, runtime_binding, crypto_version }`.

Each enabled protection contributes 32 bytes through its own domain-separated
label (`LABEL_MACHINE`, `LABEL_PASSPHRASE`, `LABEL_LOCATION`, `LABEL_TIME`).
The payload key is derived from the concatenation. **A missing contribution
cannot be synthesised by patching a branch** — there is no branch; there is
missing key material.

### `geo` — location quantization

- `Reading { lat_deg, lon_deg, accuracy_m }`, `ToleranceMeters(u32)`.
- `quantize(reading, tolerance) -> cell`.

Coordinates are unstable, so a region identifier is derived rather than a
position. **Accuracy worse than the tolerance is refused**, not rounded: a
5 km reading cannot prove presence in a 150 m region.

Location is a **policy factor, not an entropy source** — a region has little
entropy and is never treated as if it had more.

### `timewin` — time windows

- `DailySchedule { slots_minutes, tolerance_minutes, tz_offset_minutes }`.
- `window_id(now)`, `window_id_for_slot(slot)`.

**Day-invariant by design:** the window identifier does not include the date, so
"every day at 14:00" keeps working next year. A regression test opens a capsule
a year after sealing it.

### `rng` — injected randomness

- `RandomSource` trait, `OsRandom`, `FixedSource` for tests.

Injected rather than called directly so the crate has no OS dependency and
deterministic test vectors remain possible.

### `version`, `error`, `bitflags_lite`

`CRYPTO_VERSION`, `KEY_LEN`, `SALT_LEN`, `XNONCE_LEN`; a typed `CryptoError`;
and a minimal flags type so policy flags participate in derivation without a
dependency.

---

## `nyedarch-core`

- `ids`: ULID generation for build identity.
- Policy types: `Policy { location, time, one_shot }`, `PolicyEntry`,
  `PolicyRecord` — the trusted machine set with a per-machine factor secret.
  `PolicyRecord::select(fingerprint_id)` implements the **OR** semantics: any
  trusted machine opens it, never all of them.
- `launch`: runs a capsule as an independent process, repairs the execute bit,
  and provides `terminal_hint()`. Capsule identity is the `.nyarch` extension on
  every platform.

---

## `nyedarch-package`

### `format`

`Header { crypto_version, package_id, runtime_binding, policy, argon,
package_salt, compression, chunk_size }`, magic `NYARCH`, format version 1.

- `parse(bytes) -> Parsed`: **bounds every attacker-controlled length before
  allocating.** A tampered chunk count once requested 51 GB and aborted the
  process; an abort is not a refusal.
- `header_commitment(header)` and `chunk_context(base, header)`: bind the whole
  header into every chunk's AAD, so no header field is unauthenticated.

### `build` (feature `builder`, never in a capsule)

- `collect(paths) -> (Manifest, Vec<FileSrc>)`: walks the tree preserving
  structure, modes, timestamps and symlinks.
- `seal_package(...)` and `seal_package_cancellable(..., level, cancelled)`.

Streaming and chunked: the payload is never held whole in memory, and
cancellation is checked between chunks.

### `pipeline`

- `CompressionMode { Automatic, Maximum, Balanced, Fast }` with `level(payload)`
  — Automatic resolves against payload size, because heavy compression of a
  small input costs build time for nothing.
- `compress_block_level`, `seal_chunk_level`.
- Codec ids: `COMPRESSION_ZSTD`, `COMPRESSION_DEFLATE`, recorded per package so
  older capsules keep opening.

zstd blocks carry an exact length prefix **inside** the AEAD, so the decoder
allocates once instead of guessing an upper bound from attacker-influenced data.

### `restore`

`restore(parsed, key, out) -> RestoreReport`. Verifies, decompresses and writes.
Chunk AAD binds index and terminal flag, so reordering or truncation fails
authentication rather than producing partial output.

### `manifest`

`Manifest`, `Entry`, `Kind` — paths, modes, timestamps, symlinks. Sealed
separately from the payload under its own tag.

---

## `nyedarch-fingerprint`

- `capture() -> Fingerprint`: adaptive, using whatever trustworthy signals the
  platform offers. Each signal has a name and a `Strength`.
- `Fingerprint::id`, `id_hex()`, `best_strength`.
- `nyfp`: `seal`, `open`, `seal_bytes`, `open_bytes`, `Record`.

**Only salted digests are stored.** Raw hardware identifiers are never retained
or transmitted, and the interface never shows them.

Observed strengths: Linux `dmi.product_uuid` (Strong), macOS `io.platform_uuid`
(Strong), Windows `crypto.machine_guid` (Medium).

`.nyfp` records are MAC-authenticated with a key from the client keystore, so a
record cannot be relabelled by anyone without that key. Labels are metadata but
are covered by the tag.

---

## `nyedarch-platform`

### `lib` — native location

GeoClue2 over D-Bus on Linux; the system location service via PowerShell on
Windows; CoreLocation on macOS. `acquire_location()` returns nothing rather than
a fabricated reading. `acquire_location_with_fallback(tolerance)` tries native
first, then the browser, and reports which was used.

**macOS needs an application bundle** carrying
`NSLocationWhenInUseUsageDescription`; a bare CLI is refused by the platform
regardless of the code.

### `browser` — consent flow

A one-shot loopback listener on `127.0.0.1`, a 256-bit single-use token required
in both the page URL and the result path, a loopback peer check, a 120-second
deadline, and refusal of any reading without a usable accuracy figure.

**On IP-derived positions:** a browser may answer from a network estimate and the
API does not say which. The accuracy gate is what rejects them — network
estimates report kilometres.

### `hardware` — capability and policy

`HardwareBacking { Available, PresentButUnusable, Absent, Virtualized }` and
`HardwarePolicy { Preferred, Required }`, combined by `decide()`.

`PresentButUnusable` is separate because a half-initialised TPM protects
nothing, and counting it as present would produce exactly the false assurance
the feature exists to prevent. Virtualisation is not a hardware root: a virtual
TPM's state usually travels with the image.

Key operations are **DESIGNED — NOT IMPLEMENTED**; detection and policy are
implemented and tested.

---

## `nyedarch-runtime`

### `lib` — the authorization state machine

`Initializing → IntegrityCheck → BindingValidation → BootstrapPolicy →
FingerprintAcquisition → FingerprintAuthorization → PassphraseAcquisition →
LocationAcquisition → TimeAcquisition → KeyDerivation → PayloadKeyUnwrap →
PayloadAuthentication → Decryption → Decompression → Extraction →
ExtractionVerification → Cleanup → Success`, with `FailClosed` from anywhere.

**Constant-shape:** every protection is evaluated and key derivation always
runs before any verdict. A failed protection contributes a *random decoy* rather
than returning early, so every denial fails at authenticated decryption. There
is no `if !authorized` branch left to patch. Denial timing went from ~50x
separation to ~1.1x.

Providers are injected: `PassphraseProvider`, `LocationProvider`, `TimeProvider`
— which is what lets the future authenticated time source replace the local
clock without redesigning anything.

### `harden` — anti-tamper and anti-analysis

A tamper accumulator fed by debugger, ptrace, timing, environment-marker,
injected-module and instrumentation-thread probes. Observations are **folded,
not branched on**, so patching one comparison does not neutralise them.

Environment observations are deliberately **not** key material: they are not
reproducible, and a key derived from them would lock out honest users.

### `destruction`

`Destruction { Removed, OverwrittenButPresent, RemovedWithoutOverwrite, Failed,
NothingToDo, Delegated }` — every outcome is a fact, never an assumption.

Three redundant layers: in-process overwrite/rename/unlink/**verify**; the
platform's one-shot scheduler; a detached child. `Delegated` is distinct from
`Removed` because the requesting process exits before the work completes.

### `location`

The capsule-side location adapter, so the runtime never depends on builder code.

---

## `nyedarch-github`

Treats CI as an **untrusted build environment** throughout.

- `endpoints`: every REST call as a pure value — no I/O in the description.
- `http`: `Transport` and `SecretSealer` traits, `GhError` including
  `Provenance` and `BuildFailed`.
- `curl_transport`: credentials on **stdin**, never argv. Follows redirects but
  **not** `location-trusted`, so a cross-host redirect drops the token;
  redirects pinned to https and bounded.
- `sealer`: libsodium sealed box for repository secrets. Compiled by default —
  GitHub rejects an unsealed secret, so a build could never have worked without
  it.
- `orchestrator`: authenticate → ensure repo → set visibility → push secrets →
  prune stale files → push source → dispatch → **wait for the new run** → fetch
  → verify → delete artifact.
- `provenance`: `BuildManifest`, `signed_input()`, replay resistance.
- `workflow`: generates the Actions workflow — decrypt, build, stage as
  `.nyarch`, remove the decrypted source in an `if: always()` step.

Two subtleties worth knowing: the newest run is recorded **before** dispatch, so
the client cannot attach to a previous run; and stale files are pruned, because
a repository reused across builds would otherwise keep an old plaintext source
forever.

---

## `nyedarch-buildtool`

The client. Never reaches a capsule.

- `cli`: declarative subcommands, generated help, values validated against
  allowed sets so a typo cannot select a looser policy.
- `pipeline`: the single `seal_and_generate` both clients call. Reports real
  stages, honours cancellation, stages under `.partial` and renames on success
  so a cancelled build leaves nothing usable.
- `config`: protection flags, rejecting unknown values rather than defaulting.
- `machines`: the trusted registry — search, labels, ANY/ALL tag selection,
  import with verification, export, relabel, remove. This machine always appears.
- `keystore`: a random master key in the platform keystore, everything else
  derived with domain separation. Minting is serialised on a lock file; a store
  that cannot return what it was given is not trusted. Windows DPAPI is disabled
  in favour of a stable disclosed file.
- `srcpack`: the encrypted source archive and the secret-free unlocker.
- `generator`: per-build capsule source, vendored crates, hardened profile,
  imports matching enabled protections.
- `remote`: the remote build, artifact retrieval and storage.
- `harden`: the client's own anti-analysis check — advisory, never denying.
- `eula`: shared licence gating for both clients.
- `bench`: fingerprinting, Argon2id and codec measurements.
- `logging`: verbose diagnostics, timestamped in IST. Off unless `--with-logs`
  or the desktop switch asks for them. Reports **which provider answered a
  location request and how accurate the fix was** - the client previously knew
  and discarded it, so nobody could tell whether a location had really been
  captured. Accuracy in metres, never coordinates; signal names, never raw
  identifiers. Lines are buffered for the desktop client, where stderr goes
  nowhere a user will look, and the buffer is bounded so it cannot grow without
  limit.

---

## `nyedarch-installer`

Per-user, no elevation. Verifies `SHA256SUMS` before writing anything, copies
itself so `--uninstall` survives the media, and prunes empty directories.

On macOS it produces a real `.app` bundle at `~/Applications/NYEDArch.app` with
`Info.plist`, `PkgInfo`, executables in `Contents/MacOS`, resources in
`Contents/Resources`, and an ad-hoc signature. **That bundle is what makes
CoreLocation reachable at all.**

---

## Where to look next

| Question | Document |
|---|---|
| Why does this exist? | `PHILOSOPHY.md` |
| What does an attacker get? | `ANTI_RE_ANALYSIS.md`, `ATTACK_LABORATORY.md` |
| What is the crypto exactly? | `CRYPTO_FORMAT.md`, `KEY_HIERARCHY.md` |
| What is not finished? | `PENDING_DOCUMENTATION_WORK.md` |
| Why was it built this way? | `decisions/` |
