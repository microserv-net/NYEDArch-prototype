# NYEDA — NOT YOUR EVERYDAY ARCHIVE

## Master Engineering, Architecture, Security & Implementation Prompt

You are going to design and implement a production-quality prototype of **NYEDA — Not Your Everyday Archive**.

This is not a conventional archive utility, password-protected ZIP application, or ordinary file encryption tool.

NYEDA's core concept is:

> **Transform passive data into an active, self-protecting executable capsule that carries its own protected data, authorization policy, decryption logic, integrity mechanisms, and hostile-environment defenses.**

The resulting executable is intended to protect data when it leaves a trusted environment and enters an environment that must be treated as hostile.

---

# 0. CRITICAL ORIGINALITY REQUIREMENT

NYEDA is an original project and is intended to support intellectual-property/patent work.

**Do NOT copy, imitate, reproduce, or aesthetically derive NYEDA's UI from any existing project.**

In particular:

* Do NOT copy Thermite's UI.
* Do NOT copy the UI of any other project created by the user.
* Do NOT copy layouts, colors, typography, navigation patterns, terminology, animations, component arrangements, visual hierarchy, or interaction flows from Thermite.
* Do NOT simply reskin an existing application.
* Do NOT reuse Thermite's visual identity.
* Do NOT make NYEDA look like a derivative of Thermite.

The UI/UX must be **independently invented for NYEDA**.

You are encouraged to create a distinctive visual language appropriate to a security-oriented data capsule application.

The user explicitly wants you to **invent the UI yourself**.

You may study general usability principles and platform conventions, but NYEDA's resulting visual design and interaction model must be independently conceived.

---

# 1. THERMITE REFERENCE — TECHNICAL ONLY

The following project may be inspected strictly as a **technical reference for its GitHub-based Rust compilation/orchestration architecture**:

Repository:

https://github.com/microserv-net/thermite-web

Website:

https://microservices.net.in/thermite-web/

Use it to understand concepts such as:

* GitHub authentication
* repository initialization
* GitHub API orchestration
* Git Data API workflows
* GitHub Actions triggering
* build records
* live build logs
* target selection
* artifact retrieval
* one-build-per-unit-of-work architecture
* public/private repository handling

Do NOT copy its UI or visual design.

Do NOT assume its implementation is automatically suitable for NYEDA.

Do NOT blindly copy its architecture.

Instead:

1. understand the underlying concept,
2. identify what NYEDA actually needs,
3. redesign the implementation for NYEDA's security model,
4. improve it wherever necessary.

Thermite is a **reference**, not a dependency.

---

# 2. CORE PRODUCT MODEL

NYEDA consists of two fundamentally different systems.

## A. NYEDA Builder

A desktop application permanently installed on the trusted user's machine.

Written in Rust.

Responsibilities:

* file/folder selection
* compression
* encryption
* fingerprint generation
* fingerprint management
* EULA management
* package construction
* cryptographic metadata generation
* runtime generation
* GitHub authentication
* repository management
* build orchestration
* target selection
* live logs
* artifact retrieval
* artifact verification
* local key management
* security configuration
* creator/diagnostic functionality

---

## B. NYEDA Runtime

A separately compiled executable generated for a particular package.

This is NOT the desktop client.

It is the final artifact delivered to the recipient.

Responsibilities:

* establish runtime integrity
* protect its execution state
* obtain current machine identity
* decrypt protected authorization information
* validate the machine fingerprint
* obtain location automatically if enabled
* obtain current time automatically
* obtain passphrase from the user
* derive required cryptographic factors
* decrypt the payload only after authorization succeeds
* decompress the payload
* restore files/folders
* clean sensitive intermediate state
* optionally destroy itself according to package policy

The runtime must NOT require the NYEDA desktop application.

---

# 3. ZERO-TRUST ASSUMPTION

Treat the generated runtime as a zero-trust security boundary.

Assume an attacker may have:

* unrestricted access to the executable file
* the ability to copy the executable
* the ability to disassemble it
* the ability to debug it
* the ability to patch it
* the ability to instrument it
* the ability to hook APIs
* the ability to snapshot memory
* the ability to run it repeatedly
* the ability to modify its environment
* the ability to manipulate system time
* the ability to run it inside a VM
* the ability to inspect system calls
* the ability to inspect generated files
* the ability to modify branches
* the ability to attempt to bypass authorization
* the ability to replace or intercept individual functions
* the ability to analyze the generated binary statically and dynamically

Do NOT design security under the assumption that the executable is trusted merely because it was generated by NYEDA.

The runtime must assume that the execution environment is hostile.

---

# 4. FUNDAMENTAL CONFIDENTIALITY RULE

The payload itself must remain cryptographically inaccessible until authorization succeeds.

Do not rely on:

* hidden variables
* obscure filenames
* plaintext conditions
* string obfuscation
* a single authorization `if`
* self-deletion
* anti-debugging alone

as the actual confidentiality boundary.

Cryptography must be the confidentiality boundary.

Anti-tamper and anti-reverse-engineering mechanisms exist to make reaching, modifying, observing, or bypassing that boundary substantially more difficult.

---

# 5. MANDATORY AUTHORIZATION MODEL

Every NYEDA package has mandatory machine authorization.

The user MUST NOT be able to disable this.

Every NYEDA package also has mandatory passphrase protection.

The user MUST NOT be able to disable this.

Optional additional protection factors:

* geolocation
* time

Therefore the minimum package is:

```
Machine Authorization
      AND
Passphrase
```

If geolocation is enabled:

```
Machine Authorization
      AND
Passphrase
      AND
Location
```

If time protection is enabled:

```
Machine Authorization
      AND
Passphrase
      AND
Time
```

If all three encryption mechanisms are enabled:

```
Machine Authorization
      AND
Passphrase
      AND
Location
      AND
Time
```

Any failure MUST result in failure.

There is no prototype fallback mechanism.

---

# 6. FINGERPRINT AUTHORIZATION

The fingerprint represents a concrete machine state.

A package may contain multiple trusted fingerprints.

The relationship between fingerprints is OR.

Example:

```
fingerprint A
fingerprint B
fingerprint C
```

The current machine is authorized if:

