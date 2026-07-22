# Invalidated reproduction: target-path-sensitive binary hash assertion

The first full replay stopped immediately after building the feature test
binary. The script incorrectly required the rebuilt binary to equal the
original screen binary SHA-256. Rust test binaries embed target-path-dependent
metadata, so the isolated temporary target produced a different binary name
and hash even though source, code generation inputs, behavior, and stream
outputs were unchanged. No corpus measurement had begun; only the two locked
manifests were written to the external output.

The failed build log is retained here. The correction records both the original
reference SHA and rebuilt SHA, then requires Phase A, screen, formal, and all
correctness work within the replay to use that one rebuilt binary. Deterministic
rate totals, stream hashes, exactness, and every gate remain asserted. The
replay restarts into a new output directory.
