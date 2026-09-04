I believe the project context and/or current architecture documentation mentions the future License Server and explicitly instructs that it is **not part of the current implementation scope**.

That remains true.

However, the License Server resolves several important limitations and shortcomings of the current **Prototype-I architecture**, which is an **unlicensed, standalone variant intended for internal reference, architecture development, and demonstration purposes**.

I now want you to **draft a separate, serious, comprehensive architecture and design documentation set for the future NYEDArch License Server**.

## CRITICAL SCOPE BOUNDARY

**DO NOT IMPLEMENT ANY PART OF THE LICENSE SERVER.**

Do not:

* create a backend,
* create a frontend,
* create a database,
* create API routes,
* create migrations,
* add crates,
* modify the current Rust workspace,
* add dependencies,
* modify NYEDA/NYEDArch Builder,
* modify the Runtime/Capsule,
* modify cryptographic code,
* create placeholders,
* create TODO implementations,
* create stubs,
* scaffold projects,
* generate deployment files,
* start containers,
* create CI/CD workflows,
* modify the current application architecture in code.

This task is **ARCHITECTURE, DESIGN, DOCUMENTATION, AND TECHNICAL PLANNING ONLY**.

The License Server must remain entirely out of the current implementation scope.

You may propose future interfaces and integration contracts, but they must remain documented designs only and must not be implemented.

---

# FIRST: ASK QUESTIONS

Before drafting the final License Server documentation, carefully inspect the entire existing NYEDArch project context and architecture.

Then identify any genuinely unresolved architectural or product questions that materially affect the License Server.

**Ask me those questions before finalizing the documentation.**

Do not ask me to reconfirm decisions already present in the project context.

Do not ask unnecessary implementation-level questions that you can reasonably solve yourself.

If a design decision is an implementation detail and does not alter the intended product behavior or security semantics, make the strongest reasonable decision yourself and document the rationale.

If the answer materially changes:

* security semantics,
* licensing semantics,
* user-visible behavior,
* pricing/entitlement structure,
* privacy model,
* online/offline policy,
* account recovery,
* Capsule execution policy,

ask me first.

---

# OBJECTIVE

Design a **state-of-the-art licensing, identity, authorization, distribution, telemetry, security monitoring, and audit architecture** for NYEDArch.

The current standalone prototype has limitations because it has:

* no license authority,
* no online identity authority,
* no central revocation,
* no authenticated server time,
* no centralized authorization,
* limited hostile-system recovery options,
* no centralized forensic/audit infrastructure,
* no account-level security monitoring,
* no license enforcement infrastructure,
* no remote control over Capsule authorization. and more (that u know or found or will find)

The future NYEDArch License Server should address these limitations where technically and realistically possible.

Do not make fake or impossible claims.

Do not invent "security magic."

Every proposed capability must be:

1. technically achievable,
2. architecturally plausible,
3. realistically implementable,
4. clearly separated from capabilities that only raise security difficulty rather than guarantee security.

---

# THE FUNDAMENTAL ARCHITECTURAL CHANGE

Prototype-I is primarily standalone.

Later on it will be an actively online architecture.

The future License Server should integrate deeply with:

1. the NYEDArch Builder/client application,
2. the generated NYEDArch Capsule/runtime,
3. account and identity management,
4. licensing and entitlements,
5. authenticated time,
6. MFA,
7. software distribution,
8. audit logging,
9. security monitoring,
10. license revocation,
11. authorization and Capsule execution.

The future client application should no longer be treated as a pseudo offline application.

The future Capsule must also participate in the online authorization architecture.

---

# MANDATORY ONLINE POLICY FOR THE FIRST LICENSED VERSION

For the first NYEDArch licensed architecture:

> **Internet connectivity is mandatory.**

Both:

* the Builder/client, and
* the generated Capsule/runtime

must require communication with the License Server according to the defined authorization protocol.

If internet connectivity is unavailable:

```text
Builder:
    FAIL CLOSED
    No operation requiring server authorization proceeds

Capsule:
    FAIL CLOSED
    Protected payload is not released
```

the capsule must have a cli/gui (or however it is designed) failsafe that uses the MFA code and license to do verifications + user gets an email that they have to approve = opening the capsule without any checks (they still need to enter passphrase + location + time as manual input or the server can store keys idk which is registered during build.. whatever is best and secure) -> Architect this yourself with best security (this feature is for cases when legitimate user updates hardware or is in a system they trust but was not originally added to the fingerprint list)

