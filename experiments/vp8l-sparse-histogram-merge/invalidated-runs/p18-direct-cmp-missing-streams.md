# Invalidated direct P18 stream-file comparison

Task `P21 sparse exact-histogram merge recovery`, root task
`019f8321-035e-7211-8f53-987e18891c8c`, branch
`codex/vp8l-sparse-histogram-merge`, base
`8485fc0593bf6e29715350ea72b15a9dabf4c80b`, worktree
`/Users/lance/.codex/worktrees/1841/webp`.

The first direct `cmp` attempt for dense-A versus P18 named
`/private/tmp/vp8l-profile-hybrid-clustering-reproduction-4bc28f4a/raw/final-correctness-102/candidate-generated`
as its P18 stream root. That replay intentionally did not retain generated
stream files, so the first missing file was reported by the wrapper as a
`compact/clic-validation-000.webp` mismatch. This was a path-resolution error,
not a byte mismatch, and supplies no gate evidence.

The valid replacement used the P18 binary's retained per-image size and FNV-1a
stream hashes from P20's locked Phase A output. It matched dense A on 204/204
profile streams, with fixed aggregate sizes 599,398,064 and 601,400,998 bytes.
The valid result and raw hashes are recorded in `a-baseline-summary.json`.
