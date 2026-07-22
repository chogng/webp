# Manual census command-window invalidation

The external directory
`/private/tmp/vp8l-entropy-map-frequency-sink-p25-phase-r-manual-5cc3f96b`
contains a valid 204-stream identity audit and an incomplete candidate census.
The single census process was interrupted by the command window after image
`clic-validation-091`; its 92-image TSV is retained as
`candidate-census-incomplete.tsv` and is never used for a gate. No recovery
warmup or timed sample exists. The exact already-built P25 binary is reused by
the committed shard runner below to complete this non-timed evidence only.