Do not silently fall back to offline authorization.

Do not add offline grace periods unless I explicitly approve them.

The architecture may document:

> FUTURE DESIGN POSSIBILITY — OFFLINE / GRACE / LIMITED-CONNECTIVITY MODES

but these must not be part of the first licensed design.

---

# CAPSULE LICENSE REQUIREMENT

A generated Capsule must itself require valid server-side authorization to execute and release its payload.

The Capsule should not merely check whether the Builder had a license during creation.

The Capsule should have its own runtime authorization relationship with NYEDArch.

Design a serious protocol for:

```text
Capsule startup
    ↓
Runtime integrity/security initialization
    ↓
Authenticated connection to NYEDArch
    ↓
Capsule identity verification
    ↓
License/entitlement verification
    ↓
Server-side policy evaluation
    ↓
Authenticated authorization response
    ↓
Local authorization factors
    ↓
Payload key release/unwrapping
```

The server authorization must be meaningfully tied into the cryptographic authorization model.

Do not design the system so that an attacker merely patches:

```text
if server_authorized
```

and obtains the payload.

The License Server must contribute meaningful authorization material or cryptographically authenticated policy that cannot be replaced by a simple patched boolean.

However, also do not design the server as a magical holder of plaintext payloads or passphrases.

Preserve the fundamental NYEDArch confidentiality model.

---

# INTERNET CONNECTION AND CAPSULE AUDIT EVENTS

The future Capsule should communicate significant lifecycle and security events to NYEDArch.

Architect this carefully.

Potential events include:

* Capsule launch attempt,
* authorization request,
* authorization success,
* authorization failure,
* fingerprint authorization result category,
* location authorization result category,
* time authorization result,
* license failure,
* integrity/tamper detection,
* repeated failures,
* suspicious execution environment,
* payload extraction success,
* extraction failure,
* cleanup,
* self-destruction initiation,
* self-destruction result where technically observable,
* abnormal termination,
* repeated execution patterns.

However:

**Do not claim that the server can receive an event that the Capsule never successfully sends.**

For example, if self-deletion destroys the executable before a final event reaches the server, the system must honestly distinguish between:

```text
self-deletion requested
```

and:

```text
self-deletion completion cryptographically confirmed by the server
```

Architect reliable telemetry honestly.

Consider:

* durable event queues,
* ordered lifecycle states,
* event acknowledgements,
* retries,
* idempotency,
* sequence numbers,
* signed events,
* tamper evidence,
* monotonic execution/session identifiers,
* server acknowledgements,
* explicitly documented limits of reporting.

---

# IDENTITY AND ACCOUNT SYSTEM

Design a complete user identity architecture.

Users must be able to:

* create accounts,
* verify their identity/contact method as appropriate,
* manage profile information,
* manage active sessions,
* view login history,
* view security events,
* manage licenses,
* manage registered authenticators,
* manage trusted devices where appropriate,
* revoke sessions,
* initiate recovery under a carefully designed process,
* download appropriate NYEDArch software.

There is **no free tier**.

Do not design a permanently free plan.

Account creation may occur before license purchase if you believe that is the strongest user-experience/security model, but no account should receive unrestricted product functionality without an appropriate entitlement.

Design and justify the distinction between:

* unauthenticated visitor,
* registered account,
* authenticated account,
* license owner,
* administrator,
* support personnel,
* security operations personnel.

Do not casually grant administrative access to support personnel.

---

# MANDATORY MFA

MFA enrollment must be mandatory.

After initial account creation and before the account becomes fully operational, the user must be required to register an authenticator.

Support standards-based authenticator applications through a secure and interoperable mechanism.

Examples include:

* Google Authenticator,
* Apple Passwords,
* other compatible authenticator applications.

Do not force users to rely on a proprietary NYEDArch mobile application for basic MFA.

Design a modern MFA hierarchy.

At minimum, investigate and architect:

* TOTP interoperability,
* phishing-resistant passkeys/WebAuthn,
* hardware security keys where appropriate,
* recovery mechanisms,
* authenticator replacement,
* anti-account-takeover controls.

The architecture should strongly consider **passkeys/WebAuthn as a preferred high-security authentication mechanism**, while supporting authenticator-app enrollment where required for compatibility.

Do not rely solely on TOTP if a stronger architecture is realistically possible.

---

# LICENSE KEY POLICY

The license key must be shown to the user **only once**.