```
current fingerprint == A
   OR
current fingerprint == B
   OR
current fingerprint == C
```

Only one fingerprint needs to match.

Do NOT interpret this as requiring all fingerprints simultaneously.

---

# 7. MACHINE FINGERPRINT DESIGN

Do NOT create a simplistic hardcoded fingerprint schema such as:

```
CPU serial + motherboard serial + MAC address
```

because different platforms expose different information.

Instead create an adaptive fingerprinting framework.

The fingerprint engine should discover and use as many trustworthy, stable, platform-appropriate signals as legitimately available.

Investigate, where available:

* OS identity
* machine identifiers
* firmware identity
* motherboard/platform identifiers
* CPU/platform characteristics
* storage identity
* TPM-backed information
* Secure Enclave/platform security capabilities
* hardware-backed identifiers
* boot/platform information
* stable operating-system identity
* other appropriate machine-state signals

The exact available signals may differ between:

* Windows
* macOS
* Linux

This is intentional.

The fingerprint algorithm must be capable of producing a deterministic fingerprint from the information actually available on that machine.

The important abstraction is:

> The same machine, under the same relevant machine state, should reproduce the same fingerprint.

Do not require identical raw fields across every operating system.

---

# 8. MACHINE STATE SEMANTICS

The fingerprint represents the machine state at capture time.

If the machine later undergoes a meaningful change, such as:

* motherboard replacement
* storage replacement
* hardware upgrade
* OS-level identity change
* firmware change
* other identity-affecting changes

the resulting fingerprint may legitimately change.

This is acceptable.

The desktop application must allow the trusted user to:

* recapture the current fingerprint
* replace an existing fingerprint
* add a new fingerprint
* export it
* import it
* label it
* assign multiple tags

Future license-server functionality may eventually provide controlled recovery/re-authorization for changed machines.

Do NOT implement that future functionality now.

---

# 9. HARDWARE-BACKED IDENTITY

Where practical and legitimately supported:

* investigate TPM-backed identity on Windows/Linux
* investigate Secure Enclave/platform capabilities on macOS
* investigate platform key stores
* use OS-native secure facilities where appropriate

However:

Do not make NYEDA unusable solely because a particular hardware-backed facility does not exist.

The fingerprint system must degrade based on available trustworthy signals while clearly recording what was available at capture.

Do not silently substitute weak information for strong information without documenting the difference.

---

# 10. FINGERPRINT PACKAGES

Fingerprint export/import must use a dedicated secure non-human-readable format.

Example conceptual extension:

```
.nyfp
```

The format should be versioned.

It should contain whatever is necessary to reproduce/verify the fingerprint authorization record without exposing unnecessary raw machine information.

Imported fingerprint records must be authenticated.

Design the fingerprint package so that a user cannot simply edit:

```
employee = "Alice"
```

into:

```
employee = "Administrator"
```

and thereby modify the authorization material.

Use appropriate cryptographic authentication/signatures.

Labels and tags are metadata, not authorization secrets.

Example:

```
Fingerprint:
    <opaque fingerprint ID>

Labels:
    HR
    Bangalore
    Employee-A
```

The UI should not expose unnecessary raw hardware identifiers.

---

# 11. FINGERPRINT ENCRYPTION

The fingerprint authorization material inside the generated runtime MUST NOT be plaintext.

The runtime must first unlock/decrypt the protected fingerprint authorization material and then perform the fingerprint comparison.

However, solve this correctly.

Do NOT create the circular dependency:

```
fingerprint needed to decrypt fingerprint
```

The architecture must contain a cryptographically sound bootstrap mechanism that allows the runtime to access the protected authorization policy without exposing the policy in plaintext inside the binary.

Analyze:

* where the bootstrap key originates
* how it is protected
* what an attacker can extract
* whether it becomes a bypass primitive
* how runtime identity binding affects it
* how payload encryption remains independently protected

Do not simply hide a key inside an obfuscated string.

---

# 12. FINGERPRINT UI

Create an original NYEDA fingerprint management experience.

The user must be able to:

* see available trusted fingerprints
* search fingerprints
* filter by labels/tags
* select multiple tags
* select fingerprints
* create labels
* assign multiple labels
* import fingerprint packages
* export the current fingerprint
* replace fingerprints
* recapture the current machine
* see whether a fingerprint is currently active/selected

During package creation:

The current machine fingerprint MUST always be included because the creator is always trusted.

The UI should make this obvious.

---

# 13. EULA

The EULA must be shown before NYEDA can be used.

The user must explicitly accept it.

Record locally:

* EULA version
* acceptance timestamp
* application version
* relevant local acceptance metadata
* cryptographic integrity of the acceptance record

The EULA must explain that NYEDA may collect/use system information for:

* machine fingerprint generation
* authorization
* security enforcement
* package creation

It must explain that some fingerprint information may be sensitive.

It must explain:

* GitHub interaction
* repository storage implications
* public repository risks
* private repository recommendation
* local key storage
* private-key loss consequences
* location permission
* geolocation processing
* time-based protection
* self-destruction behavior
* secure deletion limitations
* inability to guarantee absolute security
* user responsibility
* lawful-use requirements
* data-loss risks

Do not claim mathematically impossible security.

---

# 14. BUILDER INPUT

Normal package creation:

```
Select files/folders
    ↓
inspect/package
    ↓
compression
    ↓
encryption
    ↓
fingerprint authorization
    ↓
runtime generation
    ↓
GitHub build
    ↓
artifact verification
    ↓
final NYEDA executable
```

Support multiple files and folders.

Preserve directory structure.

Design a versioned manifest capable of preserving appropriate:

* filenames
* directory hierarchy
* permissions
* timestamps
* executable bits
* symlinks where supported
* platform-specific metadata where practical

---

# 15. STREAMING AND PERFORMANCE

NYEDA must be designed for large datasets.

Do NOT load entire archives into memory.

Use:

* streaming
* chunking
* bounded buffers
* parallel processing
* multithreading
* backpressure
* asynchronous I/O where appropriate

Pipeline concept:

```
files
  ↓
streaming reader
  ↓
chunking
  ↓
parallel compression
  ↓
authenticated encryption
  ↓
package writer
```

Design for:

* low memory pressure
* high throughput
* cancellation
* progress reporting
* recovery from individual I/O failures

Benchmark the implementation.

Do not claim "lightning fast" without measuring bottlenecks.

---

# 16. COMPRESSION

Provide an automatic compression strategy.

Potential modes:

```
Automatic
Maximum
Balanced
Fast
```

Evaluate appropriate modern compressors.

Do not assume the highest compression level is always best.

Benchmark representative datasets.

Compression must be streamable/chunkable.

Create explicit symmetric pair functions:

```
compress()
decompress()
```

with independently testable implementations.

---

# 17. ENCRYPTION ARCHITECTURE

Encryption is composed of independent factors.

The UI must NOT represent them as radio buttons.

They are checkboxes.

Mandatory:

```
[LOCKED/ALWAYS ON] Machine Authorization
[LOCKED/ALWAYS ON] Memory-Hard Passphrase
```

Optional:

```
[ ] Geolocation Lock
[ ] Time Lock
```

The user may enable either or both optional mechanisms.

If enabled, they become mandatory decryption factors.

---

# 18. PASSPHRASE PROTECTION

Use a modern memory-hard password KDF.

Do not simply invent a custom password hash.

Evaluate:

* Argon2id or another well-established modern memory-hard primitive
* appropriate memory cost
* time cost
* parallelism
* per-package random salt
* versioned parameters

You may use a chained KDF architecture if there is a defensible security reason.

If chaining multiple KDFs:

* domain-separate each stage
* document the security purpose
* do not stack algorithms merely to create marketing complexity
* benchmark them
* prevent accidental denial-of-service settings

Passphrase-derived material must never be stored in plaintext.

The passphrase must never appear in:

* logs
* config files
* crash reports
* clipboard automatically
* GitHub repository
* GitHub Actions logs
* package metadata

Zeroize sensitive buffers where practical.

---

# 19. GEOLOCATION PROTECTION

Geolocation is optional.

During BUILDING:

The user must provide a real location through an automatic location acquisition flow.

Do NOT provide a normal manual latitude/longitude entry field.

Use an appropriate native OS location API where available.

If unavailable, an explicitly designed browser-based location flow may be used with user consent.

The user must grant permission.

During DECRYPTION:

The runtime automatically requests location permission.

The user is NOT asked to manually type coordinates.

The runtime obtains:

* latitude
* longitude
* reported accuracy

The user cannot simply provide arbitrary coordinates through the normal UI.

---

# 20. GEOLOCATION QUANTIZATION

Raw GPS coordinates are unstable.

Design a deterministic geographic quantization scheme.

Investigate appropriate methods such as:

* geohash
* H3
* S2
* another defensible spatial quantization system

The selected tolerance should define the geographic acceptance region.

Example:

```
tolerance:
    10m
    50m
    100m
    500m
    custom
```

If the reported location accuracy is worse than the configured tolerance:

```
FAIL
```

Example:

```
configured tolerance = 100m
reported accuracy = 2km
```

Result:

```
reject
```

Do not silently accept poor-quality location information.

Document the mathematical/geometric behavior.

---

# 21. GEOLOCATION KEY DERIVATION

The geolocation contribution should be generated through a strong KDF pipeline.

Conceptually:

```
normalized geographic representation
        ↓
domain separation
        ↓
KDF stages
        ↓
location key contribution
```

The exact construction is yours to design.

Do not claim that geographic coordinates themselves have high entropy.

Treat location as a policy factor.

---

# 22. TIME PROTECTION

Time protection is optional.

During package creation, the user configures:

* frequency
* day(s)
* time
* timezone
* tolerance window

Examples:

```
Every day at 14:00 ±15 minutes

Every day at:
    02:00
    14:00

Every Thursday at 06:00 ±30 minutes
```

The runtime automatically obtains the current local system time.

The user does NOT manually type the current time.

---

# 23. LOCAL TIME LIMITATION

For the prototype:

Use local system time.

Clearly document:

```
FUTURE — LICENSE SERVER
```

The future license server will provide authenticated time.

Do NOT implement the license-server time source now.

The time subsystem should be abstracted behind an interface so that a future authenticated time provider can replace the local clock without redesigning the entire runtime.

---

# 24. TIME KEY CONTRIBUTION

The current time policy must be deterministically transformed into a cryptographic key contribution.

The runtime calculates the current time window itself.

Do not embed a plaintext "correct time" comparison that can trivially be patched.

The implementation should make the time policy part of the cryptographic authorization architecture.

User-configured tolerance must be respected.

---

# 25. FINAL KEY COMPOSITION

When factors are selected:

```
K_machine
K_passphrase
K_location
K_time
```

combine them using a well-designed key derivation/composition mechanism.

Do NOT simply concatenate strings and hash them without domain separation.

Use:

* independent salts
* domain separation
* versioning
* authenticated encryption
* explicit context binding
* package identity binding

The final payload key must only become available when every enabled mandatory factor has succeeded.

If any required factor fails:

```
NO PAYLOAD KEY
```

---

# 26. PAYLOAD AUTHORIZATION ORDER

Conceptually:

```
runtime starts
   ↓
runtime integrity
   ↓
protected policy bootstrap
   ↓
decrypt protected fingerprint authorization data
   ↓
collect current fingerprint
   ↓
compare against trusted fingerprint set
   ↓
if no match → fail closed
   ↓
obtain passphrase
   ↓
obtain location if enabled
   ↓
obtain time
   ↓
derive all required factors
   ↓
derive final payload key
   ↓
authenticate/decrypt payload
   ↓
decompress
   ↓
restore
```

Do not release plaintext payload data earlier than necessary.

---

# 27. PAYLOAD MUST BE BOUND TO THE GENERATED RUNTIME

A payload should not be trivially transplantable into another NYEDA runtime.

Bind package cryptography to:

* package identity
* runtime identity
* build identity
* cryptographic version
* relevant policy metadata

The goal is:

```
Runtime A + Payload A → valid
```

while:

```
Runtime B + Payload A → invalid
```

even if someone copies the encrypted payload.

Design this carefully so the runtime binding itself does not become an easily extractable secret.

---

# 28. RUNTIME TEMPLATE

