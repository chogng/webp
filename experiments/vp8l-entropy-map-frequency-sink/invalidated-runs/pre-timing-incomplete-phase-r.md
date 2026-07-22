# Pre-timing Phase-R harness invalidation

The external run at
`/private/tmp/vp8l-entropy-map-frequency-sink-p25-phase-r-8fbab79d` is
preserved and invalidated as a harness failure, not as a mechanism or
performance result. It started at P25 commit `8fbab79d132db8866ff90366c240c01667146349`
and produced several non-timed 102-image identity files, but the runner ended
after writing `SHA256SUMS`: it did not invoke its summarizer, did not run or
gate the candidate census, did not check the P18 worktree was clean, and had
no recovery warmup or F/R/F section.

No warmup or timed recovery sample was produced. The run therefore cannot
consume P25's sole recovery screen. Its external `SHA256SUMS` digest is kept
with the raw directory; a corrected, committed harness must perform a fresh
complete non-timed audit before any recovery command.