After initial display, the full plaintext license key must not be retrievable again through the dashboard.

The architecture must define:

* how the key is generated,
* entropy requirements,
* format,
* how it is displayed,
* why it is displayed only once,
* how the server stores license-key verification material,
* whether the server stores plaintext keys,
* whether the user can reissue/rotate/revoke a license,
* what happens when a key is lost,
* how account ownership is distinguished from possession of a key.

Do not make the raw license key the sole security credential.

The system should remain secure even if:

```text
license key is known
```

without appropriate account authentication and other required authorization factors.

Think carefully about whether the product should use:

```text
human-visible license key
```

as:

* bootstrap credential,
* activation credential,
* recovery credential,
* legacy/import compatibility credential,

rather than as the complete authorization mechanism.

Document the reasoning.

---

# LICENSE TIERS

Invent a serious commercial license model.

There is no free tier.

Create well-justified license tiers.

The base paid tier must contain the minimum essential capabilities required to honestly call NYEDArch a full security product.

Do not cripple fundamental security to upsell users.

Security basics should not be treated as luxury features.

Higher tiers may add meaningful professional capabilities such as:

* larger usage limits,
* increased Capsule/build capacity,
* additional protected assets,
* advanced policy controls,
* more Capsule instances,
* advanced audit retention,
* advanced forensic visibility,
* higher administrative controls,
* team/organization management,
* delegated administration,
* advanced security analytics,
* enterprise identity integration,
* advanced recovery controls,
* dedicated support.
* the geolock and time lock can be skipped from the base tier and as such.

These are examples, not mandatory decisions.

Invent the tier model yourself.

For each tier, document:

* intended user,
* included security capabilities,
* resource/build limits,
* Capsule/runtime limits,
* audit retention,
* support model,
* account/device management,
* organizational features,
* rationale.

The tier system must not create a false situation where:

> lower-paying users receive fundamentally insecure encryption.

Core cryptographic security should remain strong across all legitimate product tiers.

The fingerprint import/export + labeling + search is basically created so that both end users as well as enterprises can easily use it. for example as an end user i might only need 1-2 fingerprints based on how many systems i own.. but for enterprise the idea changes.. and lets take an HR scenario.. employee records should only be visible to HR and that particular employee the record belongs to. therefore during creation the enterprise makes it a policy to include all HR personel and that one employee's registered company machine fingerprints to be included..  paired with geolock and timelock, even owners cannot access it outside the office premise and outside work hours.. preventing both insider and outsider data leak attempts. Refine and document this example in all relevant documentations.

There is no permanent license.. for now lets offer 3-months, 6-months and 1-year plans only.. after which user requires to renew. 
---

# PAYMENT AND PURCHASE ARCHITECTURE

Architect a realistic purchase and entitlement flow.

Users should be able to:

```text
Account
    ↓
Choose license tier
    ↓
Secure checkout
    ↓
Payment confirmation
    ↓
Entitlement creation
    ↓
License issuance
    ↓
Mandatory MFA/account security completion
    ↓
Secure software access
```

Do not claim that NYEDArch itself should process raw payment card data unless there is a compelling reason.

Prefer an architecture that minimizes payment data exposure through a suitable payment provider/tokenized checkout model.

Document:

* payment boundary,
* webhook verification,
* idempotency,
* refund effects,
* chargeback handling,
* entitlement suspension,
* entitlement restoration,
* fraud considerations.

Do not select a specific payment provider unless justified.

---

# SOFTWARE DISTRIBUTION

Design a secure distribution system for:

* Windows MSVC,
* macOS Apple Silicon,
* general Linux.

The user portal should intelligently detect the user's current platform where realistically possible and suggest the appropriate download.

However:

* detection must not override user choice,
* users must be able to manually select another supported platform,
* platform detection is a convenience, not a security decision.

Architect:

```text
Operating system detection
    ↓
Architecture detection
    ↓
Recommended binary
    ↓
User confirmation/selection
    ↓
Cryptographically verifiable download
```

Every distributed binary must have a serious authenticity strategy.

Investigate and document:

* code signing,
* checksums,
* signed manifests,
* software update verification,
* release provenance,
* update channels,
* rollback protection,
* compromise recovery.

Do not falsely imply that Linux distribution is uniform across all distributions.

Design realistically.

---

# FUTURE NYEDARCH BUILDER INTEGRATION

The Builder/client should become deeply integrated with the License Server.

Design how the online server can address current prototype limitations.

