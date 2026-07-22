# Invalidated development test filter

Before the Phase R binary or any corpus measurement, a local focused command
passed two positional test filters to `cargo test`:

`exact_spatial_cost::tests profile_hybrid::tests`

Stable Cargo accepts only one positional `TESTNAME` and exited with
`unexpected argument 'profile_hybrid::tests' found`. No compilation, test, or
experimental sample ran. The corrected command uses one common module-prefix
filter (and the locked runner later verifies its exact ignored-test filter).
