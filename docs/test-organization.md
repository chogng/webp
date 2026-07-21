# Test organization

Tests are grouped by the visibility of the behaviour they verify. Keep the
test close to the code when it needs access to module-private implementation
details; put it at the crate boundary when it verifies a user-visible
contract.

## Module-private tests

For a source module such as `src/frame.rs`, place its private implementation
tests in the sibling file `src/frame_tests.rs` and declare it at the end of
the source module:

```rust
#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
```

This makes the test file the `frame::tests` child module. It can exercise
private functions, types, and invariants in `frame.rs` without making them
public just for testing. Use the matching `*_tests.rs` name for each module:
`entropy_tests.rs`, `partition_tests.rs`, and so on.

Keep test-only builders, bit writers, and fixture constructors with the
module they serve. Move a helper to shared test support only after more than
one module genuinely needs it.

## Integration tests

Put tests that use only the public API in the crate's `tests/` directory, for
example `crates/webp/tests/vp8_libwebp_oracle.rs`. Cargo compiles each such
file as a separate consumer of the crate, so it cannot access private module
items. This is intentional: these tests protect the externally visible
decoder contract.

Use integration tests for container-to-image behaviour, error reporting,
committed fixtures, corpus vectors, and libwebp differential checks. Do not
make an internal function public only to let an integration test call it;
write a module-private test instead.

## Choosing the location

| Behaviour under test | Location |
| --- | --- |
| Private parser state, prediction edges, transforms, or filter decisions | `src/<module>_tests.rs` |
| A public decode/read-info API result | `crates/<crate>/tests/*.rs` |
| A regression fixture through the public decoder | `crates/webp/tests/*.rs` plus `tests/fixtures/` |
| Cross-check against `cwebp` or `dwebp` | `crates/webp/tests/*_oracle.rs` |

Run focused module tests with `cargo test -p webp-vp8`, public API tests with
`cargo test -p webp`, and the full suite with `cargo test --workspace`.
