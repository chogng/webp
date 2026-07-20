# Smoke corpus

This directory is the fast, committed corpus run on every pull request.  The
M0 seed is an empty input fixture: it makes the container rejection path
testable through the public Rust API before a decoder exists.

As codec milestones land, add small valid VP8L, VP8, VP8X, ALPH, metadata, and
animation fixtures here.  Put larger upstream vectors in the pinned external
corpus and historical failures in `../regressions/` instead.
