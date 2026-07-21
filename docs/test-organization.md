# Module and test conventions

## Source modules

- `src/lib.rs` is a facade: crate docs/attributes, module declarations, public
  re-exports, and the smallest necessary public decode entry point only.
- Keep implementations, domain types, tables, and internal helpers in their
  owning module; use a named `pub(crate)` module for shared internals.
- Split a growing module by responsibility. Move its related tests and module
  documentation with the implementation.

## Module-private tests

Put private tests for `src/frame.rs` in the sibling
`src/frame_tests.rs`, declared at the end of `frame.rs`:

```rust
#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
```

This is a `frame::tests` child module and may use `frame`'s private items.
Keep module-specific fixtures and builders there. Do not move existing inline
tests solely for naming; move them when extracting their implementation.

## Integration tests

Put public-API tests in `crates/<crate>/tests/*.rs`. They must not access
private items. Use them for decode behaviour, fixtures, corpus vectors, and
libwebp differential checks. They do not need to span multiple library crates:
the boundary is public API visibility.
