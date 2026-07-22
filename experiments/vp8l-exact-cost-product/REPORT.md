# VP8L exact-cost single-write product validation

Date: 2026-07-21 (America/Los_Angeles).

## Decision

**Pass every promotion gate and retain the product branch without merging it.**
The existing public opt-in `FastDecodeCompact` and `FastDecodeLowLatency`
profiles remain byte-for-byte identical to current local `main` and to the P09
candidate. Exact same-profile costing removes the losing single-main write on
the common candidate-win path.

| profile | current-main control | product | wall change | paired change | gate |
| --- | ---: | ---: | ---: | ---: | --- |
| Compact | 14.243412 s | 10.199847 s | -28.389% | -28.433% | pass |
| LowLatency | 13.944728 s | 9.905461 s | -28.966% | -29.054% | pass |

Both profiles exceed the required 25% improvement and finish below the 11.0 s
absolute limit. All five formal rounds and every 3×MAD outlier are retained.

## Provenance and branch isolation

- Source task/thread: `019f8321-035e-7211-8f53-987e18891c8c`.
- Task: `vp8l-exact-cost-product`.
- Required latest local-main base and initial full SHA:
  `130aa1f347ae1193463f35205b5bd98b4031bc7c`.
- Branch: `codex/vp8l-exact-cost-product`.
- Worktree: `/Users/lance/.codex/worktrees/6368/webp`.
- Product code commit:
  `6ed10e559e82873d89606943e183d5432634b1a1`.
- Before branch creation, detached `HEAD` and the merge-base with the required
  base both resolved exactly to
  `130aa1f347ae1193463f35205b5bd98b4031bc7c`. The branch was attached directly
  there.
- No fetch, `origin/main`, P09 branch, old worktree, or uncommitted main-tree
  file supplied ancestry or product code. The P09 candidate is not an ancestor
  of the product branch. Its common merge-base is the older source base
  `5362912a23a39175758796e07f45af3ee79143b1`.
- `AGENTS.md` was not modified.

Read-only P09 sources:

- worktree `/Users/lance/.codex/worktrees/b99f/webp`;
- branch `codex/vp8l-exact-cost-single-write`;
- candidate `a89e0f73d6f54f87df6a25d866955591c208dc92`;
- evidence `c0b6544e336ffc90ab77874da2f4e9788aa9424c`;
- final/hygiene `a8570f47cbf3f18420b13e9b720d454315455e6b`;
- report `experiments/vp8l-exact-cost-single-write/REPORT.md`.

## Source mapping and product convergence

The P09 implementation was understood and remapped onto latest local `main`;
the research commit was not cherry-picked.

- `encoder.rs`: retained only the private `single_plan` module declaration,
  documentation update, and behavior-preserving split between adaptive-table
  preparation and table writing.
- `single_plan.rs`: retained canonical table ownership and exact complete-RIFF
  accounting. The product version removes the production `payload_bytes` field
  that was read only by tests; tests derive it from validated meaningful bits.
- `spatial_writer.rs`: retained plan-before-candidate selection, one candidate
  serialization, strict complete-RIFF comparison, cached single fallback, and
  the old complete double-stream fallback for plan failure. A fieldless
  `SelectionKind` supports narrow test observation without a broad
  `allow(dead_code)`.
- Product tests retain only current-main control layouts and exact audit hooks.
  P09 phase timing, candidate-only modes, allocation attribution, research
  layout enums, and large phase benchmark paths were deliberately omitted.
- The focused plan tests remain in sibling `single_plan_tests.rs` through an
  explicit `#[path]`. They add an explicit forced plan-failure check against
  the complete current-main control path.
- The existing locked benchmark runner was sufficient and was not changed.

The production files are 179 lines for `single_plan.rs` and 331 lines for
`spatial_writer.rs`, with the latter's final 89 lines compiled only for tests.
Dependencies remain directional: `spatial_writer` consumes the plan, while the
plan depends only on encoder-owned entropy preparation/writing primitives.

