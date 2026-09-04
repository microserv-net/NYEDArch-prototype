# NYEDArch Dossier — Comprehensive Security, Red-Team and License-Server Documentation Update

Update the existing NYEDArch Technical Documentation, Prototype Record & Evaluation Dossier in place.

Do not rewrite the architecture into something different, invent implemented capabilities, or silently convert future design into present functionality.

The document is confidential and may become substantially larger. **Do not optimize for length. Optimize for technical completeness, evidentiary quality, adversarial usefulness, and strict separation between implemented, experimentally verified, designed-but-not-implemented, and unresolved behaviour.**

Preserve existing accurate technical content unless it conflicts with the corrections below.

---

# 1. Correct the description of destructive failure — do not undersell it

The current dossier mentions self-destruction/one-shot behaviour, but the security model needs to be represented much more strongly and accurately.

Where applicable to the actual implementation, explicitly document that a security-critical failure can result in **destruction or loss of the protected capsule/specimen**, and that this is deliberately part of the anti-analysis/security model.

Reflect this consistently in:

* Threat Model
* Denial Behaviour
* Adversarial Review
* Anti-Tamper
* Known Limitations
* Red-Team Research
* Prototype-II architecture discussion

Do not describe failure as merely:

> "authorization denied and the application exits."

The attacker model must account for:

> **A failed hostile experiment may consume the attacker's specimen.**

However, remain precise about deletion guarantees.

Distinguish:

1. deletion requested;
2. deletion successfully completed according to the application;
3. application-controlled temporary/working data successfully removed;
4. OS/filesystem/storage limitations that prevent absolute guarantees;
5. remote revocation making a surviving capsule unusable.

Do not claim perfect physical erasure where the implementation cannot honestly guarantee it.

---

# 2. Expand the License Server architecture, with an explicit status label everywhere

All License Server material in the dossier is **Prototype-II — yet to be developed** unless explicitly stated otherwise.

Wherever the License Server is mentioned in the document, use a visible and consistent designation such as:

> **Prototype-II — yet to be developed**

Do not allow readers to mistake License Server architecture, workflows, telemetry, revocation, key shares, anomaly detection or related functionality for currently implemented Prototype-I functionality.

Update the existing License Server sections so that they accurately represent its role as an active future authorization and accountability authority.

Within the existing legal/product boundaries already established in the document, expand the intended architecture to cover:

* mandatory online connectivity for Prototype-II capsule operation;
* server-mediated authorization;
* server participation in cryptographic key composition rather than a simple boolean license check;
* server-side authorization and security-event logging;
* anomaly/oddity detection;
* owner notification;
* revocation;
* post-compromise controls;
* legitimate user recovery workflows;
* authenticated server time;
* license/account lifecycle management.

The server should be described strictly in terms of legitimate authorization, disclosed security telemetry, audit, anomaly detection, revocation, incident response, support/recovery, and account administration.

Do not blur future Prototype-II capabilities into the currently implemented standalone prototype.

---

# 3. Do not decide the first-use license-key/TOTP execution workflow yet

Remove the previously proposed requirement that the **first execution of a capsule in a new environment must require a license key + TOTP code**.

Do not document that workflow as a settled architecture decision.

The exact first-execution authentication mechanism remains intentionally undecided and will be resolved later.

You may document the already settled fact that:

> Account-level MFA is mandatory for License Server accounts.

Do not infer from this that a particular MFA mechanism is necessarily required at every capsule execution.

Likewise, do not invent a first-use enrollment protocol.

---

# 4. Explicitly document the one-license-key-per-user model and future recovery workflow

Add/retain the rule:

> **Each user can ever be issued only one license key.**

Renewal extends continued use under that user's existing license relationship; renewal does not create a new identity/key relationship.

A capsule must remain bound to the license identity under which it was legitimately created/registered. An unrelated user's valid license must not arbitrarily authorize another user's capsule.

Add the following as a clearly marked future feature under:

> **Prototype-II — yet to be developed**

### Future License Server Feature — Verified Capsule Recovery After Permanent Revocation

If a legitimate user's license has been permanently revoked and they have existing capsules whose data they legitimately need to recover, the user may submit a support request identifying the specific capsules requiring recovery.