Potential responsibilities include:

* user authentication,
* mandatory MFA,
* license verification,
* entitlement enforcement,
* authenticated time,
* account security monitoring,
* Capsule registration,
* Capsule policy issuance,
* Capsule revocation,
* remote security policy updates where cryptographically safe,
* audit retrieval,
* build authorization,
* GitHub/build environment authorization,
* key/certificate lifecycle where appropriate,
* recovery workflows.

The Builder must still retain strong local cryptographic boundaries.

Do not turn the server into a central repository of plaintext user payloads.

The server must not require knowledge of the user's passphrase or plaintext payload merely to authorize the product.

---

# ADDRESS CURRENT PROTOTYPE LIMITATIONS

The new documentation must identify and address the limitations of Prototype-I.

Do not simply say:

> "License server fixes this."

For every relevant limitation, document:

```text
Current Prototype-I limitation
    ↓
Future NYEDArch integration
    ↓
Technical mechanism
    ↓
Security improvement
    ↓
Remaining limitation
```

Important areas include:

* unauthenticated local time,
* lack of central revocation,
* no remote authorization,
* no account identity,
* no MFA,
* lack of authenticated event/audit history,
* lack of Capsule lifecycle visibility,
* lack of centralized breach/misuse detection,
* limited recovery options,
* no centralized license enforcement,
* lack of runtime-specific online policy,
* inability to react centrally to compromised identities,
* inability to perform immediate account/session revocation,
* lack of authenticated server time,
* lack of centralized security telemetry.

Be honest about what remains impossible or difficult even after a License Server exists.

---

# AUDIT AND FORENSIC ARCHITECTURE

Design detailed, user-specific audit trails.

However, distinguish clearly between:

1. user-visible activity,
2. security-sensitive audit logs,
3. internal security operations data,
4. privacy-sensitive telemetry,
5. immutable/tamper-evident audit data.

The user should be able to securely view relevant activity relating to their own account and assets.

Do not expose internal security intelligence or sensitive detection logic unnecessarily.

Consider:

* account login events,
* MFA enrollment/change events,
* password/passkey changes,
* session lifecycle,
* license events,
* Builder activity,
* build events,
* Capsule authorization events,
* policy changes,
* revocation events,
* security alerts,
* suspicious patterns.

The audit architecture should support:

* append-only or tamper-evident design,
* event ordering,
* event correlation,
* actor identity,
* session correlation,
* request identifiers,
* cryptographic integrity where justified,
* retention policies,
* export where appropriate,
* access control.

Do not claim that an audit log is immutable merely because it is stored in a database.

If using a tamper-evident or cryptographically chained design, document exactly what it guarantees and what it does not.

---

# SECURITY MONITORING AND MISUSE DETECTION

Design systems capable of identifying potentially suspicious behavior, misuse, compromise, or security incidents.

Examples may include:

* impossible travel,
* anomalous login patterns,
* repeated MFA failures,
* repeated recovery attempts,
* repeated license activation attempts,
* unusual Capsule execution patterns,
* excessive authorization failures,
* repeated fingerprint mismatches,
* repeated tamper signals,
* suspicious account/session patterns,
* credential stuffing indicators,
* abnormal API usage,
* unusual build behavior,
* possible license sharing,
* repeated environment anomalies.

Do not simply ban a user automatically based on one heuristic.

Design:

```text
signal
    ↓
risk evaluation
    ↓
correlation
    ↓
risk score/category
    ↓
appropriate response
```

Possible responses:

* allow,
* step-up authentication,
* temporary restriction,
* security notification,
* security review,
* entitlement suspension,
* administrative review.

Every automatic response should be proportional and documented.

Avoid turning NYEDArch into an opaque surveillance system.

If decided to Revoke a license it stays revoked forever and user has to buy a new one (remaining expiry time on the old license may be transfered based on why the license was revoked in the first place).

temporary restrictions increase exponentially. just an example.. first restriction may last say a week, the next one on the same license becomes a month, and so on. that makes it potentially harder to launch a brute force technique.

---

# SECURITY AND PRIVACY

Design the License Server according to:

* least privilege,
* zero trust,
* defense in depth,
* minimal data collection,
* purpose limitation,
* data separation,
* secrets isolation,
* strong authentication,
* strong authorization,
* secure defaults.

Define what data is:

```text
Required
Optional
Derived
Security-sensitive
User-visible
Internal-only
Short-lived
Long-lived
```

