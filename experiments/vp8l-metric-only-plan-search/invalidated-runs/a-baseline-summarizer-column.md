# Invalidated A-baseline summarizer column

The first complete A/P18 corpus encode at runner commit `38dfe5b4` wrote all
four 102-image measurement files with empty stderr, then failed before a gate
decision because `summarize_a_baseline.py` read TSV column 5 (elapsed
nanoseconds) as the image identifier instead of column 4.

The failed assertion was `a.keys() == p18.keys()`. Inspection showed matching
real identifiers, sizes, and hashes at the correct column positions; no stream
mismatch was reported. The complete invalidated output remains external at
`/private/tmp/vp8l-metric-only-plan-search-p22-a-38dfe5b4`.

The only summarizer change is `row[5]` to `row[4]`. Because the runner did not
produce a gate summary or success status, P22 will rerun the entire baseline in
a fresh external directory and will not use this invalidated run for a claim.
