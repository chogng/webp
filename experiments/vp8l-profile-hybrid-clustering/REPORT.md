# P18: profile-specialized exact-cost hybrid

Status: **promote the research result to an independent product-migration
study.** Every P18 performance, exactness, correctness, resource, stable quality,
reproduction, and checksum gate passed. Do not merge this research branch into
main.

See `DESIGN.md` for the only permitted architecture, ties, stops, lifetime,
gates, and early-stop policy. This report will retain passing or negative
evidence without changing those rules after observing corpus results.

## Provenance and final Phase A binary

The creation base is `58f7b8dd047cad1733bc2766a797d8f2e4b5ff3c` and
the branch is `codex/vp8l-profile-hybrid-clustering`. The docs-only
post-creation registration `7f5cd83c` was not merged or rebased. Design was
frozen in `3dea69cc`, implementation in `a0606a83`, and the complete
attribution schema in `36ad7acd`.

The current final release test binary is
`05b8421c86f3286667c5cffef35ffc2bff77f68d944063a954793e2b870e64c9`
(2,230,912 B). Its exact ignored-test filter lists one test. The prior binary
and Phase A are retained under `invalidated-runs` because they predated the
prepare/writer timers; no result from that binary is counted below.

## Phase A: locked 102 images

The regenerated corpus manifest is 102 rows and hashes to
`9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`;
the first 41 rows hash to
`474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`.
Current-main same-binary controls exactly reproduce E37.

| rate metric | Compact | LowLatency |
| --- | ---: | ---: |
| E37/current control | 617,958,802 B | 625,321,072 B |
| P18 final | **599,398,064 B** | **601,400,998 B** |
| vs E37/current | **-3.003556%** | **-3.825247%** |
| vs E40 | 0.000000% | -2.535708% |
| vs E41 | +9.489482% | 0.000000% |
| worst image vs control | +1.490531% (`002`) | +1.483745% (`100`) |
| images over +2% | **0 / 102** | **0 / 102** |
| E / B / R / Split / Single wins | 0 / 1 / 101 / 0 / 0 | 1 / 2 / 20 / 79 / 0 |
| growth attempts / accepted | **0 / 0** | **336 / 336** |

All expected mechanism values reproduced exactly. Compact's self-cost time,
growth timings, split storage, split stream, and growth attempts are all zero.
Every accepted LowLatency step strictly reduces complete RIFF bytes.

Exactness passed for all 306 Compact E/B/R and 408 LowLatency E/B/R/Split
planner/writer rows (714 total), all 204 exact single plans, all 204 E/B
selectors, all 204 final selectors, and all 204 public selected streams.
`SpatialCostPlan::write` rejects any payload-bit, payload-byte, or complete-RIFF
mismatch, so every retained row proves all three sizes rather than only file
length. Phase A stderr is empty.

The complete fixed group evolution for `008`, `066`, and `074` is in
`TAILS.md`. LowLatency grows them 11→16, 7→16, and 5→16 groups and finishes at
4,302,942 B (-2.019478%), 3,355,168 B (-3.129497%), and 4,535,584 B
(-10.793940%) against control. Compact performs no growth; its corresponding
final values are 4,353,944 B (+0.610557%), 3,431,440 B (-0.189473%), and
5,338,236 B (+0.287209%).

## Phase A attribution and resources

One 102-image audit pass produced the following additive stage totals. Writer
is the selected candidate's isolated serialization; diagnostics serialize
other candidates only for exactness and do not enter this field.

| stage | Compact | LowLatency |
| --- | ---: | ---: |
| shared prepare | 1.644666 s | 1.619197 s |
| counter init / update | 0.000611 / 0.561321 s | 0.000365 / 0.556152 s |
| self-cost cache | **0.000000 s** | 0.134982 s |
| E / B proposal | 0.046277 / 0.014853 s | 0.012478 / 0.005582 s |
| E / B / R exact cost | 0.053887 / 0.047394 / 0.051578 s | 0.029025 / 0.027559 / 0.028749 s |
| P15 reassignment / rebuild | 0.141021 / 0.010693 s | 0.034149 / 0.002905 s |
| growth regret / seed / partition | 0 / 0 / 0 s | 0.054422 / 0.035428 / 0.056730 s |
| growth split total / split rebuild | 0 / 0 s | 0.155665 / 0.009041 s |
| growth reassignment / rebuild / cost | 0 / 0 / 0 s | 0.103110 / 0.008889 / 0.090684 s |
| final selection | 0.000374 s | 0.000214 s |
| selected writer | 2.375069 s | 2.177987 s |

Both profiles performed 973,053,692 exact counter updates. Maximum observed
block counts are 192/48; observed counter storage is 402,816/202,176 B and
retained plan storage 624,312/283,328 B. At 16384², Compact counters are
34,373,632 B with no split caches; LowLatency counter plus self-cost caches are
17,252,352 B. Conservatively retaining maximum legal plans remains below
40 MiB for either profile.

Against same-source no-feature output, the release test binary grows
2,132,272→2,230,912 B (+98,640/+4.626051%) and release rlib grows
2,158,672→2,398,368 B (+239,696/+11.103864%).

## Fair 41-image same-binary screen

