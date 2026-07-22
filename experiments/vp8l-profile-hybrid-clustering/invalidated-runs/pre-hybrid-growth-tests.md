# Pre-hybrid growth-test invocation

The first focused feature test invocation ran seven tests and reported 5 pass,
2 fail. Both failing tests constructed `ExactBlockFrequencies` with the Compact
profile and then directly invoked LowLatency-only merge-penalty/growth helpers.
P18 deliberately skips Compact self-cost and split state, so those inherited
P16 fixtures were invalid for the frozen hybrid ownership boundary.

The fixtures were changed to use a two-block LowLatency geometry for growth
tests. A Compact cross-model fixture now additionally asserts empty self-cost
caches and zero self-cost time. The corrected focused run passed 7/7. No corpus
or benchmark output was produced by either invocation.
