# Legacy fixture metadata

These TOML sidecars are retained as historical provenance for existing
fixtures. Rust tests now consume fixture files directly and keep their expected
behavior beside the public API assertion. New fixtures do not require a
manifest; add the smallest reproducer under `tests/fixtures/regressions/` and
test it from `crates/webp`.