The runtime is generated from a template/project.

However:

**The plaintext runtime template must NOT simply be embedded inside the desktop client.**

Do not ship:

```
template.rs
```

inside the application in obvious plaintext.

Instead investigate a secure/runtime generation architecture such as:

* build-time generation
* modular source generation
* protected template fragments
* generated source structures
* build-specific transformations
* cryptographically authenticated template components
* per-build diversification

Do not claim that encrypting the template makes it impossible to reverse engineer.

The objective is to avoid providing a static, immediately recoverable template to an attacker.

---

# 29. BUILDER/RUNTIME SEPARATION

Keep encryption/build-side and decryption/runtime-side functionality conceptually separated.

Builder:

```
compress
encrypt
package
construct policy
generate runtime
orchestrate build
```

Runtime:

```
verify
authorize
decrypt
decompress
restore
cleanup
```

For paired functions, maintain explicit conceptual symmetry:

```
compress ↔ decompress
encrypt ↔ decrypt
package ↔ unpack
fingerprint capture ↔ fingerprint capture
manifest creation ↔ manifest validation
```

Do not accidentally ship unnecessary builder capabilities into the runtime.

Do not ship unnecessary decryption capabilities into the builder beyond what is required for testing/verification.

---

# 30. SELF-INTEGRITY

The generated executable must detect tampering.

Do NOT rely on one self-hash check.

Investigate layered mechanisms such as:

* executable-section integrity
* signed metadata
* build-specific cryptographic commitments
* redundant integrity verification
* independent verification paths
* platform-native executable integrity facilities
* runtime consistency checks
* package/runtime binding
* control-flow integrity where practical
* anti-hooking techniques
* code/data separation

Assume an attacker can patch a single check.

Design accordingly.

A modified runtime should fail closed.

---

# 31. ANTI-REVERSE-ENGINEERING REQUIREMENT

This is a core requirement.

Do NOT merely implement textbook:

```
if debugger_detected { exit(); }
```

everywhere.

Instead perform an adversarial design process.

For every security mechanism:

1. Design it.
2. Assume an expert attacker knows it exists.
3. Determine how they would bypass it.
4. Identify the bypass.
5. Redesign or layer the mechanism.
6. Implement the improved version.
7. Test the resulting mechanism.
8. Document its limitations.

Investigate and combine appropriate techniques involving:

* control-flow diversification
* opaque predicates where defensible
* code/data separation
* function fragmentation
* sensitive operation isolation
* string/data protection
* runtime-generated values
* per-build diversification
* build-specific constants
* integrity cross-checking
* anti-hooking
* debugger detection
* instrumentation detection
* timing anomaly detection where defensible
* environment consistency checks
* VM/sandbox detection where appropriate
* syscall/API integrity validation
* import minimization
* symbol stripping
* LTO
* aggressive release optimization
* function-level transformations
* sensitive-path compartmentalization
* cryptographic state minimization
* memory zeroization
* execution-path diversification
* redundant independent checks
* delayed verification
* cross-module verification
* self-consistency checks
* runtime integrity checks

Do NOT implement mechanisms merely because they sound impressive.

If a technique is trivially bypassable, document why and seek a better construction.

---

# 32. DO NOT RELY ON CONDITIONAL LOGIC AS THE SECURITY BOUNDARY

This requirement is extremely important.

Do not make the fundamental security architecture:

```
if fingerprint_ok {
    decrypt();
}
```

because an attacker could potentially patch:

```
fingerprint_ok = true
```

Instead design the system so that cryptographic material required for successful decryption is unavailable unless authorization succeeds.

The ideal architecture should make bypassing a branch insufficient.

For example:

```
authorization state
      ↓
cryptographic key material
      ↓
authenticated decryption
```

rather than:

```
authorization state
      ↓
if statement
      ↓
same key regardless
```

Investigate architectures where authorization participates materially in key derivation and access to protected material.

---

# 33. AUTO-ACQUISITION MUST BE TAMPER RESISTANT

The following must NOT have ordinary manual fallbacks:

* fingerprint
* location
* time

The runtime must acquire them itself.

This includes:

```
Location
    ↓
OS/browser provider
    ↓
NYEDA-controlled validation
```

and:

```
Time
    ↓
platform clock
    ↓
NYEDA validation
```

Do not expose:

```
"Enter latitude manually"
```

or:

```
"Enter current time manually"
```

as a normal fallback.

If an environmental input cannot be obtained reliably, fail closed.

---

# 34. INTERRUPTION RESISTANCE

The user explicitly requires that security-sensitive operations be resistant to interruption and analysis.

Pay particular attention to:

* key derivation
* fingerprint validation
* location validation
* time validation
* payload decryption
* temporary plaintext creation
* cleanup
* self-destruction

Do not make every operation artificially slow.

The application should be fast wherever speed is safe.

Only use expensive computation where it serves an actual security purpose.

---

# 35. SELF-DESTRUCTION

The runtime supports:

```
Reusable capsule
```

or:

```
Destroy after successful extraction
```

If authorization fails:

```
fail closed
destroy sensitive state
destroy temporary material
perform best-effort cleanup
optionally self-delete
exit
```

If one-shot mode is enabled and extraction succeeds:

```
verify successful extraction
destroy sensitive state
destroy temporary plaintext/intermediate material
perform best-effort capsule deletion
exit
```

Do NOT falsely claim that software can guarantee secure deletion on SSDs/filesystems.

Explain limitations.

The cryptographic design should minimize reliance on physical deletion.

---

# 36. NO PLAINTEXT PAYLOAD BEFORE AUTHORIZATION

This is non-negotiable.

The runtime must not:

* decompress payload before authorization
* write plaintext payload to disk before authorization
* create plaintext temporary archives before authorization
* expose plaintext through logs
* expose plaintext through crash diagnostics

The payload should remain encrypted until authorization and key derivation have succeeded.

---

# 37. ONE-SHOT MODE

Expose a user setting during package creation:

```
Execution Policy

○ Reusable
○ Destroy after successful extraction
```

If one-shot is selected, implement appropriate post-success destruction.

This is not a substitute for cryptographic security.

---

# 38. GITHUB REPOSITORY INITIALIZATION

