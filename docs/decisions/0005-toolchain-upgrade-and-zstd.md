# ADR-0005 — Toolchain upgrade and zstd compression

**Status:** accepted
**Category:** IMPLEMENTATION / PERFORMANCE
**Supersedes:** the workarounds in ADR-0002

## Context

Early development ran on Rust 1.75, which forced pinning several crates to
pre-`edition2024` versions and blocked modern compressors. ADR-0002 recorded
those pins as environment workarounds to be removed on a current toolchain.

## Decision

Upgraded to **Rust 1.91.1** and removed every pin:

| Was pinned | Now |
|---|---|
| `serde = "=1.0.210"` | `serde = "1"` |
| `zeroize = "=1.7.0"`, `zeroize_derive`, `cfg-if`, `base64ct` | unpinned / removed |
| `miniz_oxide = "=0.7.4"` | `miniz_oxide = "0.8"` |

Workspace `rust-version` raised to 1.82.

Added **zstd** as a second codec behind the existing versioned compression-id
seam, and made it the default for new packages. DEFLATE remains fully supported:
the codec is recorded in the package header, so capsules sealed earlier keep
opening. An unknown codec fails closed.

## A bug this surfaced

The first zstd implementation bounded decompression with a fixed 64 MiB cap,
which allocated 64 MiB for every 1 MiB chunk. Restore throughput collapsed to
~22 MiB/s — *slower than DEFLATE*, which is what made it visible.

Fixed by prefixing the exact uncompressed length, allocating precisely once, and
still rejecting an out-of-range length. The length sits inside the AEAD-sealed
chunk, so it is authenticated before use and cannot be forged to force a large
allocation.

## Measured outcome (release, same machine)

| Codec | Seal MiB/s | Restore MiB/s | Ratio |
|---|---|---|---|
| DEFLATE | ~151 | ~198 | 20.7x |
| zstd (before fix) | ~172 | ~22 | 23.7x |
| **zstd (after fix)** | **~172** | **~604** | **23.7x** |

zstd is now better on every axis. The package test suite also dropped from 9.15s
to 0.02s, which is the same bug seen from another angle.

## Consequences

- No functional or security semantics changed; the confidentiality boundary is
  untouched.
- Package format gained codec id 2. Version negotiation already existed, so no
  format-version bump was required.
- 53 tests pass on the new toolchain with zero warnings.