Do not collect everything simply because the architecture technically can.

Capsule telemetry must be designed with careful data minimization.

For example, prefer:

```text
fingerprint authorization succeeded
```

or a privacy-preserving event category where appropriate over indiscriminately uploading raw machine identifiers.

---

# SERVER-SIDE SECURITY ARCHITECTURE

Design the complete secure frontend/backend architecture.

Consider and document:

* frontend boundary,
* backend/API boundary,
* identity/authentication service,
* licensing/entitlement service,
* Capsule authorization service,
* audit service,
* telemetry ingestion,
* risk detection,
* notification service,
* payment integration boundary,
* download/release service,
* key management,
* secrets management,
* database boundaries,
* caching/session boundaries,
* asynchronous job/event processing.

Do not create a monolithic "backend does everything" design without justification.

However, also do not recommend dozens of microservices merely for fashion.

Choose the architecture based on:

* attack surface,
* maintainability,
* scalability,
* operational complexity,
* security isolation.

Document why.

---

# API SECURITY

Design future API security in detail.

Consider:

* modern TLS,
* authenticated sessions,
* short-lived access credentials,
* refresh/session rotation,
* token theft resistance,
* device/session binding where appropriate,
* replay resistance,
* request correlation,
* nonce/timestamp protections,
* API authorization,
* service-to-service authentication,
* rate limiting,
* abuse prevention,
* DDoS considerations,
* schema validation,
* versioning,
* idempotency,
* secure error handling.

Do not expose secrets through:

* URLs,
* logs,
* telemetry,
* browser storage where inappropriate,
* stack traces.

---

# KEY MANAGEMENT AND SECRETS

The License Server architecture must have a serious key-management model.

Document:

* application secrets,
* service credentials,
* signing keys,
* Capsule authorization keys,
* license issuance keys,
* audit integrity keys if applicable,
* key rotation,
* key versioning,
* key revocation,
* compromise response,
* separation of duties,
* HSM/KMS or equivalent realistic protection where justified.

Do not use one master secret for everything.

---

# CAPSULE-SERVER PROTOCOL

This is one of the most important sections.

Draft the future protocol in detail.

The Capsule must not merely do:

```text
POST /check-license
```

and trust:

```text
{ "authorized": true }
```

Design:

* Capsule identity,
* package identity,
* runtime identity,
* authenticated request,
* replay resistance,
* freshness,
* server challenge/response where appropriate,
* policy/version binding,
* authorization response authenticity,
* how the server contributes to the cryptographic authorization path,
* revocation,
* failure handling.

A patched conditional branch must not by itself provide the payload key.

The protocol must preserve:

> The payload itself remains cryptographically inaccessible unless the required authorization succeeds.

The License Server should strengthen this model rather than replace it with a remotely supplied boolean.

---

# AUTHENTICATED SERVER TIME

The current prototype relies on local system time.

The future NYEDArch architecture should address this.

Design authenticated time usage for:

* time-based encryption policies,
* Capsule execution windows,
* authorization expiration,
* replay resistance,
* audit ordering,
* security events.

Do not simply trust an arbitrary unauthenticated timestamp supplied by the Capsule.

Document how server time and client local time interact.

Consider network delay and clock skew honestly.

---

# BREACH AND INCIDENT RESPONSE

Design for the possibility that:

* an account is compromised,
* an MFA authenticator is lost,
* a license key leaks,
* a Builder device is compromised,
* a Capsule binary is stolen,
* a GitHub/build environment is compromised,
* a server signing key is compromised,
* a database is breached,
* an internal administrator is compromised,
* the payment provider integration is attacked.

For each major category define:

```text
Detection
Containment
Revocation
Recovery
User notification
Forensics
Key rotation
Service restoration
```

Do not pretend a License Server eliminates endpoint compromise.

---

# ADMINISTRATION AND INTERNAL ACCESS

Design administrative access with strong separation.

Define roles such as:

* customer support,
* security analyst,
* billing support,
* system administrator,
* emergency security operator,

only if justified.

Apply:

* least privilege,
* approval workflows,
* strong MFA,
* privileged session controls,
* privileged action logging,
* break-glass access,
* separation of duties.

Support personnel must not automatically have access to customer-sensitive data.

---

# ACCOUNT RECOVERY

Because mandatory MFA is required, account recovery is security-critical.

Design recovery carefully.

Do not make:

```text
forgot password
```

a trivial MFA bypass.

Consider:

* recovery codes,
* additional authenticators,
* delayed recovery,
* security notifications,
* step-up verification,
* recovery lock periods,
* cancellation windows,
* manual high-risk review.

Document the attack tradeoffs.

---

# DOCUMENTATION OUTPUT

Create a separate NYEDArch License Server documentation set.

Do not modify current implementation.

At minimum, produce:

```text
NYEDARCH_LICENSE_SERVER_OVERVIEW.md
NYEDARCH_LICENSE_ARCHITECTURE.md
NYEDARCH_IDENTITY_AND_MFA.md
NYEDARCH_LICENSE_AND_ENTITLEMENTS.md
NYEDARCH_BUILDER_INTEGRATION.md
NYEDARCH_CAPSULE_AUTHORIZATION.md
NYEDARCH_CAPSULE_SERVER_PROTOCOL.md
NYEDARCH_AUDIT_AND_FORENSICS.md
NYEDARCH_SECURITY_MONITORING.md
NYEDARCH_KEY_MANAGEMENT.md
NYEDARCH_DISTRIBUTION_AND_UPDATES.md
NYEDARCH_PAYMENTS_AND_PURCHASES.md
NYEDARCH_THREAT_MODEL.md
NYEDARCH_INCIDENT_RESPONSE.md
NYEDARCH_PRIVACY_AND_DATA_MODEL.md
NYEDARCH_FUTURE_INTEGRATION_ROADMAP.md
```

You may add more documents if necessary.

These documents must be:

* detailed,
* internally consistent,
* technically realistic,
* security-focused,
* explicit about trust boundaries,
* explicit about data flows,
* explicit about cryptographic responsibilities,
* explicit about limitations.

Use diagrams, sequence diagrams, trust-boundary diagrams, state diagrams, tables, and API/protocol pseudostructures where useful.

---

# UPDATE THE CURRENT NYEDARCH DOCUMENTATION

After drafting the separate future License Server architecture documentation, do not leave the current architecture documentation filled with vague statements such as:

> "shortcoming to be fixed later."

Instead, update references to use a clear future-integration model.

Where the current prototype has a limitation, reference the planned NYEDArch future architecture and specify:

```text
CURRENT PROTOTYPE BEHAVIOR
    ↓
FUTURE NYEDARCH LICENSE SERVER INTEGRATION
    ↓
WHAT THE FUTURE INTEGRATION CHANGES
```

Examples include:

* local time → authenticated server time,
* local-only authorization → online server-backed authorization,
* no revocation → centralized revocation,
* no central recovery → controlled account/license recovery,
* no runtime audit → authenticated Capsule event architecture,
* no centralized security monitoring → risk detection and incident response.

Do not change current implementation.

Do not pretend these features already exist.

Use explicit labels such as:

> FUTURE — NYEDArch LICENSE SERVER INTEGRATION

---

# DO NOT CREATE A FAKE DESIGN

This documentation must not be a collection of fashionable technology names.

For every significant recommendation:

1. explain what problem it solves,
2. explain how it works,
3. define the trust boundary,
4. define what data it handles,
5. define what security guarantee it provides,
6. define what limitation remains.

Do not claim:

* absolute tamper-proofing,
* guaranteed breach detection,
* impossible reverse engineering,
* guaranteed self-deletion confirmation,
* impossible license bypass,
* perfect device identification,
* perfect geolocation,
* perfect fraud detection.

Be precise.

---

# ORIGINALITY

The NYEDArch License Server architecture and user experience should be independently designed.

Do not copy:

* Thermite,
* NYEDA's existing UI,
* another commercial security product,
* another SaaS dashboard.

Thermite remains a technical reference for GitHub-related orchestration concepts only.

NYEDArch must have its own:

* identity model,
* security architecture,
* portal UX,
* terminology,
* workflows,
* audit model,
* authorization model.

---

# FINAL HARD BOUNDARY

After completing the documentation:

**STOP.**

Do not begin implementation.

Do not say:

> "I have created the architecture, so I will now begin scaffolding it."

Do not create even a minimal proof of concept.

The output of this task is the future architecture and design documentation only.

The current implementation remains focused on Prototype-I.

The purpose of this documentation is to ensure that Prototype-I can later evolve into the full NYEDArch online architecture without requiring the security model to be reinvented from scratch.

Before drafting the final documentation, ask me any genuinely necessary questions that materially affect the architecture. Otherwise, make the strongest technically defensible architectural decisions yourself.