The support workflow should:

1. verify the identity and legitimacy of the request;
2. verify ownership/association of the relevant capsules;
3. identify exactly which capsules require recovery;
4. permit the user's license relationship to authorize **only those specifically verified capsules**;
5. permit each approved capsule to be unlocked **exactly once**;
6. record the complete recovery event in the audit trail.

This must never become a universal master unlock.

Present this as one example of a broader principle:

> **Prototype-II must be extremely hostile to detected threats while remaining carefully recoverable for verified legitimate users.**

The recovery process should be narrow, auditable and deliberately inconvenient compared with normal operation.

---

# 5. Add the future "Land Mine" revocation feature

Add a clearly marked section:

> **Prototype-II — yet to be developed**
>
> **Future License Server Feature — Land Mine**

The intended workflow is:

1. Legitimate owner suspects or confirms compromise of a capsule.
2. Owner uses trusted/local copies to recover their legitimate data where necessary.
3. Owner marks the affected capsule as revoked.
4. License Server records the revocation.
5. Future server-backed use attempts involving that capsule are refused.
6. The attempted use generates the relevant security telemetry.
7. Owner notification/monitoring can occur according to the documented security model.
8. The revoked artifact remains unusable through the server-backed authorization path.

The Land Mine must be described as a **post-compromise defensive control**, not retaliation.

Document the limitations:

* it cannot recall plaintext already extracted;
* it cannot erase copies already made by an attacker;
* it cannot observe an attacker who never reaches the server;
* it depends on the affected artifact attempting server-backed operation;
* all behaviour remains subject to the legal/privacy/security model already documented.

Explain why this is strategically important:

> A capsule that was previously compromised does not necessarily remain a permanently useful stolen credential. The legitimate owner can later revoke it, converting future use attempts into a security event.

---

# 6. Generate a comprehensive final-architecture diagram

Create a new architecture diagram for the intended final licensed system.

Do not use a generic security flowchart.

The diagram should show the actual relationships between:

* final capsule;
* local integrity/authenticity checks;
* environment/security initialization;
* mandatory network dependency;
* Prototype-II License Server;
* capsule identity;
* account/license identity;
* server authorization;
* future server key share;
* local machine authorization;
* passphrase;
* optional location;
* optional time;
* cryptographic key composition;
* authenticated decryption;
* plaintext;
* cleanup/destruction;
* security telemetry;
* anomaly detection;
* owner notification;
* revocation;
* Land Mine;
* legitimate recovery workflow.

Clearly separate:

* local capsule operations;
* server-side operations;
* cryptographic dependencies;
* destructive failure paths;
* telemetry/audit paths;
* legitimate recovery paths.

The diagram should communicate the architecture at a glance to an experienced security reviewer.

---

# 7. Expand secure-deletion and self-destruction testing substantially

Create a dedicated adversarial secure-deletion test programme.

Test as many meaningful real-world failure cases as supported by the implementation, including:

* normal deletion;
* file open during deletion;
* read-only filesystems;
* permission failure;
* file locking;
* concurrent readers;
* concurrent writers;
* rename-before-delete;
* replacement/swap races;
* partial write;
* process interruption;
* crash during destruction;
* authentication failure;
* integrity failure;
* signature/runtime mismatch;
* environment mismatch;
* missing required service;
* repeated failure;
* copied capsule;
* renamed capsule;
* surviving temporary files;
* surviving temporary directories;
* platform-specific behaviour;
* filesystem-specific behaviour where practical.

For every case determine:

* whether deletion was requested;
* whether it succeeded;
* whether working material survived;
* whether the capsule remained usable;
* whether a copy remained usable;
* whether failure was detectable;
* what the implementation can honestly guarantee.

Explicitly separate:

**verified deletion**

from

**best-effort deletion**

from

**irreducible OS/filesystem limitations.**

Do not claim secure deletion merely because `remove()` returned success.

---

# 8. Expand adversarial experiments into a genuine attack laboratory

The existing branch-patching experiment is valuable. Turn it into one member of a much larger adversarial test catalogue.

Perform or propose realistic experiments against:

