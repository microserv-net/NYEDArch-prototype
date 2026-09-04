# NYEDArch Licences, Tiers, and Entitlements

> DESIGN DOCUMENTATION ONLY. Nothing here is implemented.

## 1. Tier model

No free tier. Terms: **3, 6, or 12 months**, renewable. There is no perpetual
licence.

| | **Seal** (base) | **Seal Pro** | **Vault** (team) | **Vault Enterprise** |
|---|---|---|---|---|
| Intended user | Individual protecting own data | Professional / consultant | Team or department | Regulated organisation |
| Machine protection | Yes | Yes | Yes | Yes |
| Passphrase protection | Yes | Yes | Yes | Yes |
| **Time protection** | — | Yes | Yes | Yes |
| **Location protection** | — | — | Yes | Yes |
| Full-strength cryptography | **Yes** | Yes | Yes | Yes |
| Trusted fingerprints per capsule | Small (e.g. 3) | Moderate | Large | Configurable |
| Builds per term | Low | Moderate | High | Negotiated |
| Audit retention | Configurable, short bound | Configurable, longer | Configurable, long | Configurable, longest |
| Org management / delegated admin | — | — | Yes | Yes |
| Failsafe approvers | Owner | Owner | Admin + optional reviewers | Admin + required reviewers |
| Enterprise identity integration | — | — | — | Yes |
| Support | Standard | Standard | Priority | Dedicated |

## 2. Why gating time and location is defensible

The stated principle is that **core security must not be an upsell**. That
principle is honoured because:

- every tier receives the **same cryptographic strength** — same AEAD, same
  Argon2id parameters, same key composition, same integrity and binding;
- the two mandatory protections, which are the actual confidentiality boundary, are
  present at every tier;
- location and time are **low-entropy policy factors**, documented as such
  throughout Prototype-I. They constrain *where* and *when*, they add no
  meaningful entropy, and they do not strengthen the cryptography.

A Seal customer is not receiving weaker encryption. They are receiving fewer
policy controls. That distinction is honest and should be stated in exactly those
terms in marketing material — not blurred into "more security at higher tiers".

**If that framing is ever contradicted** — for example by weakening KDF
parameters at lower tiers to save server cost — the principle is broken. It must
not be.

## 3. Licence keys

| Property | Design |
|---|---|
| Generation | CSPRNG, ≥128 bits of entropy |
| Format | Grouped, checksummed, unambiguous alphabet (no O/0, I/1) |
| Display | **Once**, at issuance. Never retrievable in plaintext again |
| Server storage | Verifier only (salted hash). The server does not store plaintext keys |
| Reissue | Permitted — invalidates the previous key and is an audited security event |

### The key is a bootstrap credential, not the authorization mechanism

A licence key alone must not be sufficient for anything meaningful. It functions
as an **activation and import credential**, always in combination with an
authenticated account and MFA.

The system must remain secure when `licence key is known`, because keys leak:
they get pasted into tickets, shared between colleagues, and committed to
repositories. Ownership is established by the **account**, not by possession of a
string. Possession of a key without account authentication grants nothing.

## 4. Entitlement lifecycle

```
purchase -> entitlement created (pending MFA)
         -> MFA enrolment complete -> entitlement active
         -> renewal -> active (term extended)
         -> non-renewal -> expired  -> capsules stop opening
         -> abuse -> suspended (temporary, exponential escalation)
         -> severe abuse/fraud -> revoked (permanent)
```

| State | Builder | Existing capsules |
|---|---|---|
| Active | Full function | Authorize normally |
| Pending MFA | Blocked | N/A |
| Expired | Blocked | **Stop opening** (settled decision) |
| Suspended | Blocked | Blocked for the suspension period |
| Revoked | Blocked permanently | Blocked permanently |

## 5. Expiry is a data-availability event

This deserves its own statement because it is the sharpest edge in the product.

Under the settled ruling, **expiry denies access to capsules that already
exist**. This is commercially defensible but it converts a billing lapse into
loss of access to data the customer owns.

Required mitigations, treated as product requirements:

1. Prominent disclosure in the EULA **and** at checkout — not buried in terms.
2. A pre-expiry notification sequence at decreasing intervals (e.g. 30/14/7/1
   days), to every registered channel.
3. An explicit "extract before expiry" advisory for customers holding long-lived
   archives.
4. A clearly communicated post-expiry reinstatement path: renewing restores
   authorization for existing capsules.

Without (4), expiry is indistinguishable from destruction, and customers would
be right to treat the product as hostile.

## 6. Revocation

Revocation is **permanent**. A revoked licence is never reinstated; the customer
purchases a new one. Where revocation was not the customer's fault — for example
revocation following a compromise they reported — remaining term may be
transferred to the replacement licence.

Transfer decisions are recorded with the reason, so that "who decided this and
why" is answerable months later.
