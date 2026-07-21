# Module and test organization

## Crate root is a facade

Each crate's `src/lib.rs` is a facade, not an implementation module. Its
permitted production responsibilities are limited to:

- crate attributes and crate-level documentation;
- module declarations (`mod foo;` / `pub mod foo;`);
- public type and function re-exports; and
- the small, public decoding entry point when it cannot live naturally in a
  domain module.

Do not add codec state machines, parsers, tables, domain types, helper
functions, or `#[cfg(test)] mod tests` directly to `lib.rs`. Put each of them
in the module that owns the behaviour, then re-export only the intended public
API from the root. This keeps dependency direction explicit and prevents the
crate root from becoming a catch-all implementation module.

For example, `bitstream.rs` owns boolean arithmetic decoding,
`quantization.rs` owns dequantization, and `frame.rs` owns frame storage and
pixel output; `lib.rs` only declares those modules and exposes their supported
API. Shared internal helpers belong in a named `pub(crate)` module rather than
in the crate root.

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

## Public-API integration tests

Put tests that use only the public API in the crate's `tests/` directory, for
example `crates/webp/tests/vp8_libwebp_oracle.rs`. Cargo compiles each such
file as a separate test crate that consumes the library crate, so it cannot
access private module items. This is intentional: these tests protect the
externally visible decoder contract.

Use integration tests for container-to-image behaviour, error reporting,
committed fixtures, corpus vectors, and libwebp differential checks. Do not
make an internal function public only to let an integration test call it;
write a module-private test instead.

`tests/` does **not** require the behaviour itself to span two library crates.
The distinction is visibility: a test belongs there when it exercises only
the public API, even if it calls one crate's decoder. Tests that coordinate
multiple workspace crates also belong there, normally at the highest-level
facade crate that owns the user-visible contract.

## Choosing the location

| Behaviour under test | Location |
| --- | --- |
| Private parser state, prediction edges, transforms, or filter decisions | `src/<module>_tests.rs` |
| A public decode/read-info API result (including one-crate API tests) | `crates/<crate>/tests/*.rs` |
| A regression fixture through the public decoder | `crates/webp/tests/*.rs` plus `tests/fixtures/` |
| Cross-check against `cwebp` or `dwebp` | `crates/webp/tests/*_oracle.rs` |

Run focused module tests with `cargo test -p webp-vp8`, public API tests with
`cargo test -p webp`, and the full suite with `cargo test --workspace`.
