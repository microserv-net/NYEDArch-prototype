# NYEDArch Builder ↔ License Server Integration

> DESIGN DOCUMENTATION ONLY. Nothing here is implemented.

## 1. What changes for the Builder

| Prototype-I | Licensed |
|---|---|
| Runs standalone | Requires authenticated session; fails closed offline |
| No identity | Account + mandatory MFA |
| Local time only | Authenticated server time |
| Capsules are anonymous | Capsules are registered and revocable |
| No usage limits | Entitlement enforced per tier |
| GitHub token held locally | Still local; build authorization additionally checked |

## 2. What must not change

The Builder retains its local cryptographic boundary. Specifically, the server
must **never** receive:

- plaintext files or the plaintext archive;
- the passphrase or any passphrase-derived material;
- raw fingerprint signals (only opaque digests, and only where needed);
- raw coordinates (only quantized region identifiers, and only if required);
- the payload key or any factor other than its own share.

The server authorizes the *product*; it does not gain the ability to open the
user's data. If a future feature would require the server to hold passphrase or
payload material, that feature is wrong and should be redesigned.

## 3. Capsule registration at build time

```
builder                                   server
  |-- authorize_build(tier, target, protections) ------->|
  |<-- build_grant(build_id, quota_state) ---------|
  |                                                |
  | seal package locally (unchanged)               |
  |                                                |
  |-- register_capsule(capsule_id, package_commit, |
  |     runtime_commit, protection_policy, build_id) --->|
  |                                                | mint S_server,
  |                                                | wrap via KMS, store
  |<-- registration(S_server, policy_signature) ---|
  |                                                |
  | compose K_payload including the server factor  |
  | embed capsule_id + grant verification key      |
```

`S_server` is delivered once, used in composition, and zeroized. The Builder does
not retain it — if it did, a compromised Builder machine would hold the server
factor for every capsule it ever produced.

## 4. Protection availability is enforced server-side

Tier gating cannot be enforced only in the client UI, because the client is
software the user controls. The server refuses to register a capsule whose protection
policy exceeds the account's entitlement — a patched client that offers the
location protection on a Seal tier produces a capsule the server will not register, and
therefore one that cannot be built.

## 5. Quotas

Builds per term and fingerprints per capsule are enforced at registration. Quota
state is returned with each grant so the client can display remaining capacity
before a long operation rather than failing at the end of one.

## 6. Offline behaviour

**Fail closed.** No queuing of registrations for later, no provisional capsule
that becomes valid when connectivity returns. A capsule that exists is a capsule
the server knows about; there is no intermediate state.

The client should distinguish clearly in its interface between "you are offline"
and "your licence is invalid", because these have completely different remedies —
while still avoiding an authorization oracle in the *capsule's* runtime output.
The Builder is operated by the legitimate owner; the capsule is not.

## 7. GitHub build authorization

The remote build path is unchanged in its cryptography. Additionally:

- the server authorizes the build before dispatch (quota, tier, target);
- the build id is issued server-side and recorded, so provenance verification and
  audit share one identifier;
- artifact provenance verification remains a **client-side** check. The server
  does not become a trusted intermediary for artifacts, because that would create
  a new single point of compromise.
