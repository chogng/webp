# Final Rust architecture migration ledger

This ledger records the review boundary and reproducible gates for the migration
defined by [`final-rust-architecture.md`](final-rust-architecture.md). The
migration moves existing code and ownership only; it does not change codec
algorithms, public API semantics, corpus contents, or the post-migration product
work listed in section 9 of the contract.

## Stage 0 baseline

- Date: 2026-07-22 (America/Los_Angeles)
- Worktree: `/Users/lance/.codex/worktrees/6fc7/webp`
- Branch: `codex/final-rust-architecture`
- Base and starting HEAD: `a337b7d5a7278b6b5680e63d7b594c084f89b444`
- Starting local `main`: `a337b7d5a7278b6b5680e63d7b594c084f89b444`
- Starting `origin/main`: `5e54dd37c14cc0c810d5a2283b644161ddb2a9b2`
- Inherited working-tree change: `AGENTS.md` was already modified. It is not
  part of the migration and must remain unstaged and uncommitted.
- Inherited contract: `docs/final-rust-architecture.md` was untracked. Its
  contents were reviewed against the delegated task and are committed as the
  migration contract.

The initial Cargo graph contained fourteen production library packages plus
`xtask`. The public `webp` crate depended on `webp-alpha`, `webp-animation`,
`webp-container`, `webp-core`, `webp-vp8`, `webp-vp8l`, and
`webp-vp8l-literal`. `webp-container` depended on `webp-core`.

### Build, test, and documentation baseline

All commands ran from the stable workspace toolchain unless noted.

| Gate | Command | Result |
| --- | --- | --- |
| Formatting | `cargo fmt --all -- --check` | pass |
| Debug tests | `cargo test --workspace` | pass |
| Release tests | `cargo test --release --workspace` | pass |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| Documentation | `cargo doc --workspace --no-deps` | pass; inherited unresolved `IntraMacroblock` rustdoc-link warning |
| Bazel | `bazel test --test_output=errors --test_tag_filters=-external-corpus //...` | pass; 15/15 tests |
| Fuzz build | `cargo +nightly fuzz build` | pass; all 12 targets |
| Fuzz smoke | each target with `-runs=100` | pass; all 12 targets |

The benchmark scripts expect `target/` at the repository root while this
workspace places Cargo output in `webp-rs/target/`. A temporary local symlink
was used to execute the scripts unchanged. Pinned corpus and oracle data were
read through a temporary local link to the main checkout's ignored
`third_party/` cache; the main checkout was not modified.

### Performance baseline

| Public path | Command | Rust result | Oracle/result guard |
| --- | --- | --- | --- |
| VP8L conformance decode | `bash tools/benchmark-vp8l.sh 5` | 464.580 ms; checksum `96355` | libwebp 519.390 ms; checksum `96355` |
| VP8L CLIC decode | `bash tools/benchmark-vp8l-clic.sh 1 4` | 14,076.777 ms; checksum `997056` | libwebp 14,574.271 ms; checksum `997056` |
| VP8L static encode | `bash tools/benchmark-vp8l-encode.sh 5` | 1,219.815 ms; 91,508,840 bytes; checksum `91525650` | libwebp 9,108.858 ms; 70,883,120 bytes |
| VP8 static encode | `bash tools/benchmark-vp8-encode.sh 5` | 339.337 ms; 919,010 bytes; checksum `944840` | PSNR 25.857/37.376/48.650 dB; libwebp 676,130 bytes |
| VP8/ALPH static encode | `bash tools/benchmark-alpha-encode.sh 10` | 7,152.434 ms; 66,189,100 whole bytes; 41,186,220 ALPH bytes | libwebp 10,198.234 ms; 65,099,020 whole bytes; 40,983,250 ALPH bytes |
| VP8L-frame animation encode | `bash tools/benchmark-animation-encode.sh 5` | 30.462 ms; 1,937,440 bytes across five matrices; checksum `1937850` | 387,488 bytes per six-frame animation |

The VP8L CLIC method splits were 4,378.967 ms (method 0), 4,850.206 ms
(method 3), and 4,783.370 ms (method 6). All baseline performance results are
inside the reviewed thresholds in `quality-gates.md`.
