# NYEDArch Capsule Authorization — Key-Share Model

> DESIGN DOCUMENTATION ONLY. Nothing here is implemented.

## 1. The requirement

The server must contribute **cryptographic material**, not a permission flag.
Prototype-I demonstrated experimentally that a patched branch yields nothing
because the key is composed from factor contributions. The licensed design must
preserve exactly that property across the network boundary.

Rejected design:

```
POST /check-license  ->  { "authorized": true }   // patch the handler, done
```

## 2. Construction

The server holds a per-capsule key share `S_server`, minted at build time,
wrapped by HSM/KMS, and stored in the isolated share store. It becomes a fifth
domain-separated contribution to the existing composition:

```
ikm = HKDF(salt, machine_secret,  "factor:machine")
    || HKDF(salt, passphrase_key,  "factor:passphrase")
    || HKDF(salt, location_cell,   "factor:location")   // iff enabled
    || HKDF(salt, time_window,     "factor:time")       // iff enabled
    || HKDF(salt, S_server,        "factor:server")     // licensed builds

K_payload = HKDF(salt = package_salt, ikm = ikm, info = context)
```

`S_server` is 32 bytes of CSPRNG output. It is **not** derived from account,
licence, or capsule identity — deriving it from known values would let anyone who
learns the derivation inputs reconstruct it offline.

## 3. What this achieves, and what it does not

| Property | Status |
|---|---|
| Patching the response handler yields the payload | **Prevented.** Without `S_server` the composed key is wrong and AEAD fails |
| Server can decrypt a payload alone | **Prevented.** It holds one factor; passphrase and machine factors are never sent to it |
| Server can deny a capsule permanently | **Achieved.** Withholding the share is unforgeable denial |
| Capsule works offline | **Not achieved, by decision.** This is the accepted cost |
| Capsule survives loss of the share store | **No.** Unrecoverable by construction |

## 4. Share release is not share exposure

The share is never handed to the capsule as a durable value it could cache and
replay on a later, unauthorized run. Release is bound to a fresh server
challenge and a single session:

```
S_effective = HKDF(session_salt, S_server, "factor:server" || session_nonce)
```

The capsule receives `S_effective`, which is valid only for the session in which
it was issued. A capsule that records it gains nothing on a subsequent run,
because the next run derives a different session nonce.

**Honest limit:** an attacker who instruments a *successful authorized* run can
capture `S_effective` in memory at that moment. That does not help them open the
capsule later on an unauthorized machine, because the machine and passphrase
factors are still required — but it is not zero, and it is stated rather than
glossed. Memory capture during a legitimate unlock remains outside what any
software design can prevent.

## 5. Revocation semantics

Revocation is enforced by refusing to release the share. This is meaningfully
stronger than a policy flag: there is no local state an attacker can patch to
reconstruct a value the server never sent.

| Event | Effect on future authorization |
|---|---|
| Licence expired | Share withheld — capsules stop opening (settled decision) |
| Licence revoked | Share withheld permanently |
| Capsule revoked individually | That capsule's share withheld; others unaffected |
| Account suspended | All shares for that account withheld for the suspension period |

**What revocation cannot do:** reach payloads already extracted to disk. Once a
capsule has legitimately opened, the data exists outside NYEDArch's control.
This must be stated plainly to customers; a revocation feature that implies
recall of delivered data would be a false claim.

## 6. Ordering relative to local protections

Server authorization occurs **after** runtime integrity and **before** local key
composition, but local protections are still evaluated independently. The server is not
told the passphrase, the fingerprint, or raw coordinates — it receives capsule
identity and categorized outcomes only (see the privacy document).

```
integrity -> binding -> server authorization -> machine -> passphrase
          -> location? -> time? -> compose K_payload -> AEAD open
```

## 7. Interaction with the measured timing oracle

Prototype-I has a measured ~50x timing separation between early and late protection
failures. Adding a network round trip **widens** this, since a server denial
returns before Argon2id runs. The licensed design should therefore adopt the
constant-time protection evaluation already identified as outstanding work in
Prototype-I, otherwise the network stage becomes a new and coarser oracle.
This is recorded as a dependency, not as solved.
