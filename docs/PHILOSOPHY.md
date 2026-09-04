# NYEDArch — Philosophy and Design Intent

> **This document is context, not implementation.** It records *why* NYEDArch
> exists and what every part of it is ultimately for. Read it before making an
> architectural decision, because most of the hard choices in this project only
> make sense against it.

## 1. The premise

Conventional data protection is environmental. A file is safe because it sits
inside something safe: a disk with full-disk encryption, a network behind a
firewall, a server enforcing access control, a DRM reader honouring a policy.

Every one of those protections has the same failure mode. **The moment the data
leaves that environment, the protection is gone.** The file is copied to a USB
stick, forwarded to a personal address, exfiltrated by an intruder, or carried
out by an employee on their last day, and from that instant it is an ordinary
file that anyone can open. This failure recurs every year and costs enormous
sums, not because the protections were badly built, but because they were never
attached to the thing being protected.

NYEDArch starts from a different question:

> **What if the data protected itself, instead of relying on the environment it
> happens to be sitting in?**

That is the entire idea. Everything else is engineering in service of it.

## 2. What follows from that premise

If the data must protect itself, then it has to carry with it:

- the rules for opening it,
- the logic that enforces those rules,
- the cryptography that makes the rules more than a suggestion,
- and its own defences against the environment it lands in.

That is what a capsule is. Not "an executable that enforces a policy on data" —
that description is technically accurate and misses the point. A capsule is data
that has stopped being passive.

**Consequence for design:** anything that would make a capsule dependent on the
environment is a step backwards. This is why the payload key is composed from
factor contributions rather than gated by a branch, why the capsule carries no
reader dependency, and why a protection that cannot be satisfied fails closed
rather than degrading.

## 3. The chain reaction

The self-protection property is the first link, not the whole design. The
intended full system turns every failed attempt into a compounding cost for the
attacker:

```
unauthorized attempt
      │
      ├─→ payload stays cryptographically sealed        (attacker gains nothing)
      ├─→ attempt is recorded with enough specificity   (attacker leaves a trace)
      ├─→ owner is notified, in days not months         (the victim learns early)
      ├─→ that machine is blocked from further attempts (attacker loses the target)
      ├─→ repeated failure can destroy the capsule      (attacker loses the specimen)
      └─→ owner can request an audit trail and hand it
          to authorities under lawful process           (a clock starts on the attacker)
```

The point of the chain is asymmetry. Today, an attacker who steals a file has
unlimited time, unlimited attempts, and no exposure. Under this model they have
a sealed artifact, a limited number of attempts, a shrinking window before the
owner is notified, and a growing evidentiary trail. **The intent is to invert the
economics: make patience expensive for the attacker instead of free.**

## 4. Why the licence server is not optional later

Prototype-I is standalone by necessity, and that is a real limitation, not a
design preference. Several links in the chain above simply cannot exist inside a
capsule running alone on a hostile machine:

| Link | Why a capsule alone cannot do it |
|---|---|
| Trustworthy time | The local clock belongs to the attacker |
| Recording attempts | Anything the capsule writes locally, the attacker can delete |
| Notifying the owner | Requires a channel the attacker does not control |
| Blocking a machine | Requires state the attacker cannot roll back |
| Revoking a capsule | Requires an authority outside the artifact |
| Evidence with weight | Requires a custodian that is not the suspect's own machine |

This is why the licensed architecture makes connectivity mandatory and fails
closed without it. It is not a licensing convenience bolted on for revenue; it
is the only place several of these guarantees can live. A capsule that works
offline is a capsule that cannot be revoked, cannot report, and cannot warn
anyone.

**Design rule for Prototype-I:** build every seam so that the server can be
inserted later without redesigning the security model. Never build something
whose correctness depends on the server being absent.

## 5. The legal boundary — and why it constrains the design

The chain reaction only works if it is lawful. A protection mechanism that
breaks the law is not a security product; it is a liability that gets its owner
prosecuted alongside the attacker. Several parts of the intended design sit
close to real legal lines, and the honest response is to design inside them
rather than to hope nobody notices.

### 5.1 The capsule runs on a machine whose user may never have agreed to anything

This is the sharpest issue in the whole design, and it is easy to miss.