* authorization branches;
* policy flags;
* runtime binding;
* package transplantation;
* package substitution;
* manifest tampering;
* chunk reordering;
* chunk truncation;
* ciphertext modification;
* header modification;
* embedded constants;
* fingerprint records;
* machine identity;
* location provider;
* time source;
* environment detection;
* debugging/instrumentation;
* process state;
* memory;
* hooks;
* API/syscall behaviour;
* abnormal termination;
* copied/renamed artifacts;
* virtualization;
* dependency substitution;
* server-response manipulation;
* replay attempts;
* unauthorized license relationships;
* revoked capsules;
* server connectivity interruption;
* repeated failures;
* recovery paths.

For every experiment record:

**Attack scenario → objective → action → targeted boundary → intended prevention → observed behaviour → artifact state → telemetry → notification → plaintext exposure → timing → final status.**

Use real transcripts, traces, logs, screenshots, hashes and measurements whenever available.

Never fabricate results.

Unexecuted attacks must be marked:

> **PROPOSED EXPERIMENT**

Executed attacks must be marked with their evidence and actual result.

---

# 9. Build a dedicated "Red-Team Research / Attacker Case Study" section

Create a substantial red-team section that models how a real attacker approaches the **final NYEDArch artifact**.

The attacker receives a final capsule belonging to another party.

They do not receive:

* internal prototypes;
* source code;
* the Builder;
* internal repositories;
* the License Server.

Model the progression across:

* casual attacker;
* technically skilled attacker;
* intermediate reverse engineer;
* professional reverse engineer;
* senior/elite reverse engineer;
* dedicated offensive/security team;
* highly resourced/top-tier attacker.

For every class discuss:

* what they initially see;
* likely first attacks;
* what static analysis reveals;
* dangerous experiments;
* what happens when a failure destroys the specimen;
* mandatory online authorization in Prototype-II;
* server visibility;
* anomaly detection;
* owner notification;
* revocation;
* Land Mine;
* post-compromise survival;
* the runtime point at which plaintext must exist;
* what remains fundamentally difficult;
* what ultimately determines whether the attacker succeeds.

### Required conceptual framing

Introduce:

## "Race Against Time"

Explain that the attacker may face several clocks simultaneously:

* time to understand the artifact;
* time before a live interaction becomes suspicious;
* time before failure destroys the specimen;
* time before owner notification;
* time before revocation;
* time before an investigation begins;
* time before additional use attempts become evidence.

Do not reduce this to an arbitrary "24-hour limit."

The point is that **long preparation may be possible away from the live artifact, while the meaningful live attack can become a short, high-risk event.**

## "Failure Becomes Loss"

Explain that a normal reverse-engineering workflow values repeated experimentation because a failed hypothesis costs time.

Under NYEDArch, a security failure can potentially cost the attacker the specimen itself.

Therefore self-destruction is an **anti-iteration mechanism**.

Do not call it mathematically impossible to bypass.

### Tooling

Discuss, at a high level, legitimate red-team tooling categories:

* static reverse engineering;
* debuggers;
* binary patching;
* dynamic instrumentation;
* process/memory inspection;
* syscall/API tracing;
* virtualization/sandboxing;
* filesystem observation;
* network/protocol analysis;
* crash analysis;
* artifact comparison.

Describe why each category matters to the threat model without turning the document into an operational unauthorized-compromise guide.

### Attacker psychology

Include a subsection analysing how NYEDArch may change attacker behaviour:

* initial expectation of unlimited experimentation;
* frustration when specimens are lost;
* increasing conservatism;
* greater preparation before live execution;
* fear of server-visible activity;
* concern about triggering anomaly detection;
* pressure to make a successful interaction quickly;
* distinction between technical compromise and operational success.

Clearly label this as analytical modelling, not observed attacker testimony.

---

# 10. Expand the document's empirical test-case format

For major security tests, use detailed structured records containing:

