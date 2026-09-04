# NYEDArch Payments and Purchases

> DESIGN DOCUMENTATION ONLY. Nothing here is implemented.

## 1. Payment boundary

NYEDArch does **not** process raw card data. A tokenized, provider-hosted
checkout keeps card details entirely outside NYEDArch systems, which removes the
largest compliance and breach surface in the product for no meaningful loss of
capability.

No specific provider is selected here. Selection criteria: hosted checkout with
tokenization, verifiable signed webhooks, strong authentication support, refund
and chargeback APIs, and coverage in target markets.

## 2. Flow

```
account (may exist before purchase)
   -> choose tier and term (3 / 6 / 12 months)
   -> provider-hosted checkout
   -> payment confirmation (signed webhook)
   -> entitlement created  [state: pending MFA]
   -> mandatory MFA enrolment
   -> entitlement active -> licence key displayed ONCE
   -> software download
```

Accounts may be created before purchase. This is the stronger model: it separates
identity proofing from payment, allows MFA enrolment before any entitlement
exists, and means a failed payment does not leave an orphaned half-account.

**No account receives product functionality without an entitlement.**

## 3. Webhook handling

| Concern | Design |
|---|---|
| Authenticity | Verify the provider's signature; unsigned or invalid events are discarded and alerted |
| Idempotency | Every event carries an id; replays are no-ops |
| Ordering | Events may arrive out of order; state transitions are commutative where possible and otherwise reconciled against the provider |
| Delivery failure | Reconciliation job compares provider state to local entitlement state |

Entitlement is never granted from a client-side success redirect. Only a verified
webhook, or reconciliation against the provider, creates entitlement.

## 4. Refunds, chargebacks, and expiry

| Event | Effect |
|---|---|
| Refund | Entitlement ends. **Capsules stop opening** |
| Chargeback | Entitlement suspended pending resolution |
| Chargeback resolved for customer | Entitlement restored |
| Non-renewal | Entitlement expires; capsules stop opening |
| Renewal after expiry | Authorization for existing capsules is **restored** |

The final row is essential. Without reinstatement, expiry would be
indistinguishable from destruction of the customer's data, and the product would
deserve to be distrusted.

## 5. Disclosure obligations

Because expiry denies access to existing capsules, the following are product
requirements, not marketing choices:

1. Prominent statement at checkout — not only in the EULA.
2. Pre-expiry notifications at 30, 14, 7, and 1 days to every registered channel.
3. An "extract before expiry" advisory for long-lived archives.
4. Clear statement that renewal restores access.

## 6. Fraud considerations

Signals: mismatched geography, disposable email domains, rapid repeat purchases,
card testing patterns. Response follows the graduated model in the monitoring
document. Purchase fraud does not automatically revoke a licence — revocation is
permanent and therefore requires human review.
