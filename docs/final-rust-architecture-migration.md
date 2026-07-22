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

## Staged implementation

| Stage | Commit | Result |
| --- | --- | --- |
| Contract | `25511118` | Reviewed and committed the final architecture contract inherited with the task. |
| Stage 1 container ownership | `815758f8` | Moved existing RIFF, metadata, VP8/VP8L, and animation serialization into `webp-container`. |
| Stage 1 VP8L writer ownership | `cbfe0522` | Moved the complete existing lossless image writer behind the VP8L owner. |
| Stage 2 VP8L reader ownership | `314a5081` | Consolidated the lossless reader and former VP8L micro-crates without changing the public decode path. |
| Stages 3–5 | `7ebdd5c3` | Consolidated private codec owners into `webp`, made `webp-container` independent, removed obsolete crates, and reduced fuzz/build dependencies to the two production libraries. |

The consolidation shares the VP8L canonical-symbol writer, length/distance
prefix encoder, pixel representation, and color-cache hash with the ALPH
headerless-VP8L path. `webp-container` treats codec payloads as opaque bytes;
its provisional serializer remains deliberately narrower than a general muxer
or editor.

## Final acceptance

Final validation ran after the owner consolidation and final module-path audit.
The ordinary toolchain remained stable Rust; nightly was used only for
`cargo fuzz`.

| Gate | Result |
| --- | --- |
| Cargo metadata and dependency direction | pass; packages are `webp-container`, `webp`, and tool `xtask`; `webp` has one repository path dependency (`webp-container`), and `webp-container` has none |
| Formatting | pass: `cargo fmt --all -- --check` |
| Debug workspace tests | pass: 256 `webp` unit tests, 12 container tests, all workspace integration/oracle tests, and doctests |
| Release workspace tests | pass with the same test and oracle matrix |
| Clippy | pass: `cargo clippy --workspace --all-targets -- -D warnings` |
| Documentation | pass: `cargo doc --workspace --no-deps`; normal docs do not compile or expose the `fuzzing` module |
| Bazel | pass: three final library/tool test targets, with external corpus targets excluded as documented |
| Fuzz build | pass: all 12 targets |
| Fuzz smoke | pass: every target completed at least 100 runs with no crash |

### Final performance matrix

| Public path | Final Rust result | Guard |
| --- | --- | --- |
| VP8L conformance decode | 455.154 ms; checksum `96355` | libwebp 530.411 ms; checksum `96355` |
| VP8L CLIC decode | 13,681.954 ms; checksum `997056` | libwebp 14,355.591 ms; checksum `997056`; methods 0/3/6 were 4,255.826 / 4,781.724 / 4,637.672 ms |
| VP8L static encode | 1,260.988 ms; 91,508,840 bytes; checksum `91525650` | libwebp 9,712.771 ms; 70,883,120 bytes; within baseline and reviewed size/time limits |
| VP8 static encode | 336.263 ms; 919,010 bytes; checksum `944840` | PSNR 25.857/37.376/48.650 dB; libwebp 676,130 bytes |
| VP8/ALPH static encode | 7,026.601 ms; 66,189,100 whole bytes; 41,186,220 ALPH bytes | libwebp 10,142.944 ms; 65,099,020 whole bytes; 40,983,250 ALPH bytes; exact alpha oracle passed |
| VP8L-frame animation encode | 30.055 ms; 1,937,440 bytes; checksum `1937850` | 387,488 bytes per animation; pinned `webpmux`/`dwebp` oracle passed |

The first isolated VP8L and VP8 encode measurements landed just outside a
relative or absolute boundary while the host was still compiling other gates.
Immediate isolated reruns passed, and the post-audit results above passed again
with identical output bytes, checksums, and quality metrics. No threshold was
changed.

All section 8 architecture checks passed at the migration freeze. The section 9
full demux API, general mux/editor, decoder-only feature product, and SharpYUV
work were explicitly left to separate follow-up tasks; all four were completed
afterward and are tracked by `final-rust-architecture.md`.