During first-run initialization provide:

```
Public Repository
Only for testing

Private Repository
Recommended
```

Default:

```
Private Repository
```

The user can change repository visibility later.

Do not allow repository visibility changes while a build is actively executing.

Handle:

* GitHub authentication
* permissions
* repository creation
* repository configuration
* workflow setup
* secrets
* keys
* build state

gracefully.

---

# 39. GITHUB BUILD MODEL

Use a Thermite-inspired GitHub Actions architecture but redesign it for NYEDA.

One persistent NYEDA repository may contain multiple builds.

Conceptually:

```
NYEDA repository
    │
    ├── build/pour A
    ├── build/pour B
    ├── build/pour C
    └── ...
```

Each build should have a unique identifier.

Use an appropriate ID format such as ULID or equivalent.

The desktop application orchestrates:

* commit creation
* Git Data API operations where useful
* workflow dispatch
* target selection
* build state
* live logs
* artifact discovery
* artifact retrieval
* artifact verification

---

# 40. RUST TOOLCHAIN

Do NOT expose Rust version selection to the user.

Always use the latest appropriate stable Rust version available at build time.

The application should determine and request the latest stable toolchain automatically.

Document reproducibility implications.

If exact reproducibility becomes necessary later, design the architecture so pinning can be introduced without redesigning the entire application.

---

# 41. INITIAL TARGETS

Support initially:

### Windows

```
MSVC
```

### macOS

```
Apple Silicon
```

### Linux

```
general Linux
initially x86_64 where appropriate
```

Design the runtime project for future target expansion.

Do not hardcode platform assumptions throughout the core architecture.

Use platform abstraction layers.

---

# 42. TWO DIRECTIONAL KEY SYSTEM

During GitHub initialization generate two independent cryptographic keypairs.

Conceptually:

### Channel A

```
NYEDA Client
      ↓
GitHub repository/build environment
```

### Channel B

```
GitHub build/release
      ↓
NYEDA Client
```

Do NOT reuse one keypair for both directions.

Use modern cryptographic constructions appropriate for the actual purpose.

Do not use asymmetric encryption directly for arbitrarily large payloads.

Use hybrid encryption:

```
random symmetric key
    ↓
AEAD encrypt data
    ↓
asymmetric KEM/encryption protects symmetric key
```

where appropriate.

---

# 43. GITHUB SECRETS

Repository private secrets must be stored as actual GitHub Actions repository secrets.

They must NOT be:

* committed to Git
* written to normal repository files
* printed in logs
* included in source
* exposed through workflow output

The client should automatically configure required secrets using GitHub APIs.

Treat GitHub Actions as a build environment rather than a trusted location for unnecessary plaintext secrets.

Minimize what the Actions environment can access.

---

# 44. LOCAL KEY STORAGE

The client must retain its cryptographic key material locally.

Do NOT rely on the user remembering to manually save keys.

Store protected copies in the platform-appropriate configuration/security area.

Investigate:

Windows:
DPAPI / Credential Manager / appropriate OS facilities

macOS:
Keychain

Linux:
Secret Service/keyring where available

Additionally maintain appropriate application-level encrypted configuration storage.

The EULA and first-run wizard must warn:

> Loss of required private key material may make previously generated artifacts unrecoverable.

Do not silently discard keys.

---

# 45. ARTIFACT DELIVERY

After GitHub builds the runtime:

```
retrieve artifact
    ↓
verify artifact authenticity/integrity
    ↓
verify expected package/runtime identity
    ↓
store locally
    ↓
present to user
```

Never blindly trust an artifact merely because GitHub returned it.

---

# 46. GUI

Invent a completely original NYEDA UI.

Do not imitate:

* Thermite
* previous user projects
* existing archive applications
* common hacker-themed interfaces
* generic "cyberpunk" interfaces

The UI should feel like a serious security product.

It should be:

* rich
* modern
* extremely responsive
* easy to understand
* information-dense without being confusing
* keyboard friendly
* accessible
* platform appropriate
* visually distinctive

Create an original design system.

The user should never need to understand cryptography to use the basic workflow.

Advanced information may be progressively disclosed.

---

# 47. SUGGESTED USER JOURNEY

Do NOT copy this visually; this is functional guidance only.

First launch:

```
EULA
  ↓
Security initialization
  ↓
GitHub initialization
  ↓
Repository selection
  ↓
Key generation
  ↓
Key confirmation
  ↓
fingerprint capture
  ↓
main application
```

Main workflow:

```
Create NYEDA
  ↓
Select files/folders
  ↓
Configure protection
  ↓
Select trusted fingerprints
  ↓
Configure optional location
  ↓
Configure optional time
  ↓
Configure execution behavior
  ↓
Review security summary
  ↓
Build
  ↓
GitHub live logs
  ↓
Artifact verification
  ↓
Deliver executable
```

---

# 48. TRUSTED FINGERPRINT SELECTOR

The package creation interface should allow:

```
Search

Tags:
    HR
    Employee
    Bangalore
    Finance
    Laptop
```

The user can select multiple tags.

Support semantics such as:

```
ANY selected tag
```

and:

```
ALL selected tags
```

Clarify visually which fingerprints are ultimately included.

The current machine fingerprint must always be included.

Show:

```
Included fingerprints: N
```

before building.

---

# 49. SECURITY REVIEW SCREEN

Before build, provide a final security summary.

Example information:

```
MACHINE AUTHORIZATION
Mandatory
4 trusted fingerprints

PASSPHRASE
Mandatory
Memory-hard KDF
Configured

GEOLOCATION
Enabled
Tolerance: 100m

TIME
Enabled
Every day 14:00
±15 minutes

EXECUTION
One-shot

REPOSITORY
Private
```

Do not reveal sensitive cryptographic material.

---

# 50. CREATOR / DIAGNOSTIC MODE

Provide a creator/diagnostic mode where appropriate.

The UI must explicitly state:

> Creator Mode is for diagnostics and development visibility. It does not provide additional security or bypass authorization.

Creator mode must NOT weaken security.

It may expose additional diagnostic information that is deliberately hidden from ordinary hostile execution.

Normal runtime failure messages should avoid becoming an authorization oracle.

---