The **licensee** accepts the EULA. The person whose machine actually runs a
capsule may be someone else entirely — an intended recipient, a colleague, or an
attacker. Collecting machine identifiers from that person is processing their
personal data, and the licensee's consent does not cover them.

**Design requirement:** a capsule must state, at the point of execution and
before collecting anything, who is collecting, what is collected, and why. Not
buried in a document the runner never saw. This is a notice obligation under
essentially every modern data-protection regime, and it is also simply honest:
the person in front of the machine is entitled to know.

### 5.2 Identifiers must be specific enough to be useful, minimal enough to be lawful

The user's requirement is real: an audit trail saying "an attempt happened" is
useless to an investigator. It must be able to answer *which machine, when, from
where*.

But indiscriminate collection is both unlawful in most jurisdictions and
unnecessary. The resolution is **purpose-bound specificity**:

- collect identifiers that distinguish a machine, not identifiers that profile a
  person;
- store salted digests wherever comparison is all that is needed — a digest
  answers "is this the same machine as before?" without revealing what the
  machine is;
- retain raw detail only where it is genuinely required for an investigation,
  for a bounded period, and disclose that retention;
- record the *category* of a failure by default, and the specifics only when a
  risk threshold has been crossed.

Prototype-I already follows this: fingerprints are salted digests, raw hardware
identifiers are never stored or transmitted, and the location protection stores a
quantized region rather than coordinates.

### 5.3 Automated blocking is a decision about a person

"Flag an attempt and block that computer" is an automated decision with real
consequences. In several jurisdictions — GDPR Article 22 most explicitly —
decisions of that kind require a human review path, an explanation, and an
appeal. It is also just good engineering: a false positive locks a legitimate
user out of their own data, which is the worst failure this product can have.

**Design requirement:** automatic responses must be proportionate and reversible;
permanent consequences require human review. This is already the rule in the
security-monitoring design.

### 5.4 Destruction must never exceed our own artifacts

Self-destruction is defensible when a capsule destroys **itself** and its own
temporary material. It becomes unauthorized damage to a computer system —
a criminal offence under the Computer Misuse Act, the CFAA, and their equivalents
— the moment it touches anything else.

**Hard line:** a capsule may delete its own file and its own intermediate state.
It must never delete, corrupt, or disable anything belonging to the host. There
is no version of this product that ships a payload which harms the machine it
lands on, however satisfying that might sound against a thief.

### 5.5 Evidence, not vigilantism

The goal is to *preserve* evidence and hand it over under lawful process, not to
investigate or retaliate. Concretely:

- no counter-intrusion, no "hack back", no access to the attacker's machine
  beyond what the capsule needs to authorize itself;
- audit records are disclosed to the account owner about their own assets, and
  to authorities through proper legal channels — not published, not sold, not
  used to identify individuals for any other purpose;
- attribution is offered honestly. A machine identifier and an address indicate a
  device and a network path, **not a person**. Shared machines, VPNs, and stolen
  credentials all break that inference, and any report must say so rather than
  implying certainty an investigator would then rely on.

### 5.6 What a tamper-evident log does and does not prove

A hash-chained, signed audit trail proves that records were not altered after
they were written and anchored. It does not prove that a record is true, that a
suppressed event ever existed, or that a named person was at the keyboard.

Overstating this would be worse than useless: evidence presented as stronger
than it is damages the case it was meant to support. Every claim about the audit
system must state its limits alongside its guarantees.

## 6. The standard this sets for the implementation

Because failure of this system means data loss for a legitimate user and
impunity for an attacker, the engineering standard is higher than "it works":

- **Fail closed, always.** An ambiguous state must deny, never permit.
- **No silent degradation.** A weaker substitution must be visible to the user.
- **Cryptography is the boundary.** Anti-tamper raises cost; it never carries the
  guarantee.
- **Claims must match reality.** Every limitation is documented, including the
  ones that are inconvenient.
- **Legality is a requirement, not a filter applied afterwards.** If a capability
  cannot be delivered lawfully, it is not delivered.

## 7. One-sentence summary

> NYEDArch exists so that data can defend itself once it leaves every system
> built to protect it — and so that each attempt to break that defence costs the
> attacker time, evidence, and eventually the specimen itself, entirely within
> the law.
