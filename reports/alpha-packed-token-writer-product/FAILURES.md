# Failures and invalidated evidence

No unresolved product or evidence failure remains.

## Pre-lint-amend measurements: invalidated

The first complete measurement set used example SHA-256
`edadf129723b2314a96d524ae6988fe609683069077d82d61ed033c3c51d9061`.
Feature Clippy then found a nonstandard hexadecimal grouping in the new
integration test's FNV constant. The literal was regrouped without changing
its value or production source, and the product commit was amended. Because
the rebuilt example hash changed, every earlier measurement was conservatively
invalidated and rerun. Raw evidence is retained under
`raw/invalidated/pre-lint-amend/`.

The invalidated results were:

| Run | ALPH change | Whole change | Other |
| --- | ---: | ---: | --- |
| 41x10x3 screen | -20.330% | -5.532% | zero mismatches |
| 41x10x5 formal | -21.670% | -5.332% | CPU -6.917%, RSS -3.641% |
| 15x5 generalization | -7.670% | not promoted | real -3.274%, synthetic -10.138% |
| 224 identity cases | exact | exact | project decoder and `dwebp` passed |

Three invalidated libwebp boundary logs are retained with that set. None is
used in the recommendation.

The original failing Clippy output is retained as
`raw/gates/clippy-webp-feature-invalid.log`; the corrected rerun is
`raw/gates/clippy-webp-feature.log`.

## Corrected command failures

- The first Bazel invocation could not access Bazel's cache outside the
  workspace sandbox. It produced no test result. The same three targets were
  rerun with cache access and passed; `raw/gates/bazel-tests.log` is the valid
  log.
- The first final libwebp command passed the `cwebp` executable where the
  harness expects the libwebp repository root. Its prerequisite check stopped
  before measurement. The corrected root path produced the three valid
  `libwebp-whole-final-*.log` files.
- An initial formatting check reported the new files. Formatting was applied
  before the code commit and the final check passes.
- Login-shell attempts emit a harmless `fnm_multishells` symlink warning under
  the managed sandbox. Cargo, test, and measurement commands still completed;
  status was checked independently.
