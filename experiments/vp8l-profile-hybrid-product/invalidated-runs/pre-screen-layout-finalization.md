# Superseded pre-screen Phase A binary

The first complete P20 Phase A passed at implementation/audit commit
`efa186cc` with product test binary
`3cef95229a632dbc775b036455ddd601343caf6e17c7e90b9f5943d6ea7a8ca2`.
Its external output is
`/private/tmp/vp8l-profile-hybrid-product-p20-phase-a-efa186cc`.

It proved 102/102 P18 byte identity for each profile, Compact
599,398,064 B with growth 0/0, LowLatency 601,400,998 B with growth 336/336,
714/714 spatial plans, 204/204 single plans, and all selector/fallback rows
exact. All rate and +2% tail gates passed.

This run is not used for final top-line claims because the test-only `generate`
command still emitted four rather than all six final-correctness layouts. The
production implementation did not change, but adding latest-main control
layouts changes the Rust test binary hash. Phase A must therefore restart on
the final screen/correctness-capable binary, and every later phase must use that
same new SHA. The passing result is superseded, not a codec failure, and no
sample is discarded or reclassified.
