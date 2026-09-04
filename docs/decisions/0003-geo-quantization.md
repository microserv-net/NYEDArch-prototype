# ADR-0003: Location quantization strategy
Status: accepted (interim). Context: spec §20 permits a defensible spatial
quantization system. Decision: deterministic cos-lat-scaled grid + accuracy
rejection now; H3/S2 hierarchical cells for production, swappable behind
`geo::quantize`. Consequences: interim grid has boundary sensitivity (documented
limitation); key schedule unaffected by the future swap.
