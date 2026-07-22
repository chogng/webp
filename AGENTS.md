the upstream `../codex`

# Module organization

- Keep `lib.rs` focused on crate documentation, private module declarations, and explicit public re-exports.
- Split modules by concrete responsibility, data ownership, and stable invariants, not by another crate's layout or generic names such as `decoder` and `entropy`.
- Before extracting a module, identify the state, algorithms, callers, and invariants it owns. Moving code to another file is not architectural decoupling by itself.
- Keep module dependencies directional. If two proposed modules need each other's implementation details, revise the boundary or keep the implementation together.
- Prefer private modules and the narrowest practical visibility for cross-module items.
- Move implementation documentation and private tests with the module that owns the invariant. Keep cross-module behavior tests with the orchestration or public API layer.
- Review production modules over 500 lines, excluding tests, for separable responsibilities, with extra scrutiny near 800. Split only to improve cohesion, not merely to reduce line count.

# Toolchain

- Use the workspace's stable Rust toolchain for normal formatting, building, linting, and testing.
- Use nightly only for a command that explicitly requires it, such as `cargo fuzz` or Miri. Do not introduce a repository-wide nightly requirement for ordinary development.
- Keep formatting policy in the root `rustfmt.toml`; do not duplicate rustfmt-enforced layout rules in this file.

# Tests

## Test module organization

- When adding a new test module, define its contents in a separate sibling file rather than inline in the implementation file.
- Use an explicit `#[path = "..._tests.rs"]` attribute so the test filename is descriptive and easy to locate:

```rust
#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;
```

- This applies only when introducing a new test module. Do not move or rewrite existing inline `#[cfg(test)] mod tests { ... }` modules solely to follow this convention.
- Do not add tests for values that are statically defined.
