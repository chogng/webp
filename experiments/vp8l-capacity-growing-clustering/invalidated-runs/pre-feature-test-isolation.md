# Retained pre-isolation feature test failure

The first full `cargo test -p webp --lib --features
vp8l-capacity-growing-experiment` completed 278 tests successfully, ignored four,
and failed one pre-existing coarse-path assertion:
`product_profiles_select_the_coarse_file_when_it_is_strictly_smaller`.

That test explicitly requires the public product profile to equal the E37
coarse writer. P16's feature-private purpose is to substitute the capacity
writer for those two profiles, and dedicated P16 tests already prove its exact
selector and public output. The coarse-only assertion was therefore gated with
`not(feature = "vp8l-capacity-growing-experiment")`; it remains active in the
default product build. The subsequent feature and default full-library results
are the applicable quality evidence.