## Exact-cost invariant

Private `SinglePlan` owns exactly the canonical tables and exact complete-RIFF
cost of the same-profile, no-cache, single-group stream. It computes from the
already collected frequency arrays and never scans the token vector again:

- the 44-bit VP8L header and subtract-green transform prefix;
- the no-cache and no-meta flags;
- every normal-table header: 63 fixed bits plus four bits per alphabet entry;
- `frequency × canonical width` for all five Huffman alphabets;
- length extra bits accumulated from the 24 green length-prefix frequencies;
- the fixed distance value 121's extra bits multiplied by copy count;
- equality between green copy-prefix count and total distance count;
- byte rounding, VP8L chunk storage, odd payload padding, the 32-bit RIFF size
  bound, and complete file bytes.

All additions, products, conversions, chunk sizes, and RIFF sizes are checked.
Any preparation, arithmetic, or representation error is intercepted before a
candidate is written and invokes the unchanged complete two-stream control
logic. Candidate serialization errors propagate normally. If a cached-single
write later fails allocation, the allocation error propagates instead of
writing the candidate again.

The spatial candidate is fully serialized once. It wins only when its complete
padded RIFF size is strictly smaller than the exact single size. A single win
or byte tie writes the cached canonical plan and matches current `main`
byte-for-byte. On the normal candidate-win path, only the selected complete
payload exists; the retained lengths and canonical tables are small planning
state, not a hidden second payload.

## Exact estimator and selection audit

The final release test binary SHA-256 is
`596df0feaedc5033c18f5220540281774ddf9e3622a3b9c80e4f9b75be88c903`.
Its 102-image audit reports:

- 102/102 unique single streams with zero mismatch in meaningful bits,
  rounded payload bytes, and complete padded RIFF bytes;
- 204/204 profile rows with `estimate_exact=1`, `control_exact=1`,
  `candidate_won=1`, `losing_single_main_written=0`, and
  `estimator_fallback=0`;
- no second complete single payload and no repeated candidate write on those
  normal corpus paths.

Focused tests cover tiny single fallback, a generated byte tie decision,
forced planning fallback, transparent input, 127/128/129 and 255/256/257
boundaries, copy lengths and their extra bits from 3 through 4096, both RIFF
padding parities, and a legal 16,384-pixel single dimension.

## Byte identity and decoder correctness

Independent binaries were built from `git archive` snapshots of latest local
`main`, product commit `6ed10e55...`, and P09 candidate `a89e0f73...`. Each
image was generated and deleted independently so verification never retained
a second full corpus payload.

- Latest-main control versus product: 306/306 Default/Compact/LowLatency rows
  match in length, SHA-256, and full bytes.
- Product versus P09 candidate: 306/306 rows match in length, SHA-256, and full
  bytes.
- The project decoder reproduced full source RGBA for 306/306 product streams.
- Pinned libwebp `733c91e461c18cf1127c9ed0a80dccbcfed599d3`
  `WebPDecodeRGBA` reproduced full RGBA for 306/306 streams, `failed=0`.
- Metadata options have a focused exact-versus-control byte check including
  ICCP, EXIF, XMP, alpha, VP8X flags, and odd chunk handling.
- Default stream identity is established by the 102 Default rows. Complete
  workspace tests cover unchanged animation output, geometry/timing errors,
  metadata, malformed input, allocation limits, and public APIs.

Formal output identity across every round:

| profile | input bytes | output bytes | full-output checksum |
| --- | ---: | ---: | --- |
| Compact | 1,007,432,548 | 617,958,802 | `675ac66537d0d79f` |
| LowLatency | 1,007,432,548 | 625,321,072 | `2acaf1a5ad92d53c` |

Control and product values match for all five rounds.

## Locked performance gates

The final binary held `/private/tmp/webp-vp8l-product-benchmark.lock`
atomically. Inputs were preloaded, layouts alternated forward on odd rounds and
reverse on even rounds, every output byte was checksummed, and `wait4` recorded
process wall, CPU, and peak RSS.

