# Codec fault-injection records

This directory holds small, reviewable artificial defects used to prove that
the test suite catches important codec failure classes.  It is not a place for
unbounded mutation output.

Each fault should live in its own directory and include:

- `README.md`: the injected change, affected invariant, expected failing tests,
  and the command used to demonstrate the failure;
- a minimal patch against a pinned revision; and
- a link to the normal test or fixture that kills it.

Start with faults for checked chunk-end arithmetic, VP8L signed colour
multipliers, Huffman table sizing, LZ77 overlap handling, and animation frame
rectangle bounds.  A fault that no test kills is a test-gap ticket, not a
passing mutation result.
