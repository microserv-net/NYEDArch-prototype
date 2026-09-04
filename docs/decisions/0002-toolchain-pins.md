# ADR-0002: Rust 1.75 edition2024 dependency pins
Status: accepted (environment-driven). Context: sandbox toolchain is Rust 1.75;
`base64ct`, `zeroize`, `zeroize_derive`, `cfg-if` published releases requiring
Cargo `edition2024`. Decision: pin those below the edition2024 boundary. On the
intended latest-stable CI toolchain (§40) the pins are removed. Consequences:
slightly older utility crates in the sandbox only; core primitives unaffected.

---

**Superseded by ADR-0005.** The toolchain was upgraded to Rust 1.91.1 and every
pin recorded here has been removed. This ADR is retained as the historical
record of why the pins existed.