| Field               | Requirement                                                                            |
| ------------------- | -------------------------------------------------------------------------------------- |
| Test ID             | Stable identifier                                                                      |
| Category            | Crypto / authorization / deletion / anti-tamper / platform / Prototype-II architecture |
| Scenario            | Realistic condition                                                                    |
| Setup               | Exact environment                                                                      |
| Preconditions       | Required state                                                                         |
| Action              | Test/attack                                                                            |
| Intended Behaviour  | Expected result                                                                        |
| Acceptance Criteria | Objective condition                                                                    |
| Observed Behaviour  | Actual result                                                                          |
| Artifact State      | Survived / destroyed / inaccessible / revoked                                          |
| Telemetry           | What was recorded                                                                      |
| Notification        | Whether applicable                                                                     |
| Plaintext Exposure  | Yes / No / Unknown                                                                     |
| Timing              | Measured latency                                                                       |
| Resource Usage      | Where meaningful                                                                       |
| Platform            | Linux / macOS / Windows                                                                |
| Evidence            | Trace/log/hash/screenshot                                                              |
| Status              | PASS / FAIL / PARTIAL / NOT RUN                                                        |
| Interpretation      | Security significance                                                                  |

Where appropriate, turn individual attack scenarios into mini case studies.

---

# 11. Add graphs and metrics where measurements exist

The dossier may become substantially larger.

Where real measurements exist, use them.

Possible metrics:

* authorization latency;
* failure latency;
* destruction latency;
* cleanup verification;
* memory usage;
* CPU usage;
* network overhead;
* Argon2id cost;
* attack attempts before specimen destruction;
* telemetry generation latency;
* owner-notification latency;
* revocation propagation;
* recovery workflow latency;
* cross-platform deletion behaviour.

Do not manufacture metrics.

Do not add graphs merely as decoration.

---

# 12. Clarify post-compromise behaviour and owner response

Document the post-compromise sequence as a proper security lifecycle.

A representative future Prototype-II sequence should cover:

```text
suspicious capsule activity
        ↓
server-side event
        ↓
oddity/anomaly detection
        ↓
owner notification
        ↓
owner confirms/suspects compromise
        ↓
legitimate copies used for data recovery if required
        ↓
capsule revoked
        ↓
Land Mine becomes active
        ↓
future server-backed use denied
        ↓
future attempt produces security telemetry
```

Keep detection, notification, audit preservation, revocation and lawful investigation as distinct concepts.

This is a **Prototype-II — yet to be developed** workflow.

Also make the important limitation explicit:

> If the attacker already extracted plaintext, NYEDArch cannot make that plaintext disappear.

The value of the post-compromise system is therefore containment, continued denial, detection and evidence—not time travel.

---

# 13. Add the product philosophy: hostile to threats, humane to legitimate users

Add a clear future-architecture principle:

> **Prototype-II should be hostile by default toward suspicious activity while remaining recoverable for verified legitimate users.**

Use these future workflows as examples:

### Threat side

* fail closed;
* record security events;
* notify legitimate owners;
* revoke affected capsules;
* Land Mine previously distributed compromised artifacts;
* prevent continued server-backed access.

### Legitimate-user side

* verify identity;
* verify capsule ownership;
* narrowly scope recovery;
* approve only specifically requested capsules;
* authorize recovery exactly once where intended;
* record every recovery operation.

This should not create a master override.

The goal is not to make legitimate users suffer because the system is hostile to attackers.

The goal is:

> **Extreme restriction for suspicious activity, extreme specificity for legitimate recovery.**

All of this is **Prototype-II — yet to be developed**.

---

# 14. FIRST IMPLEMENTATION STEP — Build and integrate TPM/Secure-Enclave-backed machine protection

Before making any other implementation changes, **make hardware-backed machine protection the first engineering task.**

The purpose is to address the current `S_machine` weakness explicitly documented in the dossier:

> In the current prototype, the per-machine `S_machine` secret is extractable from a held capsule and therefore is not a standalone confidentiality boundary.

The first implementation task must therefore be to design and implement the hardware-backed correction.

Where platform capabilities permit, investigate and implement the appropriate secure hardware/storage mechanism, including:

* TPM-backed sealing on supported Linux/Windows systems;
* Secure Enclave-backed key/secret protection on supported Apple systems;
* platform-specific secure storage semantics;
* non-exportability requirements where technically achievable;
* binding the machine factor to hardware-backed material rather than merely storing an extractable secret inside the package.

