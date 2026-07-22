# A18 host-coordination hold before recovery warmup

The first invocation of P23's locked Phase R runner began while the independent
A18 task still had final RSS/formal processes active on the same host. The root
task sent an urgent hold while P23 was in its non-timed 102-image audit.

P23 allowed the audit and identity work to continue, then interrupted the
runner before any warmup file existed. The interruption landed during the last
P18 LowLatency identity encode; its empty TSV and stderr were preserved
externally as `invalidated-interrupted-p18-low-latency.*`. There were exactly
zero `[0-9][0-9]-*.tsv` timing files at the stop.

After the root task explicitly sealed A18 with 5,400 measurements, 45/45
process/resource records, and zero stderr, P23 completed only the interrupted
non-timed identity encode. It then used the unchanged locked P23 binary SHA-256
`215d5bc969c1be7bb5caa24e2ae378cd4fb73cb4c3f82b16c529371642e7c125`
for exactly one warmup and the single retained F/R/F recovery screen. No timing
sample existed before the seal, so this hold is not a consumed or contaminated
valid recovery run.
