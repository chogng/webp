# Source coding guidelines

## Module roots

- Treat directory module roots (`mod.rs`) primarily as module boundaries: declare child modules, define imports and visibility, and provide deliberate public re-exports.
- Keep implementation in the child module that owns its state and invariants. Retain implementation in `mod.rs` when the directory module itself owns cohesive state, orchestration, or invariants; do not split code merely to make `mod.rs` thin.

## Tests

### Test module organization

- When adding a new test module, define its contents in a separate sibling file rather than inline in the implementation file.
- Use an explicit `#[path = "..._tests.rs"]` attribute so the test filename is descriptive and easy to locate:

```rust
#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;
```

- This applies only when introducing a new test module. Do not move or rewrite existing inline `#[cfg(test)] mod tests { ... }` modules solely to follow this convention.
- Do not add tests for values that are statically defined.