Do not claim that TPM/Secure Enclave makes machine identity mathematically unspoofable.

Explicitly document:

* what hardware root is trusted;
* what secret is hardware-protected;
* what operation is allowed;
* whether the secret is exportable;
* what happens after motherboard/hardware replacement;
* what happens if the secure hardware is unavailable;
* what happens in virtual machines;
* what happens on unsupported hardware;
* what recovery path exists.

The implementation must fail closed where the required hardware-backed capability is mandatory.

The existing software-only fallback must not silently become a security downgrade.

Create tests for:

* successful hardware-backed authorization;
* wrong machine;
* transplanted package;
* hardware replacement;
* unavailable TPM/Secure Enclave;
* permission failure;
* virtualization;
* secure-storage reset;
* extraction attempts;
* fallback prevention;
* cross-platform behaviour.

Update the dossier only with behaviour actually implemented and experimentally verified.

Future hardware/attestation capabilities that are not yet implemented must remain explicitly marked:

> **DESIGNED — NOT IMPLEMENTED**

---

# 15. Maintain strict implementation-status discipline

Throughout all revised material use explicit status labels such as:

**IMPLEMENTED AND TESTED**

**IMPLEMENTED BUT NOT FULLY VERIFIED**

**DESIGNED — NOT IMPLEMENTED**

**PROPOSED EXPERIMENT**

**KNOWN LIMITATION**

**OPEN QUESTION**

Every License Server reference that represents future functionality must be marked:

> **Prototype-II — yet to be developed**

Do not allow current Prototype-I results to be presented as Prototype-II capabilities.

Do not turn the red-team threat model into claimed empirical evidence unless the attack has actually been executed.

---

# 16. Perform a full document-wide consistency audit

After all changes, audit the entire dossier.

Specifically verify:

* total test count;
* individual test-register arithmetic;
* number of weaknesses;
* implementation status;
* section numbering;
* cross-references;
* destructive-failure terminology;
* secure-deletion claims;
* Prototype-I terminology;
* Prototype-II terminology;
* License Server status labels;
* MFA terminology;
* TOTP references;
* one-license-key-per-user rule;
* license renewal;
* permanent revocation;
* future recovery workflow;
* Land Mine;
* server key share;
* hardware-backed machine protection;
* TPM/Secure Enclave implementation status;
* audit/telemetry claims;
* anomaly-detection claims;
* owner-notification claims;
* revocation claims;
* limitations;
* benchmark figures;
* test results.

The dossier must not contain contradictory statements about what is implemented.

---

# 17. Preserve the central philosophy

Do not allow the expanded security material to redefine NYEDArch as conventional DRM.

Retain the central idea:

> **From creating software to protect data, to creating software that enables data to protect itself.**

The documentation should show the progression:

```text
passive file
    ↓
encrypted capsule
    ↓
cryptographically conditioned reconstruction
    ↓
multiple required authorization contributions
    ↓
runtime/package binding
    ↓
failure can become destructive loss
    ↓
Prototype-II — yet to be developed
remote authorization
    ↓
server-side accountability
    ↓
revocation
    ↓
post-compromise containment
```

Archiving/extraction is the visible surface.

The deeper subject is the relationship between:

**data + authorization + execution conditions + survivability + accountability.**

---

# 18. Final deliverable expectation

The final dossier should read as a serious confidential technical, security and adversarial evaluation record.

Do not optimize for brevity.

A reviewer should be able to determine:

1. what NYEDArch is;
2. how the cryptographic architecture works;
3. what an attacker immediately obtains;
4. what is implemented today;
5. what has actually been experimentally verified;
6. what attacks were performed;
7. what those attacks demonstrated;
8. which failures can destroy the specimen;
9. how secure deletion behaves under hostile conditions;
10. how the future Prototype-II architecture changes the attack model;
11. how online authorization, server-side telemetry, anomaly detection and revocation affect post-compromise behaviour;
12. how legitimate recovery is handled without creating an unrestricted bypass;
13. what remains vulnerable;
14. what remains future work;
15. and exactly how confident the project should be about every security claim.

Make the resulting dossier **more empirical, more adversarial and more precise—not more promotional.**
