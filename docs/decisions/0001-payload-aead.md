# ADR-0001: XChaCha20-Poly1305 as the payload AEAD
Status: accepted. Context: spec left AEAD open (§54). Decision: XChaCha20-Poly1305
for 192-bit random-nonce safety and no hardware-AES dependency in hostile
binaries. Consequences: 24-byte nonces stored per sealed blob; migration handled
by `AEAD_ID_*` version byte. Alternatives: AES-256-GCM (nonce-reuse fragility,
AES-NI dependence). Security: equal-or-stronger; no external behavior change.
