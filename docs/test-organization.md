# Test organization

When adding a test module, define it in a descriptive sibling `*_tests.rs`
file and include it with an explicit `#[path = "..._tests.rs"]` attribute.
Existing inline test modules are retained unless their implementation work
otherwise requires a change.