# 51. FAILURE MESSAGES

Normal runtime mode should avoid overly detailed failure reasons.

Prefer:

```
Authorization failed.
```

rather than:

```
Fingerprint passed.
Passphrase passed.
Location failed.
Time passed.
```

Detailed diagnostics may be available in creator/diagnostic mode where appropriate.

Do not expose information that materially helps an attacker discover which individual security gate is the only remaining obstacle.

---

# 52. LOCAL AUDIT LOG

Do NOT implement a persistent forensic/login-attempt system in the prototype.

That belongs to:

```
FUTURE — LICENSE SERVER
```

The prototype may use ephemeral/internal diagnostics necessary for debugging and legitimate operation, but do not build the future forensic infrastructure now.

Future server functionality may eventually include:

* login attempts
* unlock attempts
* detailed forensic telemetry
* authenticated time
* remote unlock
* 2FA
* licensing
* user-authorized forensic disclosure

Do not implement those now.

---

# 53. FUTURE LICENSE SERVER MARKERS

Where the architecture will eventually change, explicitly mark:

```
FUTURE — LICENSE SERVER
```

Examples:

* authenticated time
* license keys
* license validation
* 2FA authenticator
* remote authorization
* hostile-system unlock
* unlock notification
* centralized audit trail
* login attempt tracking
* forensic event collection
* recovery/failsafe mechanisms

Design interfaces now where useful, but do not implement the server.

---

# 54. CRYPTOGRAPHIC ENGINEERING RULES

Use established cryptographic primitives wherever possible.

Do not invent a new cryptographic primitive merely to make NYEDA sound novel.

Innovation should occur in:

* composition
* policy enforcement
* runtime architecture
* binding
* authorization flow
* anti-tamper architecture
* protected execution flow
* package construction
* security orchestration

If you design a custom construction:

1. clearly define it
2. define threat assumptions
3. identify what security property it provides
4. explain what it does NOT provide
5. test it
6. avoid making unsupported security claims

Use authenticated encryption.

Use domain separation.

Use versioned cryptographic formats.

Design for cryptographic agility.

---

# 55. KEY ZEROIZATION

Sensitive material should be explicitly managed.

Investigate:

* zeroization crates
* locked memory where appropriate
* minimizing copies
* preventing accidental cloning
* minimizing lifetime
* preventing logging
* preventing serialization

Sensitive values include:

* passphrases
* derived keys
* decrypted fingerprint data
* location-derived key material
* time-derived key material
* payload keys
* temporary symmetric keys

---

# 56. MEMORY SECURITY

Avoid unnecessary plaintext memory.

Use:

* bounded buffers
* explicit ownership
* controlled lifetimes
* zeroization
* memory locking where justified
* minimal copies

Do not make unrealistic guarantees about Rust memory safety eliminating all forensic memory extraction.

---

# 57. CRASH SAFETY

Ensure sensitive data does not accidentally leak through:

* panic messages
* error strings
* debug formatting
* logs
* telemetry
* crash dumps
* temporary files

Never log:

* passphrase
* derived keys
* plaintext payload
* raw fingerprint data
* raw coordinates

---

# 58. PACKAGE FORMAT

Design a versioned package format.

Conceptual structure:

```
NYEDA Package
├── Magic
├── Format Version
├── Package ID
├── Runtime Binding
├── Cryptographic Version
├── Compression Metadata
├── Encryption Policy
├── Protected Fingerprint Authorization
├── Manifest
├── Integrity Metadata
└── Encrypted Payload
```

Do not treat this exact structure as mandatory.

Improve it if a better architecture exists.

---

# 59. VERSION EVERYTHING

Version:

* package format
* fingerprint format
* encryption format
* KDF parameters
* runtime protocol
* repository protocol
* build protocol
* GitHub workflow format

Future versions must be able to reject unsupported versions safely.

---

# 60. ERROR HANDLING

Use explicit typed errors.

Avoid:

```
unwrap()
```

or:

```
expect()
```

in security-critical production paths unless rigorously justified.

Never silently downgrade security.

If a security assumption fails:

```
fail closed.
```

---

# 61. CANCELLATION

The builder must support cancellation.

Cancellation must not leave:

* plaintext payload
* plaintext temporary archive
* secrets
* unfinished sensitive artifacts

behind unnecessarily.

Design cancellation as a state machine rather than arbitrary thread termination.

---

# 62. THREADING

Use concurrency where it improves throughput.

Potentially:

* parallel file reads
* parallel compression
* chunk processing
* asynchronous GitHub operations
* log streaming

Do not parallelize security-critical state transitions merely for speed if doing so weakens correctness.

Use bounded concurrency.

---

# 63. TESTING

Create comprehensive tests.

At minimum:

### Unit tests

* fingerprint generation
* fingerprint serialization
* fingerprint authentication
* fingerprint comparison
* compression
* decompression
* encryption
* decryption
* KDF composition
* geolocation quantization
* accuracy validation
* time-window evaluation
* package parsing
* manifest generation
* manifest validation
* runtime binding
* integrity validation

### Property tests

For:

* compression/decompression
* encryption/decryption
* manifest round trips
* fingerprint serialization
* geographic quantization
* time policy behavior

### Integration tests

Full:

```
input
  ↓
package
  ↓
generated runtime
  ↓
authorized execution
  ↓
restored output
```

### Negative tests

Explicitly test:

* wrong fingerprint
* wrong passphrase
* wrong location
* poor location accuracy
* wrong time
* modified payload
* modified manifest
* modified runtime
* modified fingerprint package
* modified policy
* corrupted artifact
* interrupted extraction
* interrupted deletion
* missing platform APIs
* unavailable TPM
* unavailable location provider
* unavailable keychain

---

# 64. ADVERSARIAL SECURITY TESTING

After implementation, conduct an internal adversarial review.

For each protection:

```
What is the attacker trying to bypass?

What information do they possess?

What can they patch?

What can they observe?

What can they control?

What happens if they skip this check?

Does skipping the check actually produce useful plaintext?

Can the key still be obtained?

Can the payload be decrypted independently?

Can the runtime be transplanted?

Can the policy be modified?

Can the runtime be replaced?
```

Then strengthen the design.