The exact Phase A binary `05b8421c…64c9` produced every control and candidate
stream. Inputs were preloaded; one warmup preceded three forward/reverse/forward
rounds. Every measurement contains 41 per-image samples plus a nonempty
aggregate. The first runner invocation matched zero tests and is preserved in
`invalidated-runs`; after fixing only the external runner filter, the entire
screen restarted from warmup with the unchanged encoder binary.

| gate metric | Compact | LowLatency |
| --- | ---: | ---: |
| control encode median | 5.096220 s | 4.945387 s |
| candidate encode median | 2.453193 s | 2.432862 s |
| independent encode delta | **-51.862488%** | **-50.805420%** |
| paired delta median | -51.587374% | -50.805420% |
| per-image encode regressions | **0 / 41** | **0 / 41** |
| aggregate bytes delta | -3.482487% | -5.093032% |
| worst image bytes | +1.490531% (`002`) | -1.128722% (`031`) |
| images over +2% | **0 / 41** | **0 / 41** |
| Rust decode delta | **+0.893292%** | **+0.407526%** |
| pinned C decode delta | -0.768430% | -0.438525% |
| RSS delta | -319,127,552 B / -35.246006% | -323,059,712 B / -35.604269% |
| screen gate | **pass** | **pass** |

The JSON retains each aggregate/in-process sample, process wall/CPU/RSS,
per-image median, full-output hash, MAD, paired delta, and 3×MAD flag. Compact
Rust decode has one 3×MAD paired sample but no sample is deleted; the declared
gate uses independent medians and remains below +1%.

The project decoder validated 246/246 generated Default/Single/control/candidate
streams to complete RGBA. Pinned libwebp `733c91e` independently matched
246/246. All generator and oracle stderr files are empty. Because every screen
gate passed, formal 102x5 is permitted.

## Formal locked 102 images x5

The unchanged final binary ran one warmup and five measured
forward/reverse/forward/reverse/forward rounds for both controls and both
candidates. All 24 child processes contain 102 per-image samples and one
aggregate, and each reports exactly one passed reproducer test.

| formal metric | Compact | LowLatency |
| --- | ---: | ---: |
| control aggregate samples (s) | 12.045375 / 12.117027 / 12.011114 / 12.017532 / 12.011141 | 11.978154 / 12.120934 / 12.220682 / 11.789722 / 11.768870 |
| candidate aggregate samples (s) | 5.729042 / 5.704736 / 5.743691 / 5.721840 / 5.707062 | 5.784305 / 5.849997 / 5.821272 / 5.837145 / 5.770485 |
| candidate absolute median | **5.721840 s** | **5.821272 s** |
| absolute limit | ≤7.1 s | ≤6.9 s |
| independent median delta | -52.387558% | -51.400923% |
| paired delta median | -52.437830% | -51.709545% |
| per-image median regressions | **0 / 102** | **0 / 102** |
| formal gate | **pass** | **pass** |

No sample or outlier was removed. `formal-summary.json` and raw process files
retain all absolute/paired samples, process wall/CPU/RSS, per-image medians,
MAD, and 3×MAD flags. Compact control and paired series contain flagged
outliers, but the predeclared gate uses all five samples and still passes.

## Final correctness and stable quality gates

The unchanged candidate binary generated six layouts for all 102 images.
Project decode is **612/612 exact** and pinned libwebp `733c91e` is **612/612
exact**, with empty stderr. A same-source no-feature control independently
generated and project-decoded 408/408 streams; its 102 Default streams are
**102/102 byte-identical** to the feature binary's Default streams.

Stable host validation passed:

- default workspace all-target tests: 330 passed, 4 ignored;
- feature workspace all-target tests: 335 passed, 4 ignored;
- default and feature workspace all-target Clippy with `-D warnings`;
- `cargo fmt --all -- --check` (stable rustfmt reports the repository's known
  nightly-only `imports_granularity` warnings but exits successfully);
- default and feature rustdoc with `RUSTDOCFLAGS=-D warnings`;
- default and feature workspace doctests.

Only stable `aarch64-apple-darwin` was executed. The complete resource/API/module
audit is in `raw/resource-and-api-audit.md`: no dependency, unsafe, thread,
public API, Default, metadata, animation, or error-semantics change; all new
production modules are below 500 lines and dependencies remain directional.

## Reproduction, checksum, and product decision

The repository-root `reproduce.sh` rebuilt the experiment in an isolated target
and reran Phase A, the complete screen, formal 102x5, 612-stream project/pinned-C
correctness, 102 Default identities, and all nine stable quality commands. It
exited zero with `decision=promote-research`. Because Rust test binary hashes
embed target-path-dependent metadata, the isolated binary SHA is
`2e5f7b11…e570`; the script records the original `05b8421c…64c9` reference and
proves the rebuilt SHA is identical across every replay phase. Rates and stream
outputs reproduced exactly.

The replay's 185-entry relative manifest fully verifies and its `SHA256SUMS`
hash is `640bafc9…e6096`. The full replay log, concise machine summary, status,
and output-manifest hash are retained in this experiment directory. The final
branch-relative `SHA256SUMS` covers all archived evidence except itself.

P18 therefore passes as a research architecture. The recommendation to root is
to create a new independent product-migration task/worktree from the then-latest
committed local `main`, re-understand and rebuild the minimum planner split,
delete research traces/harnesses, and rerun product gates. This branch must not
be merged, rebased, or cherry-picked wholesale into main.