The 41-image, three-round screen passed before formal execution:

| profile | control median/MAD | product median/MAD | independent | paired |
| --- | ---: | ---: | ---: | ---: |
| Compact | 5.946381 / 0.004721 s | 4.273646 / 0.008254 s | -28.130% | -28.130% |
| LowLatency | 6.045863 / 0.012743 s | 4.308236 / 0.000474 s | -28.741% | -28.733% |

Formal 102-image, five-round results retain every sample:

| profile/layout | encoder median | MAD | 3×MAD rounds | process wall | CPU | peak RSS |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| Compact control | 14.243412 s | 0.021865 s | none | 19.160958 s | 19.151521 s | 1292.97 MiB |
| Compact product | 10.199847 s | 0.006721 s | r3 | 15.096795 s | 15.087664 s | 1216.22 MiB |
| LowLatency control | 13.944728 s | 0.022783 s | r5 | 18.850454 s | 18.840279 s | 1289.41 MiB |
| LowLatency product | 9.905461 s | 0.025038 s | none | 14.808693 s | 14.798409 s | 1216.41 MiB |

The paired-ratio medians are -28.433% for Compact (MAD 0.079 percentage
points) and -29.054% for LowLatency (MAD 0.232 points), with no paired-ratio
3×MAD outliers. Independent per-image p0/p10/p50/p90/p100 changes are
-32.850/-31.264/-29.819/-28.560/-27.765% for Compact and
-33.686/-32.006/-30.496/-29.185/-28.120% for LowLatency. No image regressed.

The release `webp` rlib changed from 410,296 to 436,152 bytes: +25,856 bytes
(+6.302%). The release test binary changed from 1,460,816 to 1,481,328 bytes:
+20,512 bytes (+1.404%).

## Validation and environment

The repository-pinned stable Rust 1.88.0 toolchain passed:

- `cargo test --workspace --all-targets`;
- `cargo test --release --workspace --all-targets`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo fmt --all -- --check`;
- `RUSTDOCFLAGS='-D warnings' cargo doc -p webp --no-deps`;
- `cargo test -p webp --doc`;
- Python syntax checks for all evidence tools, shell syntax for the reproducer,
  evidence summarization, SHA verification, and repository diff checks.

The pinned 1.88.0 toolchain has only `aarch64-apple-darwin` installed. No
target, component, toolchain, dependency, or global configuration was added or
changed. wasm/windows builds were therefore not rerun; existing E33 evidence
already covers those targets. No nightly command was used.

No decoder, geometry, clustering, tokenization, wire syntax, public API,
Default behavior, metadata format, animation behavior, dependency, unsafe
code, security model, thread model, or toolchain policy changed.

## Evidence and reproduction

Primary evidence:

- `exact-audit-102.tsv` — full estimator and selection audit;
- `identity-306.tsv` — latest-main/product/P09 length, SHA-256, and byte identity;
- `oracle-306.tsv` — pinned libwebp full-RGBA results;
- `screen-compact/`, `screen-low-latency/`, and `formal-102/` — complete raw
  measurements, process records, and run metadata;
- `gate-summary.json` and `gate-summary.md` — robust independent and paired
  statistics without outlier removal;
- `binary-delta.tsv` — release artifact sizes;
- `SHA256SUMS` — hashes for every committed evidence file.

Reproduce all product evidence with:

```sh
experiments/vp8l-exact-cost-product/reproduce.sh \
  /Users/lance/Desktop/webp/third_party/benchdata/clic/vp8l-lossless-exact \
  /Users/lance/Desktop/webp/third_party/oracle/libwebp \
  /private/tmp/vp8l-exact-cost-product-reproduction
```

The reproducer builds isolated archives for the exact base, product commit,
and P09 candidate; audits exact costs; enforces both screen gates; runs the
formal locked benchmark; performs three-way streaming identity and pinned
libwebp checks; records binary deltas; summarizes robust statistics; and hashes
the complete evidence set.
