# Invalidated runs

- `superseded-selection-attribution`: complete first Phase A with valid
  rate/exactness but superseded timer and selected-128 field semantics.
- `missing-candidate-rlib`: malformed first artifact inventory.
- `wrong-implementation-sha`: reproducer preflight rejected an incorrectly
  expanded full SHA before any build or Phase A run.
- `isolated-binary-sha`: isolated build correctly demonstrated that release
  test binary SHA is path-dependent; Phase A had not started.

The preliminary smoke output is retained under `raw/smoke-008` and is not
used for a gate decision.