Do not stop at the first implementation.

---

# 65. ANTI-RE ENGINEERING REVIEW

Create:

```
ANTI_RE_ANALYSIS.md
```

Document:

* threat model
* implemented techniques
* expected attack surface
* bypass assumptions
* limitations
* platform-specific limitations
* false-positive risks
* performance impact

Do not claim "unbreakable."

Use language such as:

> Raises the cost of analysis and bypass.

---

# 66. SECURITY DOCUMENTATION

Create at least:

```
README.md
ARCHITECTURE.md
SECURITY_ARCHITECTURE.md
THREAT_MODEL.md
CRYPTO_FORMAT.md
FINGERPRINT_SPEC.md
ANTI_RE_ANALYSIS.md
GITHUB_SECURITY.md
EULA.md
FUTURE_LICENSE_SERVER.md
DEVELOPMENT.md
```

---

# 67. PATENT-SENSITIVE DEVELOPMENT

Keep a clear distinction between:

```
prior art / reference concepts
```

and:

```
NYEDA-specific implementation.
```

Do not claim that every underlying primitive is novel.

Do not copy existing implementations unnecessarily.

Where an existing primitive/library is used, document it.

Where NYEDA introduces a novel composition or architecture, document the rationale independently.

Maintain an architecture decision record:

```
docs/decisions/
```

for major design decisions.

---

# 68. DEPENDENCY POLICY

Prefer mature, maintained Rust crates.

For security-sensitive dependencies:

* inspect licensing
* inspect maintenance status
* inspect known vulnerabilities
* avoid unnecessary dependencies
* pin appropriately where reproducibility matters
* document why the dependency exists

Do not write custom cryptography when an established audited primitive exists.

---

# 69. GUI TECHNOLOGY

Choose the best Rust-native desktop GUI architecture.

You may choose between appropriate technologies such as:

* egui/eframe
* Tauri with Rust backend
* another mature Rust-compatible desktop framework

Make the choice based on:

* performance
* cross-platform support
* security
* maintainability
* GUI richness
* startup time
* binary size
* native integration
* ability to create the required original UX

Do NOT choose a technology merely because it is fashionable.

The final security-sensitive logic must remain Rust-controlled.

---

# 70. PROJECT STRUCTURE

You may redesign the exact crate structure, but aim for clean separation.

Potential conceptual modules:

```
nyeda-client
nyeda-core
nyeda-crypto
nyeda-package
nyeda-fingerprint
nyeda-runtime
nyeda-runtime-generator
nyeda-github
nyeda-platform
nyeda-location
nyeda-time
nyeda-ui
```

Do not force this exact structure if a better architecture exists.

---

# 71. BUILD PIPELINE

The application should perform:

```
User Input
    ↓
Validate
    ↓
Fingerprint selection
    ↓
Encryption configuration
    ↓
Compression
    ↓
Package construction
    ↓
Runtime generation
    ↓
Cryptographic binding
    ↓
GitHub submission
    ↓
Actions build
    ↓
Live logs
    ↓
Artifact verification
    ↓
Final executable
```

Do not upload unnecessary plaintext data to GitHub.

Ideally GitHub only needs what is required to compile the generated runtime and produce the final artifact.

---

# 72. GITHUB DATA MINIMIZATION

Treat GitHub as an external build environment.

Minimize exposure of:

* plaintext files
* plaintext archive
* passphrase
* fingerprint data
* location data
* private keys
* decryption secrets

The GitHub build should compile the runtime without requiring access to plaintext payload contents wherever possible.

---

# 73. SECURITY STATE MACHINE

Design both Builder and Runtime around explicit state machines.

Example runtime:

```
INITIALIZING
   ↓
INTEGRITY_CHECK
   ↓
BOOTSTRAP_POLICY
   ↓
FINGERPRINT_ACQUISITION
   ↓
FINGERPRINT_AUTHORIZATION
   ↓
PASSPHRASE_ACQUISITION
   ↓
LOCATION_ACQUISITION [optional]
   ↓
TIME_ACQUISITION
   ↓
KEY_DERIVATION
   ↓
PAYLOAD_AUTHENTICATION
   ↓
DECRYPTION
   ↓
DECOMPRESSION
   ↓
EXTRACTION
   ↓
CLEANUP
   ↓
SUCCESS
```

Any failure:

```
FAIL_CLOSED
   ↓
SENSITIVE_STATE_CLEANUP
   ↓
OPTIONAL_SELF_DESTRUCTION
   ↓
EXIT
```

Improve this if your implementation reveals a stronger state model.

---

# 74. DO NOT OVERFIT TO THE CURRENT SPECIFICATION

You have explicit permission to improve the internal architecture.

If you discover a better way to implement a requested function:

**Use it.**

You may redesign:

* cryptographic composition
* package format
* runtime generation
* GitHub build mechanism
* anti-tamper architecture
* fingerprint representation
* UI interaction
* concurrency model
* memory model
* platform abstraction

provided:

1. externally intended functionality remains intact,
2. security is not weakened,
3. the architecture becomes clearer/stronger,
4. the change is documented.

Do not blindly obey an inferior implementation suggestion simply because it appeared in this prompt.

---

# 75. DO NOT INVENT SECURITY THEATER

Never add a security mechanism solely because it sounds impressive.

For each security mechanism answer:

```
What threat does this mitigate?

What does it protect?

What can bypass it?

What is its performance cost?

What is its false-positive risk?

Is there a stronger alternative?
```

If it does not provide meaningful security, remove it.

---

# 76. FINAL SECURITY PRINCIPLE

NYEDA should be thought of as layered protection:

```
Layer 1
Package confidentiality

Layer 2
Machine authorization

Layer 3
Passphrase protection

Layer 4
Optional location factor

Layer 5
Optional time factor

Layer 6
Runtime integrity

Layer 7
Anti-tamper

Layer 8
Anti-analysis / anti-RE

Layer 9
Memory hygiene

Layer 10
Temporary-data minimization

Layer 11
Best-effort destruction
```

No individual layer is expected to be perfect.

The objective is defense in depth.

---

# 77. IMPLEMENTATION PROCESS

Do NOT immediately start writing hundreds of files.

First:

## Phase 1 — Architecture

Produce:

* architecture
* threat model
* trust boundaries
* data flow
* package format
* key hierarchy
* state machines
* platform abstraction
* GitHub architecture
* anti-RE architecture
* UI architecture

Then identify contradictions.

Resolve them.

---

## Phase 2 — Security design

Produce:

* cryptographic specification
* key lifecycle
* fingerprint specification
* geolocation specification
* time specification
* package format
* runtime binding
* integrity model
* anti-tamper model
* anti-RE model

Then perform an adversarial review.

---

## Phase 3 — Skeleton

Create the actual Rust workspace.

Ensure:

* builds cleanly
* tests run
* GUI launches
* platform abstractions compile
* GitHub layer is isolated
* security modules are isolated

---

## Phase 4 — Core functionality

Implement:

* EULA
* file selection
* fingerprint management
* compression
* encryption
* package format
* runtime generation

---

## Phase 5 — GitHub

Implement:

* authentication
* repository creation
* public/private switching
* key generation
* repository secrets
* workflow creation
* build triggering
* live logs
* artifact retrieval
* artifact verification

---

## Phase 6 — Runtime

Implement:

* integrity
* fingerprint acquisition
* protected fingerprint decryption
* authorization
* passphrase
* location
* time
* key derivation
* decryption
* decompression
* extraction
* cleanup
* optional destruction

---

## Phase 7 — Hardening

Perform the adversarial anti-RE/anti-tamper review.

Do not simply mark a checklist complete.

Attempt to break the architecture conceptually and, where appropriate, through controlled local testing.

---

## Phase 8 — Performance

Benchmark:

* startup
* fingerprint generation
* compression
* encryption
* package construction
* GitHub upload
* build time
* artifact retrieval
* runtime startup
* decryption
* decompression
* extraction

Identify actual bottlenecks.

---

# 78. DELIVERABLE QUALITY

Do not produce a toy prototype with:

```
TODO
fake encryption
mock GitHub API
placeholder fingerprint
fake location
fake runtime
fake anti-RE
```

unless something genuinely cannot be implemented yet.

If something cannot be implemented reliably on a particular platform:

1. isolate it,
2. document why,
3. implement the strongest legitimate alternative,
4. make the limitation explicit.

---

# 79. NO FAKE SECURITY

Never write:

```
// secure encryption here
```

with placeholder code.

Never use:

```
XOR
```

as encryption.

Never use:

```
SHA256(password)
```

as password protection.

Never pretend:

```
delete(file)
```

is secure deletion.

Never pretend:

```
hide_string()
```

is cryptographic protection.

Never pretend:

```
debugger_detected()
```

is anti-reverse engineering.

Never make unsupported security claims.

---

# 80. FUTURE LICENSE SERVER

Do not implement.

Only architect extension points.

Document:

```
FUTURE — LICENSE SERVER
```

Potential future functionality:

* license keys
* license authentication
* authenticator/2FA
* authenticated server time
* remote unlock
* hostile-system unlock
* immediate notification
* login-attempt monitoring
* detailed forensic records
* controlled data requests
* authenticated recovery
* failsafe/recovery mechanisms

These are explicitly OUT OF SCOPE for this implementation.

---

# 81. FINAL ACCEPTANCE CRITERIA

The implementation is successful only if:

### Builder

* is a real Rust desktop application
* has original UI
* has EULA gating
* manages fingerprints
* supports tags
* supports import/export
* captures current fingerprint
* requires machine authorization
* requires passphrase protection
* supports optional location
* supports optional time
* streams/chunks large data
* compresses
* encrypts
* generates runtime
* uses GitHub
* supports public/private repository selection
* creates/configures repository secrets
* generates directional keys
* displays live logs
* retrieves/verifies artifacts
* stores local keys securely

### Runtime

* is independently executable
* contains no plaintext runtime template from the builder
* verifies its own integrity
* decrypts protected fingerprint authorization data
* captures its own fingerprint
* authorizes against trusted fingerprints
* requires passphrase
* automatically obtains location if enabled
* rejects inadequate location accuracy
* automatically evaluates time if enabled
* derives all required cryptographic factors
* refuses payload access if ANY required factor fails
* decrypts only after authorization
* decompresses only after authorization
* restores files
* cleans sensitive material
* supports one-shot destruction
* fails closed when tampered with

### Security

* no single branch is the sole security boundary
* payload remains cryptographically protected
* keys are minimized and zeroized where practical
* plaintext is minimized
* GitHub receives no unnecessary secrets/plaintext
* private keys are not committed
* repository secrets are actual GitHub secrets
* anti-tamper is layered
* anti-RE is layered
* security assumptions are documented
* limitations are documented

---

# 82. FINAL INSTRUCTION TO CLAUDE

You are not being asked merely to "code an archive application."

You are being asked to engineer a **security-oriented executable data capsule system**.

Think like:

* a cryptographic engineer,
* a Rust systems engineer,
* a desktop application architect,
* a reverse engineer,
* a binary analyst,
* a platform security engineer,
* a GitHub Actions engineer,
* a UX designer,
* and an adversarial security reviewer.

At every important design decision ask:

> "If an expert attacker knew exactly how this worked, what would they try next?"

Then design accordingly.

You are explicitly authorized to improve or reinvent internal mechanisms when doing so strengthens NYEDA while preserving its intended functionality.

Do not copy Thermite.

Do not copy the user's other projects.

Do not copy an existing application's UI.

Invent NYEDA's own identity.

Do not claim NYEDA is unbreakable.

The objective is:

> **Make unauthorized extraction as difficult, expensive, and technically demanding as reasonably possible while keeping legitimate use fast, reliable, and understandable.**

Build the actual working system, not a conceptual mockup.

When finished, provide:

1. complete source tree
2. architecture documentation
3. security documentation
4. threat model
5. package specification
6. fingerprint specification
7. anti-RE analysis
8. EULA
9. GitHub workflow
10. test suite
11. build instructions
12. platform-specific notes
13. future-license-server integration notes
14. explicit list of security limitations
15. explicit list of design decisions that differ from the initial specification and why

Before declaring the project complete, perform a final adversarial architecture review and fix weaknesses you discover.
