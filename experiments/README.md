# Experiment artifacts

This directory keeps the durable, reviewable part of an experiment:

- the report, design, manifest, and provenance;
- reproduction and summarization source;
- curated top-level summaries needed by the performance ledger;
- small regression fixtures used by repository tests or smoke checks.

Row-level measurements, process samples, build and validation logs, stderr,
checksums of generated output, and invalid or superseded run output are not
source artifacts. Reproducers write them to an explicit output directory
(normally below `/private/tmp`), where a run may generate and verify its own
`SHA256SUMS` without adding the output to Git.

Reports may name paths below `raw/` to describe the reproducer's output layout.
Those paths are intentionally absent from a clean checkout and are recreated
by the corresponding `reproduce.sh`.
