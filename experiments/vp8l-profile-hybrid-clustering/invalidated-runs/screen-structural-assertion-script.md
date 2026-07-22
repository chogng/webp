# Screen structural assertion script correction

The first read-only structural check used `r and predicate` inside Python
`sum()`. An empty CSV row therefore contributed `[]` instead of `False` and
raised `TypeError` before completing the assertions. No measurement was run or
changed. The check was rerun with `bool(r) and predicate` and passed: both Rust
runner directories contain 16 processes, every process contains 41 measurement
rows and one aggregate, every child reports exactly one passed test, both
run.json files name the final binary, and the 246/246 dual-decoder summary gate
is true.
